//! TCP Layer
//!
//! ## Tips
//!
//! + One `Evented` should be used only in one poll, it won't work in another poll after that
//! + Mio sockets works properly on windows only when polling, they cannot send and receive data otherwise
//! + Sometimes poll can return empty events without waiting for timeout
//!


use std::net::{SocketAddr};
use std::time::Duration;

use mio::{Ready, PollOpt, net::{TcpStream}};

use mio_byte_fifo::{Producer, Consumer};

use ::error::{IdError};
use ::channel::{Sender, Error as ChanError};
use ::proxy::{
    AttachControl, DetachControl, ProcessControl,
    Eid, Evented, EventedWrapper as Ew, EID_CHAN_RX,
};

use super::super::{error::{Error as NetError}, layer::{self as l}};


pub const EID_TCP_SOCK: Eid = 0x10;
pub const EID_BUF_TX: Eid = 0x11;
pub const EID_BUF_RX: Eid = 0x12;

pub type Addr = SocketAddr;


#[derive(Debug, PartialEq, Eq)]
pub enum Tx {
    Connect(Addr),
    Disconnect,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Rx {
    Connected,
    Disconnected,
}


#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RecnOpt {
    pub timeout: Duration,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Opt {
    pub reconnect: Option<RecnOpt>,
}

impl Default for Opt {
    fn default() -> Self {
        Self { reconnect: None }
    }
}

struct Sock {
    addr: Addr,
    handle: Ew<TcpStream>,
}

pub struct Layer<R: From<Rx>> {
    opt: Opt,
    sock: Option<Sock>,
    chan: Sender<R>,
    txbuf: Producer,
    rxbuf: Consumer,
}

impl<R: From<Rx>> Layer<R> {
    fn process_channel(&mut self, ctrl: &mut ProcessControl, msg: Tx) -> ::Result<()> {
        Ok(())
    }

    fn process_socket(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        Ok(())
    }
}

impl<R: From<Rx>> l::Layer for Layer<R> {
    type Addr = Addr;
    type Opt = Opt;

    fn opt(&self) -> Self::Opt {
        self.opt.clone()
    }
    fn set_opt(&mut self, opt: Self::Opt) {
        self.opt = opt;
    }

