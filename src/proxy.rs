use std::error;
use std::fmt;

use mio::*;

use ::result;


pub type Id = usize;
pub type Eid = usize;

pub const EID_BITS: usize = 8;
pub const EID_MASK: usize = (1<<EID_BITS) - 1;

pub fn encode_ids(id: Id, port: Eid) -> Option<mio::Token> {
    let base = id << EID_BITS;
    if base >> EID_BITS != id {
        return None;
    }
    if port & EID_MASK != port {
        return None;
    }
    Some(mio::Token(base | port))
}

pub fn decode_ids(token: mio::Token) -> (Id, Eid) {
    let id = token.0 >> EID_BITS;
    let port = token.0 & EID_MASK;
    (id, port)
}

#[derive(Debug)]
pub enum Error {
    Closed
}


impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::Closed => "Proxy detached",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            Error::Closed => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", (self as &error::Error).description())
    }
}


#[derive(Debug)]
pub enum Action {
    Close,
    Nope,
}

pub trait Proxy {
    fn attach(&mut self, poll: &Poll, id: Id) -> result::Result<()>;
    fn detach(&mut self, poll: &Poll) -> result::Result<()>;

    fn process(&mut self, poll: &Poll, event: &Event) -> result::Result<Action>;
}


#[cfg(test)]
mod test {
    use super::*;

    use std::mem;


    #[test]
    fn ids_encode() {
        assert_eq!(
            (456 << EID_BITS) | 123,
            encode_ids(456, 123).unwrap().0
        );

        assert!(encode_ids(0, (1<<EID_BITS)+1).is_none());
        assert!(encode_ids(1<<(8*mem::size_of::<usize>()-1), 0).is_none());
    }

    #[test]
    fn ids_decode() {
        assert_eq!(
            decode_ids(Token((456 << EID_BITS) | 123)),
            (456, 123)
        );
    }
}
