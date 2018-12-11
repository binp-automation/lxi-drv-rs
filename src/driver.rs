#[cfg(test)]
#[path = "./tests/driver.rs"]
mod tests;


use std::mem;
use std::thread::{self, JoinHandle};

use ::channel::*;
use ::device::*;
use ::event_loop::*;


#[derive(Debug)]
pub enum DrvError {
    Chan(ChanError),
}

pub enum DrvCmd {
    Attach(Device, (Sender<DevRx>, Receiver<DevTx>)),
    Terminate,
}

pub struct Driver {
    thr: Option<JoinHandle<()>>,
    tx: Sender<DrvCmd>,
}

impl Driver {
    pub fn new() -> Result<Self, DrvError> {
        let (tx, rx) = channel();
        let thr = thread::spawn(move || {
            EventLoop::new(rx).unwrap().run_forever(1024);
        });

        Ok(Driver {
            thr: Some(thr),
            tx: tx,
        })
    }

    pub fn attach(&mut self, dev: Device) -> Result<DevHandle, DrvError> {
        let (dtx, hrx) = channel();
        let (htx, drx) = channel();
        match self.tx.send(DrvCmd::Attach(dev, (dtx, drx))) {
            Ok(_) => Ok(DevHandle::new(htx, hrx)),
            Err(err) => Err(DrvError::Chan(err.into())),
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.tx.send(DrvCmd::Terminate).unwrap();
        let thr = mem::replace(&mut self.thr, None).unwrap();
        thr.join().unwrap();
    }
}
