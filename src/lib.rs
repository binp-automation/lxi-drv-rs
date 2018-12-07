pub mod device;
pub mod driver;
mod event_loop;

extern crate mio;
extern crate mio_extras;
extern crate threadpool;

use std::net::IpAddr;
use std::fmt::{self, Debug, Formatter};
use mio_extras::channel as mio_mpsc;


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
    pub tx: mio_mpsc::Sender<Tx>,
    pub rx: mio_mpsc::Receiver<Rx>,
}

impl<Tx, Rx> Debug for IoChan<Tx, Rx> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "IoChan")
    }
}

impl<Tx, Rx> IoChan<Tx, Rx> {
    pub fn new_pair() -> (IoChan<Tx, Rx>, IoChan<Rx, Tx>) {
        let (tx0, rx1) = mio_mpsc::channel();
        let (tx1, rx0) = mio_mpsc::channel();
        let fc = IoChan { tx: tx0, rx: rx0};
        let bc = IoChan { tx: tx1, rx: rx1};
        (fc, bc)
    }
}
