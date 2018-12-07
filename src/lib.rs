pub mod device;
pub mod driver;
mod event_loop;

extern crate mio;
extern crate mio_extras;

use mio_extras::channel as mpsc;
use std::net::IpAddr;


#[derive(Debug)]
pub enum Error {
    Other(String)
}

#[derive(Debug)]
pub enum Addr {
    Dns(String, u16),
    Ip(IpAddr, u16),
}

pub struct IoChan<Tx, Rx> {
    pub tx: mpsc::Sender<Tx>,
    pub rx: mpsc::Receiver<Rx>,
}

impl<Tx, Rx> IoChan<Tx, Rx> {
    pub fn new_pair() -> (IoChan<Tx, Rx>, IoChan<Rx, Tx>) {
        let (tx0, rx1) = mpsc::channel();
        let (tx1, rx0) = mpsc::channel();
        let fc = IoChan { tx: tx0, rx: rx0};
        let bc = IoChan { tx: tx1, rx: rx1};
        (fc, bc)
    }
}
