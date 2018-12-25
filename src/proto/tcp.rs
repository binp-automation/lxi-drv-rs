use std::collections::{VecDeque};

use ::channel::{Sender};
use ::proxy::{self, Control, Eid};
use ::wrapper::{self, UserProxy, UserHandle};


pub enum Tx {

}

pub enum Rx {

}

pub struct Proxy {
    tx: Option<Sender<Rx>>,
}

impl Proxy {
    fn new() -> Self {
        Self { tx: None }
    }
}

impl proxy::Proxy for Proxy {
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

impl UserProxy<Tx, Rx> for Proxy {
    fn set_send_channel(&mut self, tx: Sender<Rx>) {
        self.tx = Some(tx);
    }
    fn process_recv_channel(&mut self, _ctrl: &mut Control, _msg: Tx) -> ::Result<()> {
        Ok(())
    }
}

pub struct Handle {
    tx: Option<Sender<Tx>>,
}

impl Handle {
    fn new() -> Self {
        Self { tx: None }
    }
}

impl UserHandle<Tx, Rx> for Handle {
    fn set_send_channel(&mut self, tx: Sender<Tx>) {
        self.tx = Some(tx);
    }
    fn process_recv_channel(&mut self, msg: Rx) -> ::Result<()> {
        self.msgs.push_back(msg);
        Ok(())
    }
}

pub fn create() -> ::Result<(ProxyWrapper<Proxy, Tx, Rx>, HandleWrapper<Handle, Tx, Rx>)> {
    proxy_handle::create(Proxy::new(), Handle::new())
}
