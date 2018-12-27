use mio;

use ::channel::{self, channel, Sender, Receiver, SendError, TryRecvError};

use super::id::{Eid};
use super::evented::{EventedWrapper};
use super::control;
use super::proxy::{self as p};
use super::user::{self as u};


const EID_CHAN_RX: Eid = 0;

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

impl u::Tx for Tx {}
impl u::Rx for Rx {}


pub struct Proxy<P: u::Proxy<T, R>, T: u::Tx, R: u::Rx> {
    pub user: P,
    pub tx: Sender<R>,
    pub rx: EventedWrapper<Receiver<T>>,
}

impl<P: u::Proxy<T, R>, T: u::Tx, R: u::Rx> Proxy<P, T, R> {
    fn new(mut user: P, tx: Sender<R>, rx: Receiver<T>) -> Self {
        user.set_send_channel(tx.clone());
        Self {
            user, tx,
            rx: EventedWrapper::new(rx, EID_CHAN_RX),
        }
    }
}

impl<P: u::Proxy<T, R>, T: u::Tx, R: u::Rx> p::Proxy for Proxy<P, T, R> {
    fn attach(&mut self, ctrl: &mut control::Attach) -> ::Result<()> {
        ctrl.register(&self.rx, mio::Ready::readable())
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
    fn detach(&mut self, ctrl: &mut control::Detach) -> ::Result<()> {
        self.user.detach(ctrl)
        .and_then(|_| { ctrl.deregister(&self.rx) })
        .and_then(|_| {
            match self.tx.send(Rx::Detached.into()) {
                Ok(()) => Ok(()),
                Err(err) => match err {
                    SendError::Disconnected(_) => Ok(()),
                    other => Err(::Error::Channel(other.into())),
                }
            }
        })
    }

    fn process(&mut self, ctrl: &mut control::Process) -> ::Result<()> {
        match ctrl.id() {
            EID_CHAN_RX => {
                assert!(ctrl.readiness().is_readable());
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
                            match self.user.process_recv_channel(ctrl, umsg) {
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
            _ => self.user.process(ctrl),
        }
    }
}

impl<P: u::Proxy<T, R>, T: u::Tx, R: u::Rx> Drop for Proxy<P, T, R> {
    fn drop(&mut self) {
        match self.tx.send(Rx::Closed.into()) {
            Ok(()) => (),
            Err(err) => match err {
                SendError::Disconnected(_) => (),
                other => panic!("{:?}", other),
            }
        }
    }
}


pub struct Handle<H: u::Handle<T, R>, T: u::Tx, R: u::Rx> {
    pub user: H,
    pub tx: Sender<T>,
    pub rx: Receiver<R>,
    closed: bool,
}

impl<H: u::Handle<T, R>, T: u::Tx, R: u::Rx> Handle<H, T, R> {
    fn new(mut user: H, tx: Sender<T>, rx: Receiver<R>) -> Self {
        user.set_send_channel(tx.clone());
        Self { user, tx, rx, closed: false }
    }

    fn is_exit(msg: R) -> (R, bool) {
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

    pub fn process(&mut self) -> ::Result<()> {
        if self.closed {
            return Err(p::Error::Closed.into());
        }

        loop {
            match self.rx.try_recv() {
                Ok(msg) => {
                    let (msg, exit) = Self::is_exit(msg);
                    match self.user.process_recv_channel(msg) {
                        Ok(()) => {
                            if exit {
                                self.closed = true;
                                break Err(p::Error::Closed.into());
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

    pub fn close(&mut self) -> ::Result<()> {
        if self.closed {
            return Err(p::Error::Closed.into());
        }
        self.tx.send(Tx::Close.into()).map_err(|e| ::Error::Channel(e.into()))
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl<H: u::Handle<T, R>, T: u::Tx, R: u::Rx> Drop for Handle<H, T, R> {
    fn drop(&mut self) {
        match self.close() {
            Ok(_) => (),
            Err(err) => match err {
                ::Error::Proxy(p::Error::Closed) => (),
                ::Error::Channel(channel::Error::Disconnected) => (),
                other => panic!("{:?}", other),
            },
        }
    }
}

pub fn create<P, H, T, R>(user_proxy: P, user_handle: H) -> ::Result<(Proxy<P, T, R>, Handle<H, T, R>)>
where P: u::Proxy<T, R>, H: u::Handle<T, R>, T: u::Tx, R: u::Rx {
    let (ptx, hrx) = channel();
    let (htx, prx) = channel();
    let proxy = Proxy::new(user_proxy, ptx, prx);
    let handle = Handle::new(user_handle, htx, hrx);
    Ok((proxy, handle))
}


#[cfg(test)]
mod test {
    use super::*;

    use ::channel::{SinglePoll, PollReceiver, RecvError};

    use std::thread;

    use super::super::dummy;

    #[test]
    fn handle_close_after() {
        let (_, mut h) = dummy::create().unwrap();

        assert_matches!(h.close(), Err(::Error::Channel(channel::Error::Disconnected)));

        assert_eq!(h.is_closed(), false);

        assert_matches!(h.process(), Err(::Error::Proxy(p::Error::Closed)));
        assert_matches!(h.user.msgs.pop_front(), Some(Rx::Closed));
        assert_matches!(h.user.msgs.pop_front(), None);
        assert_eq!(h.is_closed(), true);

        assert_matches!(h.rx.try_recv(), Err(TryRecvError::Disconnected));
    }

    #[test]
    fn handle_close_before() {
        let (p, mut h) = dummy::create().unwrap();

        thread::spawn(move || {
            let mp = p;
            let mut prx = PollReceiver::new(&mp.rx).unwrap();
            assert_matches!(prx.recv(None), Ok(Tx::Close));
        });

        h.process().unwrap();

        h.close().unwrap();
        assert_eq!(h.is_closed(), false);

        let mut sp = SinglePoll::new(&h.rx).unwrap();
        sp.wait(None).unwrap();
        assert_matches!(h.process(), Err(::Error::Proxy(p::Error::Closed)));

        assert_matches!(h.user.msgs.pop_front(), Some(Rx::Closed));
        assert_matches!(h.user.msgs.pop_front(), None);
        assert_eq!(h.is_closed(), true);
    }

    #[test]
    fn handle_drop() {
        let (p, _) = dummy::create().unwrap();

        let mut prx = PollReceiver::new(&p.rx).unwrap();
        assert_matches!(prx.recv(None), Ok(Tx::Close));
        assert_matches!(prx.recv(None), Err(RecvError::Disconnected));
    }
}
