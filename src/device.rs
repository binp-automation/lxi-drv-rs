#[cfg(test)]
#[path = "./tests/device.rs"]
mod tests;


use std::mem;
use std::net::{IpAddr};
use std::fmt::{self, Debug, Formatter};

use ::channel::*;


#[derive(Debug, PartialEq)]
pub enum Addr {
    Dns(String, u16),
    Ip(IpAddr, u16),
}

#[derive(Debug)]
pub enum DevError {
    Chan(ChanError),
    Detached,
}

#[derive(Debug)]
pub enum DevTx {
    Send(Vec<u8>),
    Detach,
}

#[derive(Debug)]
pub enum RxError {}

#[derive(Debug)]
pub enum DevRx {
    Received(Result<Vec<u8>, RxError>),
    
    DnsResolved(Result<IpAddr, RxError>),
    Connected(Result<(), RxError>),
    Disconnected,
    
    Attached,
    Detached(DevProxy),
}

#[derive(Debug)]
pub struct DevProxy {
    pub addr: Addr,
}

impl DevProxy {
    pub fn new(addr: Addr) -> DevProxy {
        DevProxy { addr }
    }
}

pub struct DhCnx {
    pub tx: Sender<DevTx>,
    pub rx: Receiver<DevRx>,
}

impl Debug for DevHandle {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "DhCnx")
    }
}

pub struct DevHandle {
    cnx_opt: Option<DhCnx>,
}


impl DevHandle {
    pub fn new(tx: Sender<DevTx>, rx: Receiver<DevRx>) -> Self {
        DevHandle { cnx_opt: Some(DhCnx { tx, rx }) }
    }
    pub fn cnx(&self) -> Result<&DhCnx, DevError> {
        match &self.cnx_opt {
            Some(cnx) => Ok(&cnx),
            None => Err(DevError::Detached),
        }
    }
    pub fn tx(&self) -> Result<&Sender<DevTx>, DevError> {
        self.cnx().map(|c| &c.tx)
    }
    pub fn rx(&self) -> Result<&Receiver<DevRx>, DevError> {
        self.cnx().map(|c| &c.rx)
    }
    pub fn detach_ref(&mut self) -> Result<DevProxy, DevError> {
        let cnx = mem::replace(&mut self.cnx_opt, None).ok_or(DevError::Detached)?;

        let dev_opt: Option<DevProxy> = loop {
            match cnx.rx.try_recv() {
                Ok(msg) => match msg {
                    DevRx::Detached(dev) => break Ok(Some(dev)),
                    _ => continue,
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(None),
                    TryRecvError::Disconnected => break Err(DevError::Chan(ChanError::Disconnected)),
                },
            };
        }?;


        if let Some(dev) = dev_opt {
            return Ok(dev);
        }
        
        cnx.tx.send(DevTx::Detach).map_err(|e| DevError::Chan(e.into()))?;

        let mut prx = PollReceiver::new(&cnx.rx).map_err(|e| DevError::Chan(e))?;
        loop {
            match prx.recv() {
                Ok(msg) => match msg {
                    DevRx::Detached(dev) => break Ok(dev),
                    _ => continue,
                },
                Err(err) => break Err(DevError::Chan(err.into())),
            }
        }
    }

    pub fn detach(mut self) -> Result<DevProxy, DevError> {
        self.detach_ref()
    }
}

impl Drop for DevHandle {
    fn drop(&mut self) {
        match self.detach_ref() {
            Ok(_) => (),
            Err(err) => match err {
                DevError::Detached => (),
                other => panic!("{:?}", other),
            },
        }
    }
}
