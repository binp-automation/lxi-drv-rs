use std::io;
use std::net::{SocketAddr, IpAddr, Shutdown};
use std::time::{Duration};
use std::error::{Error as StdError};
use std::fmt;

use ::channel::{Sender};
use ::proxy::{self, Control, Eid};
use ::wrapper::{self as w, UserProxy, UserHandle};

use mio::{self, Ready, PollOpt, net::{TcpStream as MioTcpStream}};


#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    NotEmpty,
    Empty,
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::Io(e) => e.description(),
            Error::Empty => "Empty",
            Error::NotEmpty => "Not empty",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            Error::Io(e) => Some(e),
            Error::Empty => None,
            Error::NotEmpty => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", (self as &StdError).description())
    }
}



#[derive(Debug, PartialEq, Eq)]
pub enum Addr {
    Dns(String, u16),
    Ip(IpAddr, u16),
}

pub enum Tx {
    Connect(Addr),
    Disconnect,
    SetOpt(Opt),
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
    DnsResolved(IpAddr),
    Connected(SocketAddr),
    Disconnected,
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

#[derive(Debug, Clone, Copy)]

pub struct ReconnectOpt {
    pub resolve_once: bool,
    pub timeout: Duration,
} 

#[derive(Debug, Clone, Copy)]
pub struct Opt {
    pub reconnect: Option<ReconnectOpt>,
}

impl Default for  Opt {
    fn default() -> Self {
        Self {
            reconnect: None,
        }
    }
}

const EID_SOCK: usize = 1;
const _EID_DNS: usize = 2;
const _EID_SOCK_TIMER: usize = 3;
const _EID_DNS_TIMER: usize = 4;


struct ProxySock {
    addr: SocketAddr,
    sock: Option<MioTcpStream>,
}

impl ProxySock {
    fn new(addr: SocketAddr) -> Self {
        ProxySock { addr, sock: None }
    }

    fn connect(&mut self, ctrl: &Control) -> ::Result<()> {
        if self.is_connected() {
            return Err(Error::NotEmpty.into());
        }
        let sock = MioTcpStream::connect(&self.addr)?;
        ctrl.register(&sock, EID_SOCK, Ready::readable(), PollOpt::edge() | PollOpt::oneshot())?;
        self.sock = Some(sock);
        Ok(())
    }

    fn disconnect(&mut self, ctrl: &Control) -> ::Result<()> {
        match self.sock.take() {
            Some(sock) => {
                ctrl.deregister(&sock)?;
                sock.shutdown(Shutdown::Both).map_err(|e| Error::Io(e).into())
            },
            None => Err(Error::Empty.into()),
        }
    }

    fn try_disconnect(&mut self, ctrl: &Control) -> ::Result<()> {
        if self.is_connected() {
            self.disconnect(ctrl)
        } else {
            Ok(())
        }
    }

    fn is_connected(&self) -> bool {
        self.sock.is_some()
    }
}

impl Drop for ProxySock {
    fn drop(&mut self) {
        if self.is_connected() {
            panic!("ProxySock dropped connected");
        }
    }
}


struct ProxyConn {
    addr: Addr,
    sock: Option<ProxySock>,
}

impl ProxyConn {
    fn new(addr: Addr) -> Self {
        Self { addr, sock: None }
    }

    fn resolve(&mut self, _ctrl: &Control) -> ::Result<Option<SocketAddr>> {
        match self.addr {
            Addr::Dns(ref _name, _port) => unimplemented!(),
            Addr::Ip(addr, port) => Ok(Some(SocketAddr::new(addr, port))),
        }
    }

    fn connect(&mut self, ctrl: &Control, sock_addr: SocketAddr) -> ::Result<()> {
        if self.is_connected() {
            return Err(Error::NotEmpty.into());
        }
        let mut sock = ProxySock::new(sock_addr);
        sock.connect(ctrl)?;
        self.sock = Some(sock);
        Ok(())
    }

    fn disconnect(&mut self, ctrl: &Control) -> ::Result<()> {
        match self.sock.take() {
            Some(mut sock) => sock.try_disconnect(ctrl),
            None => Err(Error::Empty.into()),
        }
    }

    fn try_disconnect(&mut self, ctrl: &Control) -> ::Result<()> {
        if self.is_connected() {
            self.disconnect(ctrl)
        } else {
            Ok(())
        }
    }

    fn is_connected(&self) -> bool {
        self.sock.is_some()
    }
}


pub struct Proxy {
    opt: Opt,
    tx: Option<Sender<Rx>>,
    conn: Option<ProxyConn>,
}

impl Proxy {
    fn new() -> Self {
        Self {
            opt: Opt::default(),
            tx: None,
            conn: None,
        }
    }

