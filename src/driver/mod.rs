mod event_loop;


use std::mem;
use std::thread::{self, JoinHandle};

use ::channel::{channel, Sender};
use ::proxy::{Proxy};

use self::event_loop::{EventLoop};


#[derive(Debug)]
pub enum Error {}

pub enum Tx {
    Attach(Box<Proxy + Send>),
    Terminate,
}

pub struct Driver {
    thr: Option<JoinHandle<()>>,
    tx: Sender<Tx>,
}

impl Driver {
    pub fn new() -> Result<Self, ::Error> {
        let (tx, rx) = channel();
        let thr = thread::spawn(move || {
            EventLoop::new(rx).unwrap().run_forever(1024, None);
        });

        Ok(Driver {
            thr: Some(thr),
            tx: tx,
        })
    }

    pub fn attach(&mut self, proxy: Box<Proxy + Send>) -> ::Result<()> {
        match self.tx.send(Tx::Attach(proxy)) {
            Ok(_) => Ok(()),
            Err(err) => Err(::Error::Channel(err.into())),
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.tx.send(Tx::Terminate).unwrap();
        let thr = mem::replace(&mut self.thr, None).unwrap();
        thr.join().unwrap();
    }
}


/*
#[cfg(test)]
mod test {
    use super::*;

    fn dummy_device() -> DevProxy {
        dummy_dev_port(8000)
    }

    fn dummy_dev_port(port: u16) -> DevProxy {
        DevProxy {
            addr: Addr::Dns(String::from("localhost"), port),
        }
    }

    #[test]
    fn add_remove() {
        let mut drv = Driver::new().unwrap();
        let dev = dummy_device();
        let dh = drv.attach(dev).unwrap();
        dh.detach().unwrap();
    }

    #[test]
    fn add_remove_many() {
        let mut drv = Driver::new().unwrap();
        let devs = (0..16).map(|i| dummy_dev_port(8000 + i));
        let mut dhs = Vec::new();
        for dev in devs {
            dhs.push(drv.attach(dev).unwrap());
        }
        for (i, dh) in dhs.drain(..).enumerate() {
            let dev = dh.detach().unwrap();
            if let Addr::Dns(_, p) = dev.addr {
                assert_eq!(p as usize, 8000 + i);
            } else {
                panic!();
            }
        }
        assert_eq!(dhs.len(), 0);
    }
}
*/
