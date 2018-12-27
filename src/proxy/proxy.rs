use std::error::{Error as StdError};
use std::fmt;

use super::control::{AttachControl, DetachControl, EventControl};

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

pub trait Proxy {
    fn attach(&mut self, ctrl: &mut AttachControl) -> ::Result<()>;
    fn detach(&mut self, ctrl: &mut DetachControl) -> ::Result<()>;

    fn process(&mut self, ctrl: &mut EventControl) -> ::Result<()>;
}
