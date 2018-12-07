use std::mem;
use std::thread::{self};
use std::sync::{Mutex};

use mio::*;

use ::*;
use ::device::*;
use ::event_loop::{EventLoop, EvtTx, EvtRx};


pub struct Driver {
    thr: Option<thread::JoinHandle<()>>,
    chan: Mutex<IoChan<EvtTx, EvtRx>>,
    poll: Poll,
    evts: Events,
}

impl Driver {
    pub fn new() -> Result<Self, Error> {
        let (send_chan, recv_chan) = IoChan::new_pair();
        let thr = thread::spawn(move || {
            EventLoop::new(recv_chan).run();
        });
        
        let poll = Poll::new().unwrap();
        poll.register(&send_chan.rx, Token(0), Ready::readable(), PollOpt::edge()).unwrap();
        let evts = Events::with_capacity(16);

        Ok(Driver {
            thr: Some(thr),
            chan: Mutex::new(send_chan),
            poll, evts,
        })
    }

    pub fn request(&mut self, evt: EvtTx) -> Result<EvtRx, Error> {
        let guard = self.chan.lock().unwrap();
        guard.tx.send(evt).unwrap();
        loop {
            self.poll.poll(&mut self.evts, None).unwrap();
            if !self.evts.is_empty() {
                let mut it = self.evts.iter();
                let res = it.next().unwrap();
                assert!(it.next().is_none());
                assert!(res.token() == Token(0) && res.readiness().is_readable());
                match guard.rx.try_recv() {
                    Ok(revt) => break Ok(revt),
                    Err(err) => panic!("{:?}", err),
                }
            }
        }
    }

    pub fn add(&mut self, dev: Device) -> Result<DevId, Error> {
        match self.request(EvtTx::Add(dev)).unwrap() {
            EvtRx::Added(res) => res,
            _ => unreachable!(),
        }
    }

    pub fn remove(&mut self, id: DevId) -> Result<(), Error> {
        match self.request(EvtTx::Remove(id)).unwrap() {
            EvtRx::Removed(res) => res,
            _ => unreachable!(),
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.chan.lock().unwrap().tx.send(EvtTx::Term).unwrap();
        let thr = mem::replace(&mut self.thr, None).unwrap();
        thr.join().unwrap();
    }
}

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


