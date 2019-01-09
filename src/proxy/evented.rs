use std::io;
use std::ops::{Deref, DerefMut};
use std::cell::{Cell};

use mio::{self, Poll, Token, Ready, PollOpt};

use super::id::*;


pub trait Evented: mio::Evented {
    fn id(&self) -> Eid;
}

pub struct EventedWrapper<E: mio::Evented> {
    handle: E,
    eid: Eid,
    reg: Cell<bool>,
}

impl<E: mio::Evented> EventedWrapper<E> {
    pub fn new(handle: E, eid: Eid) -> Self {
        Self { handle, eid, reg: Cell::new(false) }
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
        if self.reg.get() {
            return Err(io::Error::new(io::ErrorKind::Other, "Handle already registered"));
        }
        self.handle.register(poll, token, ready, poll_opt).and_then(|_| {
            self.reg.set(true);
            Ok(())
        })
    }
    fn reregister(&self, poll: &Poll, token: Token, ready: Ready, poll_opt: PollOpt) -> io::Result<()> {
        if !self.reg.get() {
            return Err(io::Error::new(io::ErrorKind::Other, "Handle isn't registered"));
        }
        self.handle.reregister(poll, token, ready, poll_opt)
    }
    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        if !self.reg.get() {
            return Err(io::Error::new(io::ErrorKind::Other, "Handle isn't registered"));
        }
        self.handle.deregister(poll).and_then(|_| {
            self.reg.set(false);
            Ok(())
        })
    }
}

impl<E: mio::Evented> Evented for EventedWrapper<E> {
    fn id(&self) -> Eid {
        self.eid
    }
}

impl<E: mio::Evented> Drop for EventedWrapper<E> {
    fn drop(&mut self) {
        if self.reg.get() {
            panic!("Handle dropped without being deregistered");
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct DummyHandle {}
    impl mio::Evented for DummyHandle {
        fn register(&self, _poll: &Poll, _token: Token, _ready: Ready, _poll_opt: PollOpt) -> io::Result<()> {
            Ok(())
        }
        fn reregister(&self, _poll: &Poll, _token: Token, _ready: Ready, _poll_opt: PollOpt) -> io::Result<()> {
            Ok(())
        }
        fn deregister(&self, _poll: &Poll) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    #[should_panic(expected = "Handle dropped without being deregistered")]
    fn drop() {
        let poll = Poll::new().unwrap();
        let handle = EventedWrapper::new(DummyHandle{}, 0);

        poll.register(&handle, Token(handle.id()), Ready::readable(), PollOpt::edge()).unwrap();
        (move || {
            let _ = handle;
        })();
    }

    #[test]
    fn reg_twice() {
        let poll = Poll::new().unwrap();
        let handle = EventedWrapper::new(DummyHandle{}, 0);

        poll.register(&handle, Token(handle.id()), Ready::readable(), PollOpt::edge()).unwrap();
        match poll.register(&handle, Token(handle.id()), Ready::readable(), PollOpt::edge()) {
            Ok(_) => panic!(),
            Err(e) => {
                assert_eq!(e.kind(), io::ErrorKind::Other);
                assert_eq!(e.get_ref().unwrap().description(), "Handle already registered");
            }
        }

        poll.deregister(&handle).unwrap();
    }

    #[test]
    fn rereg() {
        let poll = Poll::new().unwrap();
        let handle = EventedWrapper::new(DummyHandle{}, 0);

        match poll.reregister(&handle, Token(handle.id()), Ready::readable(), PollOpt::edge()) {
            Ok(_) => panic!(),
            Err(e) => {
                assert_eq!(e.kind(), io::ErrorKind::Other);
                assert_eq!(e.get_ref().unwrap().description(), "Handle isn't registered");
            }
        }
    }

    #[test]
    fn dereg() {
        let poll = Poll::new().unwrap();
        let handle = EventedWrapper::new(DummyHandle{}, 0);

        match poll.deregister(&handle) {
            Ok(_) => panic!(),
            Err(e) => {
                assert_eq!(e.kind(), io::ErrorKind::Other);
                assert_eq!(e.get_ref().unwrap().description(), "Handle isn't registered");
            }
        }
    }
}
