use std::io;
use std::ops::{Deref, DerefMut};
use std::collections::{HashMap};

use mio::{self, Poll, Token, Ready, PollOpt};

use ::error::{IdError};


pub type Id = usize;
pub type Eid = usize;

pub const EID_BITS: usize = 8;
pub const EID_MASK: usize = (1<<EID_BITS) - 1;

pub fn encode_ids(id: Id, eid: Eid) -> Option<Token> {
    let base = id << EID_BITS;
    if base >> EID_BITS != id {
        return None;
    }
    if eid & EID_MASK != eid {
        return None;
    }
    Some(Token(base | eid))
}

pub fn decode_ids(token: Token) -> (Id, Eid) {
    let id = token.0 >> EID_BITS;
    let eid = token.0 & EID_MASK;
    (id, eid)
}

pub trait Evented: mio::Evented {
    fn id(&self) -> Eid;
}

pub struct EventedWrapper<E: mio::Evented> {
    handle: E,
    eid: Eid,
}

impl<E: mio::Evented> EventedWrapper<E> {
    pub fn new(handle: E, eid: Eid) -> Self {
        Self { handle, eid }
    }
}

impl<E: mio::Evented> Deref for EventedWrapper<E> {
    type Target = E;
    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl<E: mio::Evented> DerefMut for EventedWrapper<E> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.handle
    }
}

impl<E: mio::Evented> mio::Evented for EventedWrapper<E> {
    fn register(&self, poll: &Poll, token: Token, ready: Ready, poll_opt: PollOpt) -> io::Result<()> {
        self.handle.register(poll, token, ready, poll_opt)
    }
    fn reregister(&self, poll: &Poll, token: Token, ready: Ready, poll_opt: PollOpt) -> io::Result<()> {
        self.handle.reregister(poll, token, ready, poll_opt)
    }
    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.handle.deregister(poll)
    }
}

impl<E: mio::Evented> Evented for EventedWrapper<E> {
    fn id(&self) -> Eid {
        self.eid
    }
}

pub trait Control {
    fn register(&mut self, handle: &Evented, interest: Ready) -> ::Result<()>;
    fn reregister(&mut self, handle: &Evented, interest: Ready) -> ::Result<()>;
    fn deregister(&mut self, handle: &Evented) -> ::Result<()>;
    fn has_error(&self) -> bool;
    fn close(&mut self);
    fn is_closed(&self) -> bool;
}

pub struct PollInfo {
    pub ready: Ready,
    pub fresh: bool,
}

pub struct BaseControl<'a> {
    pid: Id,
    poll: &'a Poll,
    map: &'a mut HashMap<Eid, PollInfo>,
    error: bool,
    closed: bool,
}

impl<'a> BaseControl<'a> {
    pub(crate) fn new(pid: Id, poll: &'a Poll, map: &'a mut HashMap<Eid, PollInfo>) -> Self {
        Self {
            pid, poll, map,
            error: false,
            closed: false,
        }
    }
}

impl<'a> Control for BaseControl<'a> {
    fn register(&mut self, handle: &Evented, interest: Ready) -> ::Result<()> {
        encode_ids(self.pid, handle.id()).ok_or(IdError::Bad.into()).and_then(|token| {
            self.poll.register(
                handle, token, interest, 
                PollOpt::edge() | PollOpt::oneshot(),
            ).map_err(|e| e.into())
        }).and_then(|_| {
            match self.map.insert(handle.id(), PollInfo { ready: interest, fresh: true }) {
                None => Ok(()),
                Some(_) => Err(IdError::Present.into()),
            }
        }).or_else(|e| {
            self.error = true;
            Err(e)
        })
    }

    fn reregister(&mut self, handle: &Evented, interest: Ready) -> ::Result<()> {
        match self.map.get_mut(&handle.id()) {
            Some(info) => {
                info.ready = interest;
                info.fresh = true;
                Ok(())
            },
            None => Err(IdError::Missing.into()),
        }.and_then(|_| {
            encode_ids(self.pid, handle.id()).ok_or(IdError::Bad.into())
        }).and_then(|token| {
            self.poll.register(
                handle, token, interest, 
                PollOpt::edge() | PollOpt::oneshot(),
            ).map_err(|e| e.into())
        }).or_else(|e| {
            self.error = true;
            Err(e)
        })
    }

    fn deregister(&mut self, handle: &Evented) -> ::Result<()> {
        match self.map.remove(&handle.id()) {
            Some(_) => Ok(()),
            None => Err(IdError::Missing.into()),
        }.and_then(|_| {
            self.poll.deregister(handle).map_err(|e| e.into())
        }).or_else(|e| {
            self.error = true;
            Err(e)
        })
    }

    fn has_error(&self) -> bool {
        self.error
    }

    fn close(&mut self) {
        self.closed = true;
    }
    fn is_closed(&self) -> bool {
        self.closed
    }
}

pub struct EventControl<'a> {
    base: BaseControl<'a>,
    eid: Eid,
    ready: Ready,
}

impl<'a> EventControl<'a> {
    pub(crate) fn new(base: BaseControl<'a>, eid: Eid, ready: Ready) -> Self {
        Self {
            base,
            eid, ready,
        }
    }
    pub fn id(&self) -> Eid {
        self.eid
    }
    pub fn readiness(&self) -> Ready {
        self.ready
    }
}

impl<'a> Control for EventControl<'a> {
    fn register(&mut self, handle: &Evented, interest: Ready) -> ::Result<()> {
        self.base.register(handle, interest)
    }
    fn reregister(&mut self, handle: &Evented, interest: Ready) -> ::Result<()> {
        self.base.reregister(handle, interest)
    }
    fn deregister(&mut self, handle: &Evented) -> ::Result<()> {
        self.base.deregister(handle)
    }
    fn has_error(&self) -> bool {
        self.base.has_error()
    }
    fn close(&mut self) {
        self.base.close()
    }
    fn is_closed(&self) -> bool {
        self.base.is_closed()
    }
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
