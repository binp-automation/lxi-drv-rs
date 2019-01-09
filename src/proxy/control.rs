use std::ops::{Deref, DerefMut};
use std::collections::{HashMap};

use mio::{Poll, Ready, PollOpt};

use ::error::{IdError};

use super::id::{Id, Eid, encode_ids};
use super::evented::{Evented};


pub struct PollInfo {
    pub ready: Ready,
    pub fresh: bool,
}

pub struct Base<'a> {
    pid: Id,
    poll: &'a Poll,
    map: &'a mut HashMap<Eid, PollInfo>,
}

impl<'a> Base<'a> {
    pub(crate) fn new(pid: Id, poll: &'a Poll, map: &'a mut HashMap<Eid, PollInfo>) -> Self {
        Self { pid, poll, map }
    }
}

impl<'a> Base<'a> {
    pub fn proxy_id(&self) -> Id {
        self.pid
    }

    pub fn register(&mut self, handle: &Evented, interest: Ready) -> ::Result<()> {
        encode_ids(self.pid, handle.id()).ok_or(IdError::Bad.into()).and_then(|token| {
            self.poll.register(
                handle, token, interest, 
                PollOpt::edge(),
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
                PollOpt::edge(),
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

pub struct Close<'a> {
    base: Base<'a>,
    closed: bool,
}

impl<'a> Close<'a> {
    pub(crate) fn new(pid: Id, poll: &'a Poll, map: &'a mut HashMap<Eid, PollInfo>) -> Self {
        Self { base: Base::new(pid, poll, map), closed: false }
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl<'a> Deref for Close<'a> {
    type Target = Base<'a>;
    fn deref(&self) -> &Base<'a> {
        &self.base
    }
}
impl<'a> DerefMut for Close<'a> {
    fn deref_mut(&mut self) -> &mut Base<'a> {
        &mut self.base
    }
}

pub type Attach<'a> = Close<'a>;
pub type Detach<'a> = Base<'a>;

pub struct Process<'a> {
    base: Close<'a>,
    eid: Eid,
    ready: Ready,
}

impl<'a> Process<'a> {
    pub(crate) fn new(
        pid: Id, poll: &'a Poll, map: &'a mut HashMap<Eid, PollInfo>,
        eid: Eid, ready: Ready,
    ) -> Self {
        Self { base: Close::new(pid, poll, map), eid, ready }
    }
    pub fn id(&self) -> Eid {
        self.eid
    }
    pub fn readiness(&self) -> Ready {
        self.ready
    }
}

impl<'a> Deref for Process<'a> {
    type Target = Close<'a>;
    fn deref(&self) -> &Close<'a> {
        &self.base
    }
}
impl<'a> DerefMut for Process<'a> {
    fn deref_mut(&mut self) -> &mut Close<'a> {
        &mut self.base
    }
}
