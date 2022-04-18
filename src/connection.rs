use crate::error::{Error, Result};
use crate::msg::FrameMsg; //, Msg2C, Msg2S};
use bytes::BytesMut; //{Buf, Bytes, };
use std::io::Cursor; //, Read, Write};
use std::marker::PhantomData;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader; //, BufWriter};
use tokio::net::tcp::OwnedReadHalf;
//use tokio::net::TcpStream;

#[derive(Debug)]
pub struct Connection<T: FrameMsg> {
    stream: BufReader<OwnedReadHalf>,
    buffer: BytesMut,
    cursor: usize,
    marker: PhantomData<T>, // another choice is using generic method
}

impl<T: FrameMsg> Connection<T> {
    pub fn new(stream: BufReader<OwnedReadHalf>) -> Self {
        //pub fn new(stream: BufReader<TcpStream>) -> Self {
        Self {
            stream,
            buffer: BytesMut::with_capacity(4096),
            cursor: 0,
            marker: PhantomData,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear(); // release used bytes
        self.cursor = 0;
    }

    // pub async fn is_safe_to_read(&mut self) -> bool {
    //     // 这里其实用到了协议的具体内容，封装地不是很好,
    // 	// 也不是很好用
    //     if !self.buffer.is_empty() {
    //         return self.buffer[0] == b'l';
    //     } else {
    //         // 这里不会加长 buffer 的长度，所以可以认为是安全的
    //         return match self.read_buf().await {
    //             Ok(n) if n > 0 => self.buffer[0] == b'l',
    //             _ => false,
    //         };
    //     }
    // }

    pub async fn read_frame(&mut self) -> Result<Option<T>> {
        loop {
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            if self.buffer.len() == self.buffer.capacity() {
                self.buffer.resize(self.buffer.len() << 1, 0); // default value 0
            }

            match self.read_buf().await {
                Ok(0) => return Ok(None),
                Err(e) => return Err(e),
                _ => (),
            }
        }
    }

    async fn read_buf(&mut self) -> Result<usize> {
        //match self.stream.read(&mut self.buffer[self.cursor..]).await {
        match self.stream.read_buf(&mut self.buffer).await {
            Ok(0) => {
                println!("Connection closed by peer");
                Ok(0)
            }
            Ok(n) => Ok(n),
            Err(_) => Err("Connection reset by peer".into()),
        }
    }

    fn parse_frame(&mut self) -> Result<Option<T>> {
        let mut buf = Cursor::new(&self.buffer[self.cursor..]); // NOTE: cursor
        match T::check(&mut buf) {
            Ok(_) => {
                buf.set_position(0);
                let msg = T::parse(&mut buf);
                self.cursor += buf.position() as usize;
                if self.cursor == self.buffer.len() {
                    self.clear();
                }
                Ok(Some(msg))
            }
            Err(Error::Incomplete) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
