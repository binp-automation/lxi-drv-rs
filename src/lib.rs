extern crate mio;

use std::mem;
use std::collections::{BTreeMap};
use std::thread::{self};
use std::sync::{mpsc, Mutex};
use std::net::{IpAddr, TcpStream};

#[derive(Debug)]
pub enum Error {
    Other(String)
}

#[derive(Debug)]
pub enum Addr {
    Dns(String),
    Ip(IpAddr, u16),
}

#[derive(Debug)]
pub enum DevMsg {
    DnsResolve(Result<(), Error>),
    Connect(Result<(), Error>),
    Disconnect(),
    Data(Result<Vec<u8>, Error>),
}

pub struct Device {
    addr: Addr,
    chan: Option<mpsc::Receiver<DevMsg>>,
}

pub type DevId = u32;

#[derive(Debug)]
enum LoopCmd {
    Term,
    Add(DevId, mpsc::Receiver<DevMsg>),
    Del(DevId),
}

pub struct Driver {
    evtthr: Option<thread::JoinHandle<()>>,
    evtchan: mpsc::Sender<LoopCmd>,
    devs: Mutex<BTreeMap<DevId, Device>>,
}

impl Driver {
    fn event_thread_main(cmds: mpsc::Receiver<LoopCmd>) {

    }

    pub fn new() -> Result<Self, Error> {
        let (send_chan, recv_chan) = mpsc::channel();
        let thr = thread::spawn(move || {
            Self::event_thread_main(recv_chan)
        });
        Ok(Driver {
            evtthr: Some(thr),
            evtchan: send_chan,
            devs: Mutex::new(BTreeMap::new()),
        })
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.evtchan.send(LoopCmd::Term).unwrap();
        let thr = mem::replace(&mut self.evtthr, None);
        thr.unwrap().join().unwrap();
    }
}
