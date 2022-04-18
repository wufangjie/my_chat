extern crate my_chat;
use my_chat::connection::Connection;
use my_chat::msg::{FrameMsg, Msg2C, Msg2S};
use my_chat::time::get_current_timestamp;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};

type MsgQueue = Arc<Mutex<VecDeque<(u64, Vec<u8>)>>>;
type PushDict = Arc<Mutex<HashMap<u64, Vec<Vec<u8>>>>>;
type Connected = Arc<Mutex<HashMap<u64, Option<BufWriter<OwnedWriteHalf>>>>>;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

    let msg_queue: MsgQueue = Default::default();
    let push_dict: PushDict = Default::default();
    let connected: Connected = Default::default();

    // Add 4 concurrent senders
    for _ in 0..4 {
        {
            let msg_queue = msg_queue.clone();
            let push_dict = push_dict.clone();
            let connected = connected.clone();
            tokio::spawn(async move {
                send_loop(msg_queue, push_dict, connected).await;
            });
        }
    }

    // TODO: 这里没有对 socket 上限进行限制
    loop {
        let msg_queue = msg_queue.clone();
        let push_dict = push_dict.clone();
        let connected = connected.clone();
        let (socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            recv_loop(socket, msg_queue, push_dict, connected).await;
        });
    }
}

async fn recv_loop(
    socket: TcpStream,
    msg_queue: MsgQueue,
    push_dict: PushDict,
    connected: Connected,
) {
    //let mut conn = Connection::<Msg2S>::new(BufReader::new(socket));

    //let (mut reader, mut writer) = socket.split();
    let (reader, writer) = socket.into_split();
    // can be moved to seperate taks at the cost of heap allocation
    let mut conn = Connection::<Msg2S>::new(BufReader::new(reader));
    let mut writer = BufWriter::new(writer);
    let mut login_user_id = 0u64;
    let mut message_id = 0u64; // 没有数据库，都从 0 开始吧

    // 理论上应该先验证登录，而不是直接解析，这样可以防止匿名长消息攻击
    // TODO: 用户断网（不同时长）时重连的处理，都需要重新登录好像不 ergonomic
    while let Some(msg) = conn.read_frame().await.unwrap() {
        dbg!(&msg);
        if let Msg2S::Login { user_id } = msg {
            // TODO: 本来应该判断，user_id 有没有对应的 client，做一些处理
            // 不过这样写太复杂了，以后再说，下面的 login 同理
            connected.lock().unwrap().insert(user_id, Some(writer));
            login_user_id = user_id;
            break;
        } else {
            writer
                .write_all(&Msg2C::AuthRequired.to_bytes())
                .await
                .unwrap();
            writer.flush().await.unwrap();
        }
    }

    while let Some(msg) = conn.read_frame().await.unwrap() {
        dbg!(&msg);
        match msg {
            Msg2S::Msg {
                fake_msg_id,
                to,
                len,
                msg,
            } => {
                let mut mq = msg_queue.lock().unwrap();
                message_id += 1;

                mq.push_back((
                    login_user_id,
                    Msg2C::Update {
                        fake_msg_id,
                        real_msg_id: message_id,
                    }
                    .to_bytes(),
                ));

                mq.push_back((
                    to,
                    Msg2C::Msg {
                        msg_id: message_id,
                        from: login_user_id,
                        ts: get_current_timestamp(),
                        len,
                        msg,
                    }
                    .to_bytes(),
                ));
            }
            Msg2S::Login { user_id } => {
                if let Some(client) = connected.lock().unwrap().remove(&user_id) {
                    connected.lock().unwrap().insert(login_user_id, client);
                }
                //connected.lock().unwrap().insert
                //connected.lock().unwrap().insert(user_id, Some(writer));
                login_user_id = user_id;
            }
            Msg2S::Pull => {
                // NOTE: 其他地方锁了 push_dict 的话都是在插数据，不会 take out
                if let Some(pq) = push_dict.lock().unwrap().remove(&login_user_id) {
                    let mut mq = msg_queue.lock().unwrap();
                    for msg in pq {
                        mq.push_back((login_user_id, msg));
                    }
                }
            }
            Msg2S::Beat => {
                // do nothing
            }
        }
    }
}

async fn send_loop(msg_queue: MsgQueue, push_dict: PushDict, connected: Connected) {
    // push_dict 会在这里添加，会在用户上线时减少
    // connected 会在这里减少（发送失败时），会在用户登录时增加
    // 没有两个同时 lock，所以不会造成死锁
    loop {
        let first = msg_queue.lock().unwrap().pop_front();
        // if let Some((user_id, msg)) = msg_queue.lock().unwrap().pop_front()
        // 会被认为 block 内还会锁着 msg_queue, rust 还是不够智能

        if first.is_none() {
            // no data in msg_queue
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        } else {
            let (user_id, msg) = first.unwrap();

            loop {
                if connected.lock().unwrap().get(&user_id).is_none() {
                    // 离线
                    (*push_dict.lock().unwrap().entry(user_id).or_insert(vec![])).push(msg);
                } else {
                    let mut take_out = None::<BufWriter<OwnedWriteHalf>>;
                    std::mem::swap(
                        &mut take_out,
                        connected.lock().unwrap().get_mut(&user_id).unwrap(),
                    );
                    if let Some(mut client) = take_out {
                        // TODO: 错误处理，什么时候需要再试，
                        // 什么时候要删掉 client, 并加到 push_dict 中
                        let mut succeed = true;
                        if client.write_all(&msg).await.is_err() {
                            succeed = false;
                        }
                        if succeed && client.flush().await.is_err() {
                            succeed = false;
                        }

                        if succeed {
                            // 换回去，注意这个时候 client 一定是存在的，因为只能在此处删除
                            std::mem::swap(
                                &mut Some(client),
                                connected.lock().unwrap().get_mut(&user_id).unwrap(),
                            );
                        } else {
                            // 移除
                            connected.lock().unwrap().remove(&user_id);
                            (*push_dict.lock().unwrap().entry(user_id).or_insert(vec![])).push(msg);
                        }
                    } else {
                        // 使用中, async wait
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                        continue;
                    };
                }
                break;
            }
        }
    }
}
