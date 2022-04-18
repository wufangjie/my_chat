use std::fmt;

pub type DynError = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Incomplete,
    Invalid(u8),
    Other(DynError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Incomplete => "Stream ended early".fmt(f),
            Self::Invalid(c) => write!(f, "Invalid msg type: {}", c),
            Self::Other(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(src: String) -> Error {
        Error::Other(src.into()) // temporary StringError
    }
}

impl From<&str> for Error {
    fn from(src: &str) -> Error {
        src.to_string().into()
    }
}
