use std::net::{IpAddr};

use ::*;


pub type DevId = u32;

#[derive(Debug)]
pub enum DevTx {
    Data(Vec<u8>),
}

#[derive(Debug)]
pub enum DevRx {
    DnsResolved(Result<IpAddr, Error>),
    Connected(Result<(), Error>),
    Disconnected,
    Data(Result<Vec<u8>, Error>)
}

pub struct Device {
    pub addr: Addr,
    pub chan: IoChan<DevRx, DevTx>,
}

