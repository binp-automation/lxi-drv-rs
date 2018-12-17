use mio;

use ::channel::{self, channel, Sender, Receiver, PollReceiver, TryRecvError};
use ::proxy::{self, Proxy, Control, Eid};


#[derive(Debug)]
pub enum Tx {
    Close,
}

#[derive(Debug)]
pub enum Rx {
    Attached,
    Detached,
    Closed,
}

pub trait TxExt: From<Tx> + Into<Result<Tx, Self>> {}
pub trait RxExt: From<Rx> + Into<Result<Rx, Self>> {}

impl Into<Result<Tx, Self>> for Tx {
    fn into(self) -> Result<Tx, Self> {
        Ok(self)
    }
}

impl Into<Result<Rx, Self>> for Rx {
    fn into(self) -> Result<Rx, Self> {
        Ok(self)
    }
}

impl TxExt for Tx {}
impl RxExt for Rx {}


pub trait UserProxy<T: TxExt, R: RxExt>: Proxy {
    fn process_channel(&mut self, ctrl: &mut Control, msg: T) -> ::Result<()>;
}

pub struct ChannelProxy<P: UserProxy<T, R>, T: TxExt, R: RxExt> {
    pub user: P,
    pub tx: Sender<R>,
    pub rx: Receiver<T>,
}

impl<P: UserProxy<T, R>, T: TxExt, R: RxExt> ChannelProxy<P, T, R> {
    fn new(user: P, tx: Sender<R>, rx: Receiver<T>) -> ChannelProxy<P, T, R> {
        ChannelProxy { user, tx, rx }
    }
}

impl<P: UserProxy<T, R>, T: TxExt, R: RxExt> Proxy for ChannelProxy<P, T, R> {
    fn attach(&mut self, ctrl: &mut Control) -> ::Result<()> {
        ctrl.register(&self.rx, 0, mio::Ready::readable(), mio::PollOpt::edge())
        .and_then(|_| {
            self.user.attach(ctrl)
            .and_then(|_| {
                self.tx.send(Rx::Attached.into()).map_err(|e| ::Error::Channel(e.into()))
                .and_then(|_| {
                    Ok(())
                })
                .or_else(|e| {
                    self.user.detach(ctrl).unwrap();
                    Err(e)
                })
            })
            .or_else(|e| {
                ctrl.deregister(&self.rx).unwrap();
                Err(e)
            })
        })
    }
    fn detach(&mut self, ctrl: &mut Control) -> ::Result<()> {
        self.user.detach(ctrl)
        .and(ctrl.deregister(&self.rx))
        .and(self.tx.send(Rx::Detached.into()).map_err(|e| ::Error::Channel(e.into())))
    }

    fn process(&mut self, ctrl: &mut Control, readiness: mio::Ready, eid: Eid) -> ::Result<()> {
        match eid {
            0 => {
                assert!(readiness.is_readable());
                loop {
                    match self.rx.try_recv() {
                        Ok(msg) => {
                            let umsg = match msg.into() {
                                Ok(bmsg) => {
                                    match bmsg {
                                        Tx::Close => ctrl.close(),
                                    }
                                    bmsg.into()
                                },
                                Err(umsg) => umsg,
                            };
                            match self.user.process_channel(ctrl, umsg) {
                                Ok(()) => (),
                                Err(e) => break Err(e),
                            }
                        },
                        Err(err) => match err {
                            TryRecvError::Empty => break Ok(()),
                            TryRecvError::Disconnected => break Err(channel::Error::Disconnected.into()),
                        }
                    }
                }
            },
            other_eid => self.user.process(ctrl, readiness, other_eid),
        }
    }
}

impl<P: UserProxy<T, R>, T: TxExt, R: RxExt> Drop for ChannelProxy<P, T, R> {
    fn drop(&mut self) {
        self.tx.send(Rx::Closed.into()).unwrap()
    }
}


pub trait UserHandle<T: TxExt, R: RxExt> {}

pub struct ChannelHandle<H: UserHandle<T, R>, T: TxExt, R: RxExt> {
    pub user: H,
    pub tx: Sender<T>,
    pub rx: Receiver<R>,
    closed: bool,
}

impl<H: UserHandle<T, R>, T: TxExt, R: RxExt> ChannelHandle<H, T, R> {
    fn new(user: H, tx: Sender<T>, rx: Receiver<R>) -> Self {
        ChannelHandle { user, tx, rx, closed: false }
    }

    fn close_ref(&mut self) -> ::Result<()> {
        if self.closed {
            return Err(proxy::Error::Closed.into());
        }
        self.closed = true;

        let closed = loop {
            match self.rx.try_recv() {
                Ok(msg) => match msg.into() {
                    Ok(Rx::Closed) => break Ok(true),
                    _ => continue,
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(false),
                    TryRecvError::Disconnected => break Err(::Error::from(channel::Error::Disconnected)),
                },
            };
        }?;


        if closed {
            return Ok(());
        }
        
        self.tx.send(Tx::Close.into()).map_err(|e| ::Error::Channel(e.into()))?;

        let mut prx = PollReceiver::new(&self.rx).map_err(|e| ::Error::Channel(e.into()))?;
        loop {
            match prx.recv() {
                Ok(msg) => match msg.into() {
                    Ok(Rx::Closed) => break Ok(()),
                    _ => continue,
                },
                Err(err) => break Err(channel::Error::from(err).into()),
            }
        }
    }

    pub fn close(mut self) -> ::Result<()> {
        self.close_ref()
    }
}

impl<H: UserHandle<T, R>, T: TxExt, R: RxExt> Drop for ChannelHandle<H, T, R> {
    fn drop(&mut self) {
        match self.close_ref() {
            Ok(_) => (),
            Err(err) => match err {
                ::Error::Proxy(proxy::Error::Closed) => (),
                other => panic!("{:?}", other),
            },
        }
    }
}

pub fn create<P, H, T, R>(user_proxy: P, user_handle: H) -> ::Result<(ChannelProxy<P, T, R>, ChannelHandle<H, T, R>)>
where P: UserProxy<T, R>, H: UserHandle<T, R>, T: TxExt, R: RxExt {
    let (ptx, hrx) = channel();
    let (htx, prx) = channel();
    let proxy = ChannelProxy::new(user_proxy, ptx, prx);
    let handle = ChannelHandle::new(user_handle, htx, hrx);
    Ok((proxy, handle))
}


#[cfg(test)]
mod test {
    use super::*;

    use ::channel::{RecvError};

    use std::thread;
    use std::time::{Duration};
    //use std::sync::{Arc, atomic::{Ordering, AtomicBool}};

    use ::test::dummy;

    #[test]
    fn close_after() {
        let (_, mut h) = dummy::create().unwrap();

        h.close_ref().unwrap();

        let mut hprx = PollReceiver::new(&h.rx).unwrap();
        assert_matches!(hprx.recv(), Err(RecvError::Disconnected));
    }

    #[test]
    fn close_before() {
        let (p, h) = dummy::create().unwrap();

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
