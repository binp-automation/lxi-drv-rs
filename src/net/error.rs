use std::io;
use std::error::{Error as StdError};
use std::fmt;


#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    NotConnected,
    AlreadyConnected,
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::Io(e) => e.description(),
            Error::AlreadyConnected => "Empty",
            Error::NotConnected => "Not empty",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            Error::Io(e) => Some(e),
            Error::AlreadyConnected => None,
            Error::NotConnected => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", (self as &StdError).description())
    }
}
