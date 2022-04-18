use crate::error::{Error, Result};
use bytes::Buf;
use std::io::{Cursor, Read};

pub trait FrameMsg {
    fn check(src: &mut Cursor<&[u8]>) -> Result<()>;

    fn parse(src: &mut Cursor<&[u8]>) -> Self;

    fn to_bytes(&self) -> Vec<u8>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Msg2C {
    Msg {
        msg_id: u64,
        from: u64,
        ts: i64, // use chrono
        len: u64,
        msg: String,
    },

    Update {
        fake_msg_id: i64, // use negative
        real_msg_id: u64,
    },

    Quit,         // b"q"
    Ok,           // b"o"
    Err,          // b"e"
    AuthRequired, // b'a'
}

impl FrameMsg for Msg2C {
    fn check(src: &mut Cursor<&[u8]>) -> Result<()> {
        match get_u8(src)? {
            b'<' => {
                let start = src.position() as usize;
                let end = src.get_ref().len();
                if end - start < 32 {
                    Err(Error::Incomplete)
                } else {
                    skip(src, 24)?;
                    let len = src.get_u64();
                    skip(src, len as usize)
                }
            }
            b'u' => skip(src, 16),
            b'q' | b'o' | b'e' | b'a' => Ok(()),
            b => Err(Error::Invalid(b)),
        }
    }

