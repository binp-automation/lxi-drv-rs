use std::mem;
use std::thread::{self, JoinHandle};

use ::channel::{*, Error as ChanError};
use ::device::*;
use ::event_loop::*;

#[derive(Debug)]
pub enum Error {
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
    pub fn new() -> Result<Self, Error> {
        let (tx, rx) = channel();
        let thr = thread::spawn(move || {
            EventLoop::new(rx).unwrap().run();
        });

        Ok(Driver {
            thr: Some(thr),
            tx: tx,
        })
    }

    pub fn attach(&mut self, dev: Device) -> Result<DevHandle, Error> {
        let (dtx, hrx) = channel();
        let (htx, drx) = channel();
        match self.tx.send(DrvCmd::Attach(dev, (dtx, drx))) {
            Ok(_) => Ok(DevHandle::new(htx, hrx)),
            Err(err) => Err(Error::Chan(err.into())),
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

/*
#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_device() -> Device {
        Device {
            addr: Addr::Dns(String::from("localhost"), 8000),
        }
    }
    #[test]
    fn add_remove() {
        let mut drv = Driver::new().unwrap();
        let dev = dummy_device();
        let dh = drv.attach(dev).unwrap();
        dh.detach().unwrap();
    }

    #[test]
    fn add_remove() {
        let mut drv = Driver::new().unwrap();
        let dev = dummy_device();
        let devid = drv.add(dev).unwrap();
        assert!(drv.remove(devid).is_ok());
    }

    #[test]
    fn remove_empty() {
        let mut drv = Driver::new().unwrap();
        assert!(drv.remove(0).is_err());
    }

    #[test]
    fn remove_twice() {
        let mut drv = Driver::new().unwrap();
        let dev = dummy_device();
        let devid = drv.add(dev).unwrap();
        assert!(drv.remove(devid).is_ok());
        assert!(drv.remove(devid).is_err());
    }

    #[test]
    fn add_remove_many() {
        let mut drv = Driver::new().unwrap();
        let devs = (0..16).map(|_| dummy_device());
        let mut ids = Vec::new();
        for dev in devs {
            ids.push(drv.add(dev).unwrap());
        }
        for id in ids {
            drv.remove(id).unwrap();
        }
    }
}
*/
