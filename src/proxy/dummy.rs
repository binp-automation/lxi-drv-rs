use std::collections::{VecDeque};

use ::channel::{Sender, SinglePoll};

use super::control;
use super::proxy::{self as p};
use super::wrapper::{self as w};
use super::user::{self as u};

pub use self::w::{Tx, Rx};

pub struct Proxy {}

impl Proxy {
    fn new(_tx: Sender<Rx>) -> Self {
        Self {}
    }
}

impl p::Proxy for Proxy {
    fn attach(&mut self, _ctrl: &mut control::Attach) -> ::Result<()> {
        Ok(())
    }

    fn detach(&mut self, _ctrl: &mut control::Detach) -> ::Result<()> {
        Ok(())
    }

    fn process(&mut self, _ctrl: &mut control::Process) -> ::Result<()> {
        Ok(())
    }

    fn commit(&mut self, _ctrl: &mut control::Process) -> ::Result<()> {
        Ok(())
    }
}

impl u::Proxy<Tx, Rx> for Proxy {
    fn process_recv_channel(&mut self, _ctrl: &mut control::Process, _msg: Tx) -> ::Result<()> {
        Ok(())
    }
}

pub struct Handle {
    pub msgs: VecDeque<Rx>,
}

impl Handle {
    fn new(_tx: Sender<Tx>) -> Self {
        Self {
            msgs: VecDeque::new(),
        }
    }
}

impl u::Handle<Tx, Rx> for Handle {
    fn process_recv_channel(&mut self, msg: Rx) -> ::Result<()> {
        self.msgs.push_back(msg);
        Ok(())
    }
}

pub fn create() -> ::Result<(w::Proxy<Proxy, Tx, Rx>, w::Handle<Handle, Tx, Rx>)> {
    w::create(|tx| Proxy::new(tx), |tx| Handle::new(tx))
}

pub fn wait_msgs(h: &mut w::Handle<Handle, Tx, Rx>, sp: &mut SinglePoll, n: usize) -> ::Result<()> {
    let ns = h.user.msgs.len();
    loop {
        if let Err(e) = sp.wait(None) {
            break Err(::Error::Channel(e.into()));
        }
        if let Err(e) = h.process() {
            break Err(e);
        }
        if h.user.msgs.len() - ns >= n {
            break Ok(());
        }
    }
}

pub fn wait_close(h: &mut w::Handle<Handle, Tx, Rx>, sp: &mut SinglePoll) -> ::Result<()> {
    loop {
        if let Err(e) = sp.wait(None) {
            break Err(::Error::Channel(e.into()));
        }
        match h.process() {
            Ok(()) => continue,
            Err(err) => match err {
                ::Error::Proxy(super::Error::Closed) => break Ok(()),
                other => break Err(other),
            }
        }
    }
}