    fn connect(&mut self, ctrl: &mut AttachControl, addr: Self::Addr) -> ::Result<()> {
        if self.sock.is_none() {
            TcpStream::connect(&addr).map_err(|e| e.into()).and_then(|sock| {
                let handle = Ew::new(sock, EID_TCP_SOCK);
                ctrl.register(&handle, Ready::all()).and_then(|_| {
                    self.sock = Some(Sock { addr, handle });
                    self.chan.send(Rx::Connected.into()).map_err(|e| ChanError::from(e).into())
                })
            })
        } else {
            Err(NetError::AlreadyConnected.into())
        }
    }
    fn disconnect(&mut self, ctrl: &mut DetachControl) -> ::Result<()> {
        match self.sock.take() {
            Some(sock) => {
                ctrl.deregister(&sock.handle).and_then(|_| {
                    self.chan.send(Rx::Disconnected.into()).map_err(|e| ChanError::from(e).into())
                })
            },
            None => Err(NetError::NotConnected.into()),
        }
    }
    fn addr(&self) -> Option<&Self::Addr> {
        match self.sock {
            Some(ref sock) => Some(&sock.addr),
            None => None,
        }
    }
    fn process(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        match ctrl.id() {
            //EID_CHAN_RX => self.process_channel(ctrl),
            EID_TCP_SOCK => self.process_socket(ctrl),
            _ => Err(IdError::Bad.into()),
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;

    use std::io::{self, Read, Write};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, Shutdown};
    use std::thread;
    use std::time::{Duration};
    use std::ops::{Range};

    use mio::{Poll, Events, Token};

    use rand::{prelude::*, rngs::SmallRng};

    static LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    fn listen_free(addr: IpAddr, range: Range<u16>) -> Option<TcpListener> {
        let mut rng = SmallRng::from_entropy();
        let rlen = range.end - range.start;
        for _ in 0..rlen {
            let port = range.start + (rng.next_u32() % rlen as u32) as u16;
            match TcpListener::bind((addr, port)) {
                Ok(lis) => return Some(lis),
                Err(_) => (),
            }
        }
        None
    }

    #[test]
    fn sock() {
        TcpStream::connect(&(LOCALHOST, 9001).into()).unwrap();
    }

    #[test]
    fn sock_poll() {
        let lis = listen_free(LOCALHOST, 8000..9000).unwrap();
        let port = lis.local_addr().unwrap().port();
        
        let thr = thread::spawn(move || {
            let mut stream = lis.incoming().next().unwrap().unwrap();
            
            let mut buf = vec!(0; 4);
            stream.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"Send");

            assert_eq!(stream.write(b"Rcv0").unwrap(), 4);
            assert_eq!(stream.write(b"Rcv1").unwrap(), 4);

            thread::sleep(Duration::from_millis(10));
            stream.shutdown(Shutdown::Both).unwrap();
        });

        let mut sock = TcpStream::connect(&SocketAddr::new(LOCALHOST, port)).unwrap();

        let poll = Poll::new().unwrap();
        let mut evts = Events::with_capacity(16);

        poll.register(&sock, Token(0), Ready::writable(), PollOpt::edge() | PollOpt::oneshot()).unwrap();
        
        let mut ss = 0;
        let mut sss = 0;
        'outer: loop {
            poll.poll(&mut evts, Some(Duration::from_secs(10))).unwrap();

            for evt in evts.iter() {
                assert_eq!(evt.token().0, 0);
                match ss {
                    0 => { // send
                        assert!(evt.readiness().is_writable());
                        assert_eq!(sock.write(b"Send").unwrap(), 4);
                        poll.reregister(&sock, Token(0), Ready::readable(), PollOpt::edge() | PollOpt::oneshot()).unwrap();
                        ss = 1;
                    },
                    1 => { // receive
                        assert!(evt.readiness().is_readable());
                        let mut buf = vec!(0; 4);

                        match sss {
                            0 => {
                                assert_eq!(sock.read(&mut buf).unwrap(), 4);
                                assert_eq!(&buf, b"Rcv0");
                                sss = 1;
                                thread::sleep(Duration::from_millis(10));
                            },
                            1 => {
                                assert_eq!(sock.read(&mut buf).unwrap(), 4);
                                assert_eq!(&buf, b"Rcv1");
                                sss = 2;
                            },
                            2 => {
                                match sock.read(&mut buf) {
                                    Err(err) => {
                                        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
                                    },
                                    Ok(n) => {
                                        assert_eq!(n, 0);
                                    },
                                }
                                ss = 2;
                            },
                            _ => panic!(),
                        }
                        poll.reregister(&sock, Token(0), Ready::readable(), PollOpt::edge() | PollOpt::oneshot()).unwrap();
                    },
                    2 => { // closed
                        assert!(evt.readiness().is_readable());
                        let mut buf = vec!(0; 4);
                        assert_eq!(sock.read(&mut buf).unwrap(), 0);
                        break 'outer;
                    },
                    _ => unreachable!(),
                }
            }
        }
        
        thr.join().unwrap();
    }


    use std::collections::{HashMap};

    use ::channel::{channel, PollReceiver};
    use ::proxy::{AttachControl};
    use self::l::Layer as RawLayer;

    #[test]
    fn layer_connect() {
        let lis = listen_free(LOCALHOST, 8000..9000).unwrap();
        let port = lis.local_addr().unwrap().port();
        
        let (txch, rxch) = channel();
        let (txbuf, rxbuf) = mio_byte_fifo::create(16);

        let thr = thread::spawn(move || {
            let stream = lis.incoming().next().unwrap().unwrap();
            
            let mut prx = PollReceiver::new(&rxch).unwrap();

            assert_eq!(
                prx.recv(Some(Duration::from_secs(10))).unwrap(),
                Rx::Connected
            );

            assert_eq!(
                prx.recv(Some(Duration::from_secs(10))).unwrap(),
                Rx::Disconnected
            );

            stream.shutdown(Shutdown::Both).unwrap();
        });

        let poll = Poll::new().unwrap();
        let mut hmap = HashMap::new();
        let mut ctrl = AttachControl::new(0, &poll, &mut hmap);

        let mut layer = Layer::<Rx> {
            opt: Opt::default(),
            sock: None,
            chan: txch,
            txbuf,
            rxbuf,
        };

        layer.connect(&mut ctrl, SocketAddr::new(LOCALHOST, port)).unwrap();

        thread::sleep(Duration::from_millis(10));

        layer.disconnect(&mut ctrl).unwrap();
        
        thr.join().unwrap();
    }
}