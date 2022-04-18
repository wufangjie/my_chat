extern crate my_chat;
use my_chat::connection::Connection;
use my_chat::msg::{FrameMsg, Msg2C, Msg2S};
use regex::Regex;
use std::io::{stdout, Write};
use std::sync::{Arc, RwLock};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

pub struct Console {
    user_id: Option<u64>,
    send_to: Option<u64>,
    //input_string: String, //TODO: 能否得到输入了一半但没按回车的字符
}

impl Console {
    fn new() -> Self {
        Self {
            user_id: None,
            send_to: None,
            //input_string: String::new(),
        }
    }

    fn login(&mut self, user_id: u64) {
        self.user_id = Some(user_id);
    }

    fn send_to(&mut self, user_id: u64) {
        self.send_to = Some(user_id);
    }

    fn help(&self) {
        println!("界面介绍（以行为单位）：");
        println!("user_id> 表示等待输入消息发送给对方");
        println!("user_id< 表示收到对方的消息");

        println!();
        println!("特殊命令：");
        println!("!login your_user_id    登录");
        println!("!to send_to_user_id    改变聊天对象");
        println!("!quit                  退出客户端");
        println!("!pull                  获取离线消息");
        println!("!help                  打印本帮助信息");
        println!();
        println!("操作流程：先登录，指定要发消息的 user_id，然后开始聊天吧");
        println!();
        self.newline();
    }

    fn newline(&self) {
        if self.user_id.is_none() {
            println!("请先登录！");
            print!("> ");
            stdout().flush().unwrap();
            return;
        }

        if let Some(user_id) = self.send_to {
            print!("to {}> ", user_id);
        } else {
            print!("to server only> ");
        }

        //print!("to {}> {}", self.send_to.unwrap(), self.input_string);
        stdout().flush().unwrap();
    }
}

#[tokio::main]
async fn main() {
    let console = Console::new();
    console.help();
    // console.newline();

    let console = Arc::new(RwLock::new(console));
    let console_cloned = console.clone();
    // 几乎是不怎么变化的，所以用 RwLock 很合适

    let socket = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let (reader, writer) = socket.into_split();

    let (tx, rx) = mpsc::channel(2);

    // tokio::select!(recv_loop
    // tokio::join!(
    //     recv_loop(reader, console.clone()),
    //     send_loop(writer, console)
    // );

    // join 会等待全部任务完成，而 select 则是最短任务完成
    tokio::select!(
    _ = main_loop(tx, console) => {},
    _ = send_loop(rx, writer) => {},
    _ = recv_loop(reader, console_cloned) => {},
    );
}

async fn recv_loop(reader: OwnedReadHalf, console: Arc<RwLock<Console>>) {
    // NOTE: 如何优雅地打印，是难点，但不是重点，先不做
    // \r 移到行首后，继续输入会是覆盖状态，而不是插入
    // \x08 退格
    let mut conn = Connection::<Msg2C>::new(BufReader::new(reader));
    while let Some(frame) = conn.read_frame().await.unwrap() {
        dbg!(&frame);
        // NOTE: 暂时不保存消息 collections
        match frame {
            Msg2C::Msg {
                //msg_id,
                from,
                //ts,
                //len,
                msg,
                ..
            } => {
                println!("\nfrom {} < {}", from, msg);
            }
            Msg2C::Update {
                fake_msg_id,
                real_msg_id,
            } => {
                println!(
                    "\nfrom server < Update msg_id from {} to {}",
                    fake_msg_id, real_msg_id
                );
            }
            Msg2C::Quit => {
                println!("\nBye");
                return;
            }
            Msg2C::Ok => {
                println!("\nfrom server < Ok");
            }
            Msg2C::Err => {
                println!("\nfrom server < Err");
            }
            Msg2C::AuthRequired => {
                println!("\nfrom server < Authorization Required");
            }
        }

        console.read().unwrap().newline();
    }
}

async fn main_loop(tx: mpsc::Sender<Msg2S>, console: Arc<RwLock<Console>>) {
    // 客户端并发不高，且保证顺序，不需要引入消息队列
    // io::stdin() 挺好用的，不需要把控制台读取单独做一个任务
    let mut input_reader = BufReader::new(io::stdin());
    let mut input_string = String::new();
    let mut fake_msg_id = 0;

    let reg_set = Regex::new(r"^!(login|to)\s+(\d+)").unwrap();

    loop {
        input_string.clear();
        input_reader.read_line(&mut input_string).await.unwrap();
        input_string = input_string.trim().to_string();
        //println!("{}", input_string);

        match input_string.as_str() {
            "" => continue,
            "!quit" => return,
            "!help" => console.read().unwrap().help(),
            "!pull" => {
                console.read().unwrap().newline();
                tx.send(Msg2S::Pull).await.unwrap();
            }
            _ => {
                if let Some(caps) = reg_set.captures(&input_string) {
                    let user_id = caps.get(2).unwrap().as_str().parse::<u64>().unwrap();
                    match caps.get(1).unwrap().as_str() {
                        "login" => {
                            console.read().unwrap().newline();
                            console.write().unwrap().login(user_id);
                            // user_id 只是一个标记，并非表示登录成功
                            // 如果验证失败的话，服务端会返回 Msg2C::Quit
                            tx.send(Msg2S::Login { user_id }).await.unwrap();
                        }
                        "to" => {
                            console.write().unwrap().send_to(user_id);
                            console.read().unwrap().newline();
                        }
                        _ => unimplemented!(),
                    }
                } else {
                    console.read().unwrap().newline();
                    if let Some(user_id) = console.read().unwrap().send_to {
                        fake_msg_id -= 1;

                        tx.send(Msg2S::Msg {
                            fake_msg_id,
                            to: user_id,
                            len: input_string.bytes().len() as u64,
                            msg: input_string.clone(),
                        })
                        .await
                        .unwrap();
                    }
                }
            }
        }
    }
}

async fn send_loop(mut rx: mpsc::Receiver<Msg2S>, writer: OwnedWriteHalf) {
    // 发送任务很耗时的话，需要不影响不依赖发送的任务 (比如 !to)
    // 所以这里把发送单独分出来了
    let mut writer = BufWriter::new(writer);
    while let Some(msg) = rx.recv().await {
        // tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        // 用于测试耗时任务
        writer.write_all(&msg.to_bytes()).await.unwrap();
        writer.flush().await.unwrap();
    }
}
