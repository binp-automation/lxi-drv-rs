extern crate mio;

use std::mem;
use std::io::{Read, Write};
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
    Ip(IpAddr),
}

#[derive(Debug)]
pub enum DevSend {
    Data(Vec<u8>),
}

#[derive(Debug)]
pub enum DevRecv {
    DnsResolved(Result<(), Error>),
    Connected(Result<(), Error>),
    Disconnected,
    Data(Result<Vec<u8>, Error>)
}

#[derive(Debug)]
pub struct IoChan<Tx, Rx> {
    send: mpsc::Sender<Tx>,
    recv: mpsc::Receiver<Rx>,
}

impl<Tx, Rx> IoChan<Tx, Rx> {
    fn new_pair() -> (IoChan<Tx, Rx>, IoChan<Rx, Tx>) {
        let (tx0, rx1) = mpsc::channel();
        let (tx1, rx0) = mpsc::channel();
        let forw = IoChan { send: tx0, recv: rx0};
        let back = IoChan { send: tx1, recv: rx1};
        (forw, back)
    }
}


pub trait Device {
    fn recv(self: &mut Self, &mut Read);
}

pub type DevId = u32;

enum EvtSend {
    Term,
    Add(DevId, Box<Device>),
    Del(DevId),
}

enum EvtRecv {}

pub struct Driver {
    evtthr: Option<thread::JoinHandle<()>>,
    evtchan: IoChan<EvtSend, EvtRecv>,
    devid_cnt: DevId,
}

impl Driver {
    fn event_thread_main(chan: IoChan<EvtRecv, EvtSend>) {

    }

    pub fn new() -> Result<Self, Error> {
        let (send_chan, recv_chan) = IoChan::new_pair();
        let thr = thread::spawn(move || {
            Self::event_thread_main(recv_chan)
        });
        Ok(Driver {
            evtthr: Some(thr),
            evtchan: send_chan,
            devs: Mutex::new(BTreeMap::new()),
            devid_cnt: 1,
        })
    }

    pub fn add()
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.evtchan.send.send(EvtSend::Term).unwrap();
        let thr = mem::replace(&mut self.evtthr, None).unwrap();
        thr.join().unwrap();
    }
}
