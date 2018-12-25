use std::collections::{VecDeque};

use ::channel::{Sender};
use ::proxy::{self, Control, Eid};
use ::wrapper::{self as w, UserProxy, UserHandle};


pub enum Tx {
    // Base
    Close,
}

impl From<w::Tx> for Tx {
    fn from(other: w::Tx) -> Tx {
        match other {
            w::Tx::Close => Tx::Close,
        }
    } 
}

impl Into<Result<w::Tx, Tx>> for Tx {
    fn into(self) -> Result<w::Tx, Tx> {
        match self {
            Tx::Close => Ok(w::Tx::Close),
            other => Err(other),
        }
    }
}

impl w::TxExt for Tx {}

pub enum Rx {
    // Base
    Attached,
    Detached,
    Closed,
}

impl From<w::Rx> for Rx {
    fn from(other: w::Rx) -> Rx {
        match other {
            w::Rx::Attached => Rx::Attached,
            w::Rx::Detached => Rx::Detached,
            w::Rx::Closed => Rx::Closed,
        }
    } 
}

impl Into<Result<w::Rx, Rx>> for Rx {
    fn into(self) -> Result<w::Rx, Rx> {
        match self {
            Rx::Attached => Ok(w::Rx::Attached),
            Rx::Detached => Ok(w::Rx::Detached),
            Rx::Closed => Ok(w::Rx::Closed),
            other => Err(other),
        }
    }
}

impl w::RxExt for Rx {}


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
    fn process_recv_channel(&mut self, _msg: Rx) -> ::Result<()> {
        Ok(())
    }
}

pub fn create() -> ::Result<(w::Proxy<Proxy, Tx, Rx>, w::Handle<Handle, Tx, Rx>)> {
    w::create(Proxy::new(), Handle::new())
}


#[cfg(test)]
mod test {
    use super::*;
    
}
