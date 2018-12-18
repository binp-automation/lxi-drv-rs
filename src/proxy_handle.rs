use std::time::{Duration, Instant};

use mio;

use ::channel::{self, channel, Sender, Receiver, PollReceiver, TryRecvError, SendError};
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

pub struct ProxyWrapper<P: UserProxy<T, R>, T: TxExt, R: RxExt> {
    pub user: P,
    pub tx: Sender<R>,
    pub rx: Receiver<T>,
}

impl<P: UserProxy<T, R>, T: TxExt, R: RxExt> ProxyWrapper<P, T, R> {
    fn new(user: P, tx: Sender<R>, rx: Receiver<T>) -> ProxyWrapper<P, T, R> {
        ProxyWrapper { user, tx, rx }
    }
}

impl<P: UserProxy<T, R>, T: TxExt, R: RxExt> Proxy for ProxyWrapper<P, T, R> {
    fn attach(&mut self, ctrl: &Control) -> ::Result<()> {
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
    fn detach(&mut self, ctrl: &Control) -> ::Result<()> {
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

impl<P: UserProxy<T, R>, T: TxExt, R: RxExt> Drop for ProxyWrapper<P, T, R> {
    fn drop(&mut self) {
        self.tx.send(Rx::Closed.into()).unwrap()
    }
}


pub trait UserHandle<T: TxExt, R: RxExt> {
    fn process_channel(&mut self, msg: R) -> ::Result<()>;
}

pub struct Handle<H: UserHandle<T, R>, T: TxExt, R: RxExt> {
    pub user: H,
    pub tx: Sender<T>,
    pub rx: Receiver<R>,
    closed: bool,
}

impl<H: UserHandle<T, R>, T: TxExt, R: RxExt> Handle<H, T, R> {
    fn new(user: H, tx: Sender<T>, rx: Receiver<R>) -> Self {
        Handle { user, tx, rx, closed: false }
    }

    fn is_closed_msg(msg: R) -> (R, bool) {
        match msg.into() {
            Ok(bmsg) => {
                let v = match bmsg {
                    Rx::Closed => true,
                    _ => false,
                };
                (bmsg.into(), v)
            },
            Err(umsg) => (umsg, false),
        }
    }

    fn process(&mut self) -> ::Result<()> {
        if self.closed {
            return Err(proxy::Error::Closed.into());
        }

        loop {
            match self.rx.try_recv() {
                Ok(msg) => {
                    let (msg, exit) = Self::is_closed_msg(msg);
                    match self.user.process_channel(msg) {
                        Ok(()) => {
                            if exit {
                                self.closed = true;
                                break Err(proxy::Error::Closed.into());
                            } else {
                                continue;
                            }
                        },
                        Err(e) => break Err(e),
                    }
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(()),
                    TryRecvError::Disconnected => break Err(::Error::from(channel::Error::Disconnected)),
                },
            };
        }
    }

    fn process_for(&mut self, timeout: Option<Duration>) -> ::Result<()> {
        self.process();
        let prx = PollReceiver::new(&self.rx).map_err(|e| ::Error::Channel(e))?;
        let now = Instant::now();
        loop {
            let to = match timeout {
                Some(to) => {
                    let ela = now.elapsed();
                    if (ela >= to) {
                        break Ok(())
                    } else {
                        Some(to - ela)
                    }
                },
                None => None
            };

            match prx.wait(to) {
                Ok(()) => match self.process() {
                    Ok(()) => (),
                    Err(err) => break Err(err)
                },
                Err(err) => match err {
                    channel::RecvError::Empty => break Ok(()),
                    other_recv_error => break Err(::Error::Channel(other_recv_error.into())),
                }
            }
        }
    }

    fn close_ref(&mut self) -> ::Result<()> {
        match self.process().and_then(|_| {
            match self.tx.send(Tx::Close.into()) {
                Ok(()) => self.process_for(None),
                Err(err) => match err {
                    SendError::Io(io_err) => Err(::Error::Channel(channel::Error::Io(io_err))),
                    SendError::Disconnected(_) => self.process_for(Some(Duration::from_millis(10))),
                }
            }
        }) {
            Ok(()) => Err(channel::Error::Empty.into()),
            Err(err) => match err {
                ::Error::Proxy(proxy::Error::Closed) => Ok(()),
                others => Err(others),
            }
        }
    }

    pub fn close(mut self) -> ::Result<()> {
        self.close_ref()
    }
}

impl<H: UserHandle<T, R>, T: TxExt, R: RxExt> Drop for Handle<H, T, R> {
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

pub fn create<P, H, T, R>(user_proxy: P, user_handle: H) -> ::Result<(ProxyWrapper<P, T, R>, Handle<H, T, R>)>
where P: UserProxy<T, R>, H: UserHandle<T, R>, T: TxExt, R: RxExt {
    let (ptx, hrx) = channel();
    let (htx, prx) = channel();
    let proxy = ProxyWrapper::new(user_proxy, ptx, prx);
    let handle = Handle::new(user_handle, htx, hrx);
    Ok((proxy, handle))
}


#[cfg(test)]
mod test {
    use super::*;

    use ::channel::{RecvError};

    use std::thread;
    use std::time::{Duration};
    use std::sync::{Arc, atomic::{Ordering, AtomicBool}};

    use ::test::dummy;

    #[test]
    fn close_after() {
        let (_, mut h) = dummy::create().unwrap();

        h.close_ref().unwrap();

        let hrx = PollReceiver::new(&h.rx).unwrap();
        assert_matches!(hrx.recv(None), Err(RecvError::Disconnected));
    }

    #[test]
    fn close_before() {
        let (p, h) = dummy::create().unwrap();

        thread::spawn(move || {
            let mp = p;
            let prx = PollReceiver::new(&mp.rx).unwrap();
            assert_matches!(prx.recv(None), Ok(Tx::Close));
        });

        thread::sleep(Duration::from_millis(10));
        h.close().unwrap();
    }

    #[test]
    fn drop() {
        let (p, h) = dummy::create().unwrap();

        let af = Arc::new(AtomicBool::new(false));
        let afc = af.clone();

        thread::spawn(move || {
            {
                let _mh = h;
                // dropped here
            }
            afc.store(true, Ordering::SeqCst);
        });

        thread::sleep(Duration::from_millis(10));
        assert_eq!(af.load(Ordering::SeqCst), false);

        (move || {
            let _mp = p;
        })();

        thread::sleep(Duration::from_millis(10));
        assert_eq!(af.load(Ordering::SeqCst), true);
    }
}
