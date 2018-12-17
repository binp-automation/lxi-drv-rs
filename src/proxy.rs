use std::error::{Error as StdError};
use std::fmt;

use mio;

use ::error::{IdError};


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


pub struct Control<'a> {
    pub(crate) id: Id,
    pub(crate) poll: &'a mio::Poll,
    pub(crate) closed: bool,
}

impl<'a> Control<'a> {
    pub(crate) fn new(id: Id, poll: &'a mio::Poll) -> Self {
        Self { id, poll, closed: false }
    }

    pub fn register<E: mio::Evented>(&mut self, handle: &E, eid: Eid, interest: mio::Ready, opts: mio::PollOpt) -> ::Result<()> {
        self.poll.register(
            handle, 
            encode_ids(self.id, eid).ok_or(::Error::from(IdError::Bad))?, 
            interest, 
            opts,
        ).map_err(|e| ::Error::from(e))
    }

    pub fn deregister<E: mio::Evented>(&mut self, handle: &E) -> ::Result<()> {
        self.poll.deregister(handle).map_err(|e| ::Error::from(e))
    }

    pub fn close(&mut self) {
        self.closed = true;
    }
}

pub trait Proxy {
    fn attach(&mut self, ctrl: &mut Control) -> ::Result<()>;
    fn detach(&mut self, ctrl: &mut Control) -> ::Result<()>;

    fn process(&mut self, ctrl: &mut Control, readiness: mio::Ready, eid: Eid) -> ::Result<()>;
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
            decode_ids(mio::Token((456 << EID_BITS) | 123)),
            (456, 123)
        );
    }
}
