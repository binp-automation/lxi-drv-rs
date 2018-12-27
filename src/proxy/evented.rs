use std::io;
use std::ops::{Deref, DerefMut};

use mio::{self, Poll, Token, Ready, PollOpt};

use super::id::*;


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
