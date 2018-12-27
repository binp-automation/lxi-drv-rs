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

pub struct PollInfo {
    pub ready: Ready,
    pub fresh: bool,
}

pub struct Control<'a> {
    pid: Id,
    poll: &'a Poll,
    map: &'a mut HashMap<Eid, PollInfo>,
}

impl<'a> Control<'a> {
    pub(crate) fn new(pid: Id, poll: &'a Poll, map: &'a mut HashMap<Eid, PollInfo>) -> Self {
        Self { pid, poll, map }
    }
}

impl<'a> Control<'a> {
    pub fn proxy_id(&self) -> Id {
        self.pid
    }

    pub fn register(&mut self, handle: &Evented, interest: Ready) -> ::Result<()> {
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
        })
    }

    pub fn reregister(&mut self, handle: &Evented, interest: Ready) -> ::Result<()> {
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
        })
    }

    pub fn deregister(&mut self, handle: &Evented) -> ::Result<()> {
        match self.map.remove(&handle.id()) {
            Some(_) => Ok(()),
            None => Err(IdError::Missing.into()),
        }.and_then(|_| {
            self.poll.deregister(handle).map_err(|e| e.into())
        })
    }
}

pub struct CloseControl<'a> {
    base: Control<'a>,
    closed: bool,
}

impl<'a> CloseControl<'a> {
    pub(crate) fn new(pid: Id, poll: &'a Poll, map: &'a mut HashMap<Eid, PollInfo>) -> Self {
        Self { base: Control::new(pid, poll, map), closed: false }
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl<'a> Deref for CloseControl<'a> {
    type Target = Control<'a>;
    fn deref(&self) -> &Control<'a> {
        &self.base
    }
}
impl<'a> DerefMut for CloseControl<'a> {
    fn deref_mut(&mut self) -> &mut Control<'a> {
        &mut self.base
    }
}

pub type AttachControl<'a> = CloseControl<'a>;
pub type DetachControl<'a> = Control<'a>;

pub struct EventControl<'a> {
    base: CloseControl<'a>,
    eid: Eid,
    ready: Ready,
}

impl<'a> EventControl<'a> {
    pub(crate) fn new(
        pid: Id, poll: &'a Poll, map: &'a mut HashMap<Eid, PollInfo>,
        eid: Eid, ready: Ready,
    ) -> Self {
        Self { base: CloseControl::new(pid, poll, map), eid, ready }
    }
    pub fn id(&self) -> Eid {
        self.eid
    }
    pub fn readiness(&self) -> Ready {
        self.ready
    }
}

impl<'a> Deref for EventControl<'a> {
    type Target = CloseControl<'a>;
    fn deref(&self) -> &CloseControl<'a> {
        &self.base
    }
}
impl<'a> DerefMut for EventControl<'a> {
    fn deref_mut(&mut self) -> &mut CloseControl<'a> {
        &mut self.base
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
