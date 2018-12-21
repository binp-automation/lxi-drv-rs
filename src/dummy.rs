use std::collections::{VecDeque};

use ::channel::{SinglePoll};
use ::proxy::{self, Proxy, Control, Eid};
use ::proxy_handle::{self, ProxyWrapper, Handle, UserProxy, UserHandle};

pub use proxy_handle::{Tx, Rx};

pub struct DummyProxy {}

impl DummyProxy {
    fn new() -> Self {
        Self {}
    }
}

impl Proxy for DummyProxy {
    fn attach(&mut self, _ctrl: &Control) -> ::Result<()> {
        Ok(())
    }

    fn detach(&mut self, _ctrl: &Control) -> ::Result<()> {
        Ok(())
    }

    fn process(&mut self, _ctrl: &mut Control, _readiness: mio::Ready, _eid: Eid) -> ::Result<()> {
        Ok(())
    }
}

impl UserProxy<Tx, Rx> for DummyProxy {
    fn process_channel(&mut self, _ctrl: &mut Control, _msg: Tx) -> ::Result<()> {
        Ok(())
    }
}

pub struct DummyHandle {
    pub msgs: VecDeque<Rx>,
}

impl DummyHandle {
    fn new() -> Self {
        Self {
            msgs: VecDeque::new(),
        }
    }
}

impl UserHandle<Tx, Rx> for DummyHandle {
    fn process_channel(&mut self, msg: Rx) -> ::Result<()> {
        self.msgs.push_back(msg);
        Ok(())
    }
}

pub fn create() -> ::Result<(ProxyWrapper<DummyProxy, Tx, Rx>, Handle<DummyHandle, Tx, Rx>)> {
    proxy_handle::create(DummyProxy::new(), DummyHandle::new())
}

pub fn wait_msgs(h: &mut Handle<DummyHandle, Tx, Rx>, sp: &mut SinglePoll, n: usize) -> ::Result<()> {
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

pub fn wait_close(h: &mut Handle<DummyHandle, Tx, Rx>, sp: &mut SinglePoll) -> ::Result<()> {
    loop {
        if let Err(e) = sp.wait(None) {
            break Err(::Error::Channel(e.into()));
        }
        match h.process() {
            Ok(()) => continue,
            Err(err) => match err {
                ::Error::Proxy(proxy::Error::Closed) => break Ok(()),
                other => break Err(other),
            }
        }
    }
}