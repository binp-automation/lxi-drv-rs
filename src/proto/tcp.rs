use std::collections::{VecDeque};

use ::channel::{Sender};
use ::proxy::{self, Control, Eid};
use ::wrapper::{self as w, UserProxy, UserHandle};


pub enum Tx {
    // Base
    Close,
}

impl From<w::Tx> for Tx {
    fn from(other: w::Tx) -> Tx {
        match other {
            w::Tx::Close => Tx::Close,
        }
    } 
}

impl Into<Result<w::Tx, Tx>> for Tx {
    fn into(self) -> Result<w::Tx, Tx> {
        match self {
            Tx::Close => Ok(w::Tx::Close),
            other => Err(other),
        }
    }
}

impl w::TxExt for Tx {}

pub enum Rx {
    // Base
    Attached,
    Detached,
    Closed,
}

impl From<w::Rx> for Rx {
    fn from(other: w::Rx) -> Rx {
        match other {
            w::Rx::Attached => Rx::Attached,
            w::Rx::Detached => Rx::Detached,
            w::Rx::Closed => Rx::Closed,
        }
    } 
}

impl Into<Result<w::Rx, Rx>> for Rx {
    fn into(self) -> Result<w::Rx, Rx> {
        match self {
            Rx::Attached => Ok(w::Rx::Attached),
            Rx::Detached => Ok(w::Rx::Detached),
            Rx::Closed => Ok(w::Rx::Closed),
            other => Err(other),
        }
    }
}

impl w::RxExt for Rx {}


pub struct Proxy {
    tx: Option<Sender<Rx>>,
}

impl Proxy {
    fn new() -> Self {
        Self { tx: None }
    }
}

impl proxy::Proxy for Proxy {
    fn attach(&mut self, _ctrl: &Control) -> ::Result<()> {
        Ok(())
    }

    fn detach(&mut self, _ctrl: &Control) -> ::Result<()> {
        Ok(())
    }

    fn process(&mut self, _ctrl: &mut Control, _readiness: mio::Ready, _eid: Eid) -> ::Result<()> {
        Ok(())
    }
}

impl UserProxy<Tx, Rx> for Proxy {
    fn set_send_channel(&mut self, tx: Sender<Rx>) {
        self.tx = Some(tx);
    }
    fn process_recv_channel(&mut self, _ctrl: &mut Control, _msg: Tx) -> ::Result<()> {
        Ok(())
    }
}

pub struct Handle {
    tx: Option<Sender<Tx>>,
}

impl Handle {
    fn new() -> Self {
        Self { tx: None }
    }
}

impl UserHandle<Tx, Rx> for Handle {
    fn set_send_channel(&mut self, tx: Sender<Tx>) {
        self.tx = Some(tx);
    }
    fn process_recv_channel(&mut self, _msg: Rx) -> ::Result<()> {
        Ok(())
    }
}

pub fn create() -> ::Result<(w::Proxy<Proxy, Tx, Rx>, w::Handle<Handle, Tx, Rx>)> {
    w::create(Proxy::new(), Handle::new())
}


#[cfg(test)]
mod test {
    use super::*;

    use std::io::{self, Read, Write};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, Shutdown};
    use std::thread;
    use std::time::{Duration};
    use std::ops::{Range};

    use mio::{self, Poll, Events, Token, Ready, PollOpt, net::{TcpStream as MioTcpStream}};

    use rand::{prelude::*, rngs::SmallRng};

    static LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    fn bind_free_port(addr: IpAddr, range: Range<u16>) -> Option<TcpListener> {
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
        let lis = bind_free_port(LOCALHOST, 8000..9000).unwrap();
        let port = lis.local_addr().unwrap().port();
        
        let thr = thread::spawn(move || {
            let mut stream = lis.incoming().next().unwrap().unwrap();
            
            let mut buf = vec!(0; 4);
            stream.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"Send");

            assert_eq!(stream.write(b"Recv").unwrap(), 4);
        });

        let mut sock = TcpStream::connect((LOCALHOST, port)).unwrap();

        assert_eq!(sock.write(b"Send").unwrap(), 4);

        let mut buf = vec!(0; 4);
        sock.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"Recv");

        thr.join().unwrap();
    }
    
    #[test]
    fn sock_poll() {
        let lis = bind_free_port(LOCALHOST, 8000..9000).unwrap();
        let port = lis.local_addr().unwrap().port();
        
        let thr = thread::spawn(move || {
            let mut stream = lis.incoming().next().unwrap().unwrap();
            
            let mut buf = vec!(0; 4);
            stream.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"Send");

            assert_eq!(stream.write(b"Rcv0").unwrap(), 4);
            assert_eq!(stream.write(b"Rcv1").unwrap(), 4);

            thread::sleep(Duration::from_millis(1000));
            stream.shutdown(Shutdown::Both).unwrap();
        });

        let mut sock = MioTcpStream::connect(&SocketAddr::new(LOCALHOST, port)).unwrap();

        let poll = Poll::new().unwrap();
        let mut evts = Events::with_capacity(16);

        poll.register(&sock, Token(0), Ready::writable(), PollOpt::edge() | PollOpt::oneshot()).unwrap();
        
        let mut ss = 0;
        let mut sss = 0;
        'outer: loop {
            poll.poll(&mut evts, None).unwrap();

            for evt in evts.iter() {
                assert_eq!(evt.token().0, 0);
                println!("ss: {}", ss);
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
