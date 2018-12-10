use std::mem;
use std::net::{IpAddr};
use std::fmt::{self, Debug, Formatter};

use ::channel::{Sender, Receiver, PollReceiver, Error as ChanError, TryRecvError};


#[derive(Debug, PartialEq)]
pub enum Addr {
    Dns(String, u16),
    Ip(IpAddr, u16),
}

#[derive(Debug)]
pub enum Error {
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
    pub fn cnx(&self) -> Result<&DhCnx, Error> {
        match &self.cnx_opt {
            Some(cnx) => Ok(&cnx),
            None => Err(Error::Detached),
        }
    }
    pub fn tx(&self) -> Result<&Sender<DevTx>, Error> {
        self.cnx().map(|c| &c.tx)
    }
    pub fn rx(&self) -> Result<&Receiver<DevRx>, Error> {
        self.cnx().map(|c| &c.rx)
    }
    pub fn detach_ref(&mut self) -> Result<Device, Error> {
        let cnx = mem::replace(&mut self.cnx_opt, None).ok_or(Error::Detached)?;

        let dev_opt: Option<Device> = loop {
            match cnx.rx.try_recv() {
                Ok(msg) => match msg {
                    DevRx::Detached(dev) => break Ok(Some(dev)),
                    _ => continue,
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(None),
                    TryRecvError::Disconnected => break Err(Error::Chan(ChanError::Disconnected)),
                },
            };
        }?;


        if let Some(dev) = dev_opt {
            return Ok(dev);
        }
        
        cnx.tx.send(DevTx::Detach).map_err(|e| Error::Chan(e.into()))?;

        let mut prx = PollReceiver::new(&cnx.rx).map_err(|e| Error::Chan(ChanError::Io(e)))?;
        loop {
            match prx.recv() {
                Ok(msg) => match msg {
                    DevRx::Detached(dev) => break Ok(dev),
                    _ => continue,
                },
                Err(err) => break Err(Error::Chan(err.into())),
            }
        }
    }

    pub fn detach(mut self) -> Result<Device, Error> {
        self.detach_ref()
    }
}

impl Drop for DevHandle {
    fn drop(&mut self) {
        match self.detach_ref() {
            Ok(_) => (),
            Err(err) => match err {
                Error::Detached => (),
                other => panic!("{:?}", other),
            },
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use ::channel::{channel, PollReceiver, Error as ChanError, RecvError};

    use std::thread;
    use std::time::{Duration};
    use std::sync::{Arc, atomic::{Ordering, AtomicBool}};


    fn dummy_device() -> Device {
        Device {
            addr: Addr::Dns(String::from("localhost"), 8000),
        }
    }

    #[test]
    fn detach_after() {
        let (tx, rxh) = channel();
        let (txh, rx) = channel();
        let mut prx = PollReceiver::new(&rx).unwrap();

        let dh = DevHandle::new(txh, rxh);
        
        tx.send(DevRx::Detached(dummy_device())).unwrap();

        let dev = dh.detach().unwrap();
        assert_eq!(dev.addr, dummy_device().addr);

        if let Err(RecvError::Disconnected) = prx.recv() {
            // ok
        } else {
            panic!();
        }
    }

    #[test]
    fn detach_before() {
        let (tx, rxh) = channel();
        let (txh, rx) = channel();

        thread::spawn(move || {
            let mut prx = PollReceiver::new(&rx).unwrap();
            
            match prx.recv().unwrap() {
                DevTx::Detach => tx.send(DevRx::Detached(dummy_device())).unwrap(),
                x => panic!("{:?}", x),
            };
            match prx.recv() {
                Err(RecvError::Disconnected) => (),
                x => panic!("{:?}", x),
            };
        });
        thread::sleep(Duration::from_millis(10));

        let dh = DevHandle::new(txh, rxh);
        let dev = dh.detach().unwrap();

        assert_eq!(dev.addr, dummy_device().addr);
    }

    #[test]
    fn detach_txclose() {
        let rxh = channel().1;
        let (txh, _rx) = channel();

        if let Err(Error::Chan(ChanError::Disconnected)) = DevHandle::new(txh, rxh).detach() {
            // ok
        } else {
            panic!();
        }
    }

    #[test]
    fn detach_rxclose() {
        let (_tx, rxh) = channel();
        let txh = channel().0;

        if let Err(Error::Chan(ChanError::Disconnected)) = DevHandle::new(txh, rxh).detach() {
            // ok
        } else {
            panic!();
        }
    }

    #[test]
    fn drop() {
        let (tx, rxh) = channel();
        let (txh, rx) = channel();
        let mut prx = PollReceiver::new(&rx).unwrap();
        let af = Arc::new(AtomicBool::new(false));
        let afc = af.clone();

        thread::spawn(move || {
            {
                DevHandle::new(txh, rxh);
            }
            afc.store(true, Ordering::SeqCst);
        });

        thread::sleep(Duration::from_millis(10));
        assert_eq!(af.load(Ordering::SeqCst), false);

        if let DevTx::Detach = prx.recv().unwrap() {
            tx.send(DevRx::Detached(dummy_device())).unwrap();
        } else {
            panic!();
        }

        thread::sleep(Duration::from_millis(10));
        assert_eq!(af.load(Ordering::SeqCst), true);
    }
}