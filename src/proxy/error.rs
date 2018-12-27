use std::error::{Error as StdError};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Closed
}


impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::Closed => "Proxy detached",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            Error::Closed => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", (self as &StdError).description())
    }
}
