use std::mem;
use std::thread::{self, JoinHandle};

use mio_extras::channel::{self as mio_channel, Sender, Receiver};

use ::channel;
use ::device::*;
use ::event_loop::*;

#[derive(Debug)]
pub enum Error {
    Chan(channel::Error),
    Send(mio_channel::SendError<DrvCmd>),
}

pub enum DrvCmd {
    Attach(Device, Sender<DevRx>, Receiver<DevTx>),
    Terminate,
}

pub struct Driver {
    thr: Option<JoinHandle<()>>,
    tx: Sender<DrvCmd>,
}

impl Driver {
    pub fn new() -> Result<Self, Error> {
        let (tx, rx) = mio_channel::channel();
        let thr = thread::spawn(move || {
            EventLoop::new(rx).unwrap().run();
        });

        Ok(Driver {
            thr: Some(thr),
            tx: tx,
        })
    }

    pub fn attach(&mut self, dev: Device) -> Result<DeviceHandle, Error> {
        let (dtx, hrx) = mio_channel::channel();
        let (htx, drx) = mio_channel::channel();
        match self.tx.send(DrvCmd::Attach(dev, dtx, drx)) {
            Ok(_) => Ok(DeviceHandle { tx: htx, rx: hrx }),
            Err(err) => Err(Error::Send(err)),
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.tx.send(DrvCmd::Terminate);
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
            chan: IoChan::new_pair().0,
        }
    }

    #[test]
    fn add() {
        let mut drv = Driver::new().unwrap();
        let dev = dummy_device();
        drv.add(dev).unwrap();
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

