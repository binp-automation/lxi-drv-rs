use std::mem;
use std::thread::{self};
use std::sync::{Mutex};

use ::*;
use ::device::*;
use event_loop::{EventLoop, EvtTx, EvtRx};


pub struct Driver {
    evtthr: Option<thread::JoinHandle<()>>,
    evtchan: Mutex<IoChan<EvtTx, EvtRx>>,
}

impl Driver {
    pub fn new() -> Result<Self, Error> {
        let (send_chan, recv_chan) = IoChan::new_pair();
        let thr = thread::spawn(move || {
            EventLoop::new(recv_chan).run();
        });
        Ok(Driver {
            evtthr: Some(thr),
            evtchan: Mutex::new(send_chan),
        })
    }

    pub fn add(&mut self, dev: Device) -> Result<DevId, Error> {
        let guard = self.evtchan.lock().unwrap();
        guard.tx.send(EvtTx::Add(dev)).unwrap();
        match guard.rx.recv().unwrap() {
            EvtRx::Added(res) => res,
            _ => unreachable!(),
        }
    }

    pub fn del(&mut self, id: DevId) -> Result<(), Error> {
        let guard = self.evtchan.lock().unwrap();
        guard.tx.send(EvtTx::Remove(id)).unwrap();
        match guard.rx.recv().unwrap() {
            EvtRx::Removed(res) => res,
            _ => unreachable!()
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.evtchan.lock().unwrap().tx.send(EvtTx::Term).unwrap();
        let thr = mem::replace(&mut self.evtthr, None).unwrap();
        thr.join().unwrap();
    }
}