    fn on_resolve(&mut self, ctrl: &Control, addr: SocketAddr) -> ::Result<()> {
        match self.conn {
            Some(ref mut conn) => conn.connect(ctrl, addr),
            None => Err(Error::Empty.into()),
        }
    }

    fn connect(&mut self, ctrl: &Control, addr: Addr) -> ::Result<()> {
        if self.conn.is_some() {
            return Err(Error::NotEmpty.into());
        }
        let mut conn = ProxyConn::new(addr);
        let res = conn.resolve(ctrl).and_then(|addr_opt| match addr_opt {
            Some(addr) => self.on_resolve(ctrl, addr),
            None => Ok(()), // resolve asynchronously
        });
        self.conn = Some(conn);
        res
    }

    fn disconnect(&mut self, ctrl: &Control) -> ::Result<()> {
        match self.conn.take() {
            Some(mut conn) => conn.try_disconnect(ctrl),
            None => Err(Error::Empty.into()),
        }
    }

    fn try_disconnect(&mut self, ctrl: &Control) -> ::Result<()> {
        if self.is_connected() {
            self.disconnect(ctrl)
        } else {
            Ok(())
        }
    }

    fn is_connected(&self) -> bool {
        self.conn.is_some()
    }
}

impl proxy::Proxy for Proxy {
    fn attach(&mut self, _ctrl: &Control) -> ::Result<()> {
        if self.is_connected() {
            Err(Error::NotEmpty.into())
        } else {
            Ok(())
        }
    }

    fn detach(&mut self, ctrl: &Control) -> ::Result<()> {
        self.try_disconnect(ctrl)
    }

    fn process(&mut self, _ctrl: &mut Control, _readiness: mio::Ready, _eid: Eid) -> ::Result<()> {
        Ok(())
    }
}

impl UserProxy<Tx, Rx> for Proxy {
    fn set_send_channel(&mut self, tx: Sender<Rx>) {
        self.tx = Some(tx);
    }

    fn process_recv_channel(&mut self, ctrl: &mut Control, msg: Tx) -> ::Result<()> {
        match msg {
            Tx::Connect(addr) => {
                self.connect(ctrl, addr)
            },
            Tx::Disconnect => {
                self.try_disconnect(ctrl)
            },
            Tx::SetOpt(opt) => {
                self.opt = opt;
                Ok(())
            },
            Tx::Close => {
                self.try_disconnect(ctrl)
            },
        }
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
        MioTcpStream::connect(&(LOCALHOST, 9001).into()).unwrap();
    }

    #[test]
    fn sock_data() {
        let lis = listen_free(LOCALHOST, 8000..9000).unwrap();
        let port = lis.local_addr().unwrap().port();
        
        let thr = thread::spawn(move || {
            let mut stream = lis.incoming().next().unwrap().unwrap();
            
            let mut buf = vec!(0; 4);
            stream.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"Send");

            assert_eq!(stream.write(b"Recv").unwrap(), 4);
        });

        let mut sock = MioTcpStream::connect(&(LOCALHOST, port).into()).unwrap();

        assert_eq!(sock.write(b"Send").unwrap(), 4);

        let mut buf = vec!(0; 4);
        let mut got = false;
        for _ in 0..100 {
            match sock.read_exact(&mut buf) {
                Err(err) => {
                    assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
                },
                Ok(()) => {
                    assert_eq!(&buf, b"Recv");
                    got = true;
                    break;
                },
            }
            thread::sleep(Duration::from_millis(10));
        }
        if !got {
            panic!();
        }

        thr.join().unwrap();
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

        let mut sock = MioTcpStream::connect(&SocketAddr::new(LOCALHOST, port)).unwrap();

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

    #[test]
    fn proxy_sock() {
        let lis = listen_free(LOCALHOST, 8000..9000).unwrap();
        let port = lis.local_addr().unwrap().port();
        
        let thr = thread::spawn(move || {
            lis.incoming().next().unwrap().unwrap();
        });

        let poll = Poll::new().unwrap();
        let mut evts = Events::with_capacity(16);
        let mut c = Control::new(0, &poll);
        let mut p = ProxySock::new(SocketAddr::new(LOCALHOST, port));
        p.connect(&mut c).unwrap();

        poll.poll(&mut evts, Some(Duration::from_secs(10))).unwrap();
        let evt = evts.iter().next().unwrap();
        assert_eq!(evt.token().0, 1);
        assert!(evt.readiness().is_readable());

        p.disconnect(&c).unwrap();
        thr.join().unwrap();
    }
}
