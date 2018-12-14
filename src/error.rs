use std::error;
use std::fmt;
use std::io;

use ::channel;
use ::proxy;


#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Id(IdError),
    Channel(channel::Error),
    Proxy(proxy::Error),
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::Io(e) => e.description(),
            Error::Id(e) => e.description(),
            Error::Channel(e) => e.description(),
            Error::Proxy(e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            Error::Io(e) => Some(e),
            Error::Id(e) => Some(e),
            Error::Channel(e) => Some(e),
            Error::Proxy(e) => Some(e),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", (self as &error::Error).description())
    }
}

#[derive(Debug)]
pub enum IdError {
    Present,
    Missing,
    Bad
}

impl error::Error for IdError {
    fn description(&self) -> &str {
        match self {
            IdError::Present => "Id already present",
            IdError::Missing => "Id is missing",
            IdError::Bad => "Iad id",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", (self as &error::Error).description())
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<channel::Error> for Error {
    fn from(err: channel::Error) -> Error {
        Error::Channel(err)
    }
}

impl From<IdError> for Error {
    fn from(err: IdError) -> Error {
        Error::Id(err)
    }
}

impl From<proxy::Error> for Error {
    fn from(err: proxy::Error) -> Error {
        Error::Proxy(err)
    }
}