    fn parse(src: &mut Cursor<&[u8]>) -> Self {
        match get_u8(src).unwrap() {
            b'<' => {
                let msg_id = src.get_u64();
                let from = src.get_u64();
                let ts = src.get_i64();
                let len = src.get_u64();
                let mut bytes = vec![0; len as usize];
                // Vec::with_capacity(len as usize); // get 0 bytes
                src.read_exact(&mut bytes).unwrap();
                // it's safe to unwrap() after check
                let msg = String::from_utf8_lossy(&bytes).to_string();
                Self::Msg {
                    msg_id,
                    from,
                    ts,
                    len,
                    msg,
                }
            }
            b'u' => Self::Update {
                fake_msg_id: src.get_i64(),
                real_msg_id: src.get_u64(),
            },
            b'q' => Self::Quit,
            b'o' => Self::Ok,
            b'e' => Self::Err,
            b'a' => Self::AuthRequired,
            _ => panic!("Please call check() first"),
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut res: Vec<u8> = vec![];
        match self {
            Self::Msg {
                msg_id,
                from,
                ts,
                len,
                msg,
            } => {
                res.push(b'<');
                res.extend(msg_id.to_be_bytes());
                res.extend(from.to_be_bytes());
                res.extend(ts.to_be_bytes());
                res.extend(len.to_be_bytes());
                res.extend(msg.bytes());
            }
            Self::Update {
                fake_msg_id,
                real_msg_id,
            } => {
                res.push(b'u');
                res.extend(fake_msg_id.to_be_bytes());
                res.extend(real_msg_id.to_be_bytes());
            }
            Self::Quit => res.push(b'q'),
            Self::Ok => res.push(b'o'),
            Self::Err => res.push(b'e'),
            Self::AuthRequired => res.push(b'a'),
        }
        res
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Msg2S {
    Msg {
        fake_msg_id: i64, // use negative
        to: u64,
        len: u64,
        msg: String,
    }, // need to send to another user

    Login {
        // b"l"
        user_id: u64,
        // len: usize,
        // password: String,
    },

    Pull, // b"p" // pull
    Beat, // b"?" // beat
}

impl FrameMsg for Msg2S {
    fn check(src: &mut Cursor<&[u8]>) -> Result<()> {
        match get_u8(src)? {
            b'>' => {
                let start = src.position() as usize;
                let end = src.get_ref().len();
                if end - start < 24 {
                    Err(Error::Incomplete)
                } else {
                    skip(src, 16)?;
                    let len = src.get_u64();
                    skip(src, len as usize)
                }
            }
            b'l' => skip(src, 8),
            b'p' | b'?' => Ok(()),
            b => Err(Error::Invalid(b)),
        }
    }

    fn parse(src: &mut Cursor<&[u8]>) -> Self {
        match get_u8(src).unwrap() {
            b'>' => {
                let fake_msg_id = src.get_i64();
                let to = src.get_u64();
                let len = src.get_u64();
                let mut bytes = vec![0; len as usize];
                src.read_exact(&mut bytes).unwrap();
                // it's safe to unwrap() after check
                let msg = String::from_utf8_lossy(&bytes).to_string();
                Self::Msg {
                    fake_msg_id,
                    to,
                    len,
                    msg,
                }
            }
            b'l' => Self::Login {
                user_id: src.get_u64(),
            },
            b'p' => Self::Pull,
            b'?' => Self::Beat,
            _ => panic!("Please call check() first"),
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut res: Vec<u8> = vec![];
        match self {
            Self::Msg {
                fake_msg_id,
                to,
                len,
                msg,
            } => {
                res.push(b'>');
                res.extend(fake_msg_id.to_be_bytes());
                res.extend(to.to_be_bytes());
                res.extend(len.to_be_bytes());
                res.extend(msg.bytes());
            }
            Self::Login { user_id } => {
                res.push(b'l');
                res.extend(user_id.to_be_bytes());
            }
            Self::Pull => res.push(b'p'),
            Self::Beat => res.push(b'?'),
        }
        res
    }
}

// impl Msg2S {
//     fn is_login_msg(src: &mut Cursor<&[u8]>) -> bool {
//         matches!(peek_u8(src), Ok(b'l'))
//         // if let Ok(b'l') = peek_u8(src) {
//         //     true
//         // } else {
//         //     false
//         // }
//     }
// }

// fn peek_u8(src: &mut Cursor<&[u8]>) -> Result<u8> {
//     if !src.has_remaining() {
//         return Err(Error::Incomplete);
//     }
//     Ok(src.chunk()[0])
// }

fn get_u8(src: &mut Cursor<&[u8]>) -> Result<u8> {
    if !src.has_remaining() {
        return Err(Error::Incomplete);
    }
    Ok(src.get_u8())
}

fn skip(src: &mut Cursor<&[u8]>, n: usize) -> Result<()> {
    if src.remaining() < n {
        return Err(Error::Incomplete);
    }
    src.advance(n);
    Ok(())
}

// fn write_decimal(val: u64) -> [u8; 8] {
//     val.to_be_bytes()
// }

#[cfg(test)]
mod test {
    use super::*;
    use crate::time::get_current_timestamp;
    use utils::dbgt;

    #[ignore]
    #[test]
    fn test_cursor() {
        let mut buf = &b"hello world"[..];
        assert_eq!(buf.remaining(), 11);
        buf.get_u8();
        assert_eq!(buf.remaining(), 10);
        dbgt!(&buf.chunk());

        let buf = Cursor::new(b"hello world");
        dbgt!(&buf);
        let buf = Cursor::new(vec![255u8; 8]);
        dbgt!(&buf);
    }

    #[test]
    fn test_msg2c() {
        let msg = "hello world!".to_string();
        let lst = vec![
            Msg2C::Msg {
                msg_id: 1234,
                from: 5678,
                ts: get_current_timestamp(),
                len: msg.bytes().len() as u64,
                msg,
            },
            Msg2C::Update {
                fake_msg_id: -1,
                real_msg_id: 99999,
            },
            Msg2C::Quit,
            Msg2C::Ok,
            Msg2C::Err,
            Msg2C::AuthRequired,
        ];

        for item in lst {
            let bytes = item.to_bytes();
            let mut buf = Cursor::new(&bytes[..]);
            assert_eq!(item, Msg2C::parse(&mut buf));
        }

        // assert!(Msg2C::check(&mut Cursor::new(b"l")).is_err());
        // assert!(Msg2C::check(&mut Cursor::new(b"e")).is_ok());
    }

    #[test]
    fn test_parse_msg2s() {
        let msg = "hello world!".to_string();
        let lst = vec![
            Msg2S::Msg {
                fake_msg_id: -1234,
                to: 5678,
                len: msg.bytes().len() as u64,
                msg,
            },
            Msg2S::Login { user_id: 88888 },
            Msg2S::Pull,
            Msg2S::Beat,
        ];

        for item in lst {
            let bytes = item.to_bytes();
            let mut buf = Cursor::new(&bytes[..]);
            assert_eq!(item, Msg2S::parse(&mut buf));
        }

        assert!(Msg2S::check(&mut Cursor::new(b"e")).is_err());
        assert!(Msg2S::check(&mut Cursor::new(b"l")).is_err());
        assert!(Msg2S::check(&mut Cursor::new(b"p")).is_ok());
    }

    // #[test]
    // fn test_is_login_msg() {
    //     assert!(Msg2S::is_login_msg(&mut Cursor::new(b"l")));
    //     assert!(!Msg2S::is_login_msg(&mut Cursor::new(b"p")));
    // }
}
