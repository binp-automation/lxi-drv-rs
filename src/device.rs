use std::net::{IpAddr};
use std::fmt::{self, Debug, Formatter};

use mio_extras::channel as mio_channel;

use ::channel::{self, PollReceiver};


#[derive(Debug)]
pub enum Addr {
    Dns(String, u16),
    Ip(IpAddr, u16),
}

#[derive(Debug)]
pub enum Error {
    Chan(channel::Error),
    Send(mio_channel::SendError<DevTx>),
}

#[derive(Debug)]
pub enum DevTx {
    Send(Vec<u8>),
    Detach,
}

#[derive(Debug)]
pub enum DevRx {
    Received(Result<Vec<u8>, Error>),
    
    DnsResolved(Result<IpAddr, Error>),
    Connected(Result<(), Error>),
    Disconnected,
    
    Attached,
    Detached(Device),
}

#[derive(Debug)]
pub struct Device {
    pub addr: Addr,
}

impl Device {
    pub fn new(addr: Addr) -> Device {
        Device { addr }
    }
}

pub struct DeviceHandle {
    pub tx: mio_channel::Sender<DevTx>,
    pub rx: mio_channel::Receiver<DevRx>,
}

impl Debug for DeviceHandle {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "DeviceHandle")
    }
}

impl DeviceHandle {
    pub fn detach(mut self) -> Result<Device, Error> {
        self.tx.send(DevTx::Detach).map_err(|e| Error::Send(e))?;
        let prx = PollReceiver::new(self.rx).map_err(|e| Error::Chan(e))?;
        loop {
            match prx.recv() {
                Ok(msg) => match msg {
                    DevRx::Detached(dev) => break Ok(dev),
                    _ => continue,
                },
                Err(err) => break Err(Error::Chan(err)),
            }
        }
    }
}

impl Drop for DeviceHandle {
    fn drop(&mut self) {
        self.detach();
    }
}
