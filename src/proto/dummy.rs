use mio::*;

use ::error::{Error, IdError};
use ::result;
use ::channel::{self, channel, Sender, Receiver, PollReceiver, TryRecvError};
use ::proxy::{self, *};


#[derive(Debug)]
enum Tx {
    Close,
}

#[derive(Debug)]
enum Rx {
    Attached,
    Detached,
    Closed,
}

pub struct Proxy {
    tx: Sender<Rx>,
    rx: Receiver<Tx>,
}

impl Proxy {
    fn new(tx: Sender<Rx>, rx: Receiver<Tx>) -> Proxy {
        Proxy { tx, rx }
    }
}

impl proxy::Proxy for Proxy {
    fn attach(&mut self, poll: &Poll, id: Id) -> result::Result<()> {
        poll.register(
            &self.rx, 
            encode_ids(id, 0).ok_or(Error::from(IdError::Bad))?, 
            Ready::readable(), 
            PollOpt::edge()
        ).map_err(|e| Error::from(e))?;
        self.tx.send(Rx::Attached).map_err(|e| Error::Channel(e.into()))
    }
    fn detach(&mut self, poll: &Poll) -> result::Result<()> {
        poll.deregister(&self.rx).map_err(|e| Error::from(e))?;
        self.tx.send(Rx::Detached).map_err(|e| Error::Channel(e.into()))
    }

    fn process(&mut self, _poll: &Poll, event: &Event) -> result::Result<Action> {
        let token = event.token();
        let (_, port) = decode_ids(token);
        assert_eq!(port, 0);
        assert!(event.readiness().is_readable());

        loop {
            match self.rx.try_recv() {
                Ok(msg) => match msg {
                    Tx::Close => {
                        break Ok(Action::Close);
                    }
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(Action::Nope),
                    TryRecvError::Disconnected => break Err(channel::Error::Disconnected.into()),
                }
            }
        }
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.tx.send(Rx::Closed).unwrap()
    }
}

pub struct Handle {
    tx: Sender<Tx>,
    rx: Receiver<Rx>,
    closed: bool,
}


impl Handle {
    fn new(tx: Sender<Tx>, rx: Receiver<Rx>) -> Self {
        Handle { tx, rx, closed: false }
    }

    fn close_ref(&mut self) -> result::Result<()> {
        if self.closed {
            return Err(proxy::Error::Closed.into());
        }
        self.closed = true;

        let closed = loop {
            match self.rx.try_recv() {
                Ok(msg) => match msg {
                    Rx::Closed => break Ok(true),
                    _ => continue,
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(false),
                    TryRecvError::Disconnected => break Err(Error::from(channel::Error::Disconnected)),
                },
            };
        }?;


        if closed {
            return Ok(());
        }
        
        self.tx.send(Tx::Close).map_err(|e| Error::Channel(e.into()))?;

        let mut prx = PollReceiver::new(&self.rx).map_err(|e| Error::Channel(e.into()))?;
        loop {
            match prx.recv() {
                Ok(msg) => match msg {
                    Rx::Closed => break Ok(()),
                    _ => continue,
                },
                Err(err) => break Err(channel::Error::from(err).into()),
            }
        }
    }

    pub fn close(mut self) -> result::Result<()> {
        self.close_ref()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        match self.close_ref() {
            Ok(_) => (),
            Err(err) => match err {
                Error::Proxy(proxy::Error::Closed) => (),
                other => panic!("{:?}", other),
            },
        }
    }
}

pub fn create() -> (Proxy, Handle) {
    let (ptx, hrx) = channel();
    let (htx, prx) = channel();
    let proxy = Proxy::new(ptx, prx);
    let handle = Handle::new(htx, hrx);
    (proxy, handle)
}


#[cfg(test)]
mod test {
    use super::*;

    use ::channel::{RecvError};

    use std::thread;
    use std::time::{Duration};
    //use std::sync::{Arc, atomic::{Ordering, AtomicBool}};

    #[test]
    fn close_after() {
        let (_, mut h) = create();

        h.close_ref().unwrap();

        let mut hprx = PollReceiver::new(&h.rx).unwrap();
        assert_matches!(hprx.recv(), Err(RecvError::Disconnected));
    }

    #[test]
    fn close_before() {
        let (p, h) = create();

        thread::spawn(move || {
            let mp = p;
            let mut pprx = PollReceiver::new(&mp.rx).unwrap();
            assert_matches!(pprx.recv(), Ok(Tx::Close));
        });

        thread::sleep(Duration::from_millis(10));
        h.close().unwrap();
    }

    /*
    use super::*;

    use std::thread;
    use std::time::{Duration};
    use std::sync::{Arc, atomic::{Ordering, AtomicBool}};

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

        if let Err(DevError::Chan(ChanError::Disconnected)) = DevHandle::new(txh, rxh).detach() {
            // ok
        } else {
            panic!();
        }
    }

    #[test]
    fn detach_rxclose() {
        let (_tx, rxh) = channel();
        let txh = channel().0;

        if let Err(DevError::Chan(ChanError::Disconnected)) = DevHandle::new(txh, rxh).detach() {
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
    */
}
