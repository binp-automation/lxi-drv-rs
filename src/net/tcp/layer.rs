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

use super::super::layer::{self as l};

use ::proxy::{
    RawProxy,
    AttachControl, DetachControl, ProcessControl,
};


pub type Addr = SocketAddr;


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

pub struct Layer {
    pub opt: Opt,
    pub addr: Option<Addr>,
    pub sock: Option<TcpStream>,
}

impl RawProxy for Layer {
    fn attach(&mut self, ctrl: &mut AttachControl) -> ::Result<()> {
        Ok(())
    }
    fn detach(&mut self, ctrl: &mut DetachControl) -> ::Result<()> {
        Ok(())
    }
    fn process(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        Ok(())
    }
}

impl l::Layer for Layer {
    type Addr = Addr;
    type Opt = Opt;

    fn opt(&self) -> Self::Opt {
        self.opt.clone()
    }
    fn set_opt(&mut self, opt: Self::Opt) {
        self.opt = opt;
    }

    fn connect(&mut self, ctrl: &mut AttachControl, addr: Self::Addr) -> ::Result<()> {
        Ok(())
    }
    fn disconnect(&mut self, ctrl: &mut DetachControl) -> ::Result<()> {
        Ok(())
    }
    fn addr(&self) -> Option<&Self::Addr> {
        self.addr.as_ref()
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
}