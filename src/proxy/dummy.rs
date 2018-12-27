use std::collections::{VecDeque};

use ::channel::{Sender, SinglePoll};

use super::control::{AttachControl, DetachControl, EventControl};
use super::proxy::{self as p};
use super::wrapper::{self as w};
use super::user::{self as u};

pub use self::w::{Tx, Rx};

pub struct Proxy {}

impl Proxy {
    fn new() -> Self {
        Self {}
    }
}

impl p::Proxy for Proxy {
    fn attach(&mut self, _ctrl: &mut AttachControl) -> ::Result<()> {
        Ok(())
    }

    fn detach(&mut self, _ctrl: &mut DetachControl) -> ::Result<()> {
        Ok(())
    }

    fn process(&mut self, _ctrl: &mut EventControl) -> ::Result<()> {
        Ok(())
    }
}

impl u::Proxy<Tx, Rx> for Proxy {
    fn set_send_channel(&mut self, _tx: Sender<Rx>) {

    }

    fn process_recv_channel(&mut self, _ctrl: &mut EventControl, _msg: Tx) -> ::Result<()> {
        Ok(())
    }
}

pub struct Handle {
    pub msgs: VecDeque<Rx>,
}

impl Handle {
    fn new() -> Self {
        Self {
            msgs: VecDeque::new(),
        }
    }
}

impl u::Handle<Tx, Rx> for Handle {
    fn set_send_channel(&mut self, _tx: Sender<Tx>) {}
    fn process_recv_channel(&mut self, msg: Rx) -> ::Result<()> {
        self.msgs.push_back(msg);
        Ok(())
    }
}

pub fn create() -> ::Result<(w::Proxy<Proxy, Tx, Rx>, w::Handle<Handle, Tx, Rx>)> {
    w::create(Proxy::new(), Handle::new())
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
                ::Error::Proxy(p::Error::Closed) => break Ok(()),
                other => break Err(other),
            }
        }
    }
}
