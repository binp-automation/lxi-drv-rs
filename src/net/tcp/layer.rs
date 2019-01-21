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

use mio::{Ready, net::{TcpStream}};

use mio_byte_fifo::{Producer, Consumer, ReadTransmit, WriteTransmit};

use ::error::{IdError};
use ::proxy::{
    AttachControl, DetachControl, ProcessControl,
    Eid, Evented, EventedWrapper as Ew,
};

use super::super::{error::{Error as NetError}, layer::{Layer as RawLayer, Notifier}};


const EID_TCP_SOCK: Eid = 0;
const EID_BUF_TX: Eid = 1;
const EID_BUF_RX: Eid = 2;

pub const EIDS_NEXT: Eid = 3;

const BUF_SIZE: usize = 0x400;

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

pub struct Layer<N: Notifier<Msg=R>, R: From<Rx>> {
    opt: Opt,
    sock: Option<Sock>,
    msgr: N,
    txbuf: Ew<Producer>,
    rxbuf: Ew<Consumer>,
    eid_base: Eid,
}

impl<N: Notifier<Msg=R>, R: From<Rx>> Layer<N, R> {
    pub fn new(msgr: N, txbuf: Producer, rxbuf: Consumer, eid_base: Eid) -> Self {
        Self {
            opt: Opt::default(),
            sock: None,
            msgr: msgr,
            txbuf: Ew::new(txbuf, eid_base + EID_BUF_TX),
            rxbuf: Ew::new(rxbuf, eid_base + EID_BUF_RX),
            eid_base,
        }
    }

    pub fn process_message(&mut self, ctrl: &mut ProcessControl, msg: Tx) -> ::Result<()> {
        match msg {
            Tx::Connect(addr) => {
                self.try_disconnect(ctrl).and_then(|_| {
                    self.connect(ctrl, addr)
                })
            },
            Tx::Disconnect => self.try_disconnect(ctrl),
        }
    }

    fn process_socket(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        let mut ready = Ready::empty();
        if ctrl.readiness().is_writable() {
            self.rxbuf.read_transmit()
        }
        if ctrl.readiness().is_readable() {
            
        }
        Ok(())
    }

    fn process_txbuf(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        Ok(())
    }

    fn process_rxbuf(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        Ok(())
    }
}

impl<N: Notifier<Msg=R>, R: From<Rx>> RawLayer for Layer<N, R> {
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
                let handle = Ew::new(sock, self.eid_base + EID_TCP_SOCK);
                ctrl.register(&handle, Ready::all()).and_then(|_| {
                    self.sock = Some(Sock { addr, handle });
                    self.msgr.notify(Rx::Connected.into())
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
                    self.msgr.notify(Rx::Disconnected.into())
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
        match ctrl.id() - self.eid_base {
            EID_TCP_SOCK => self.process_socket(ctrl),
            _ => Err(IdError::Bad.into()),
        }
    }
    fn commit(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use super::*;

    use std::net::{SocketAddr, Shutdown};
    use std::thread;
    use std::time::{Duration};

    use mio::{Poll};

    use super::super::test::{LOCALHOST, listen_free};

    use std::collections::{HashMap};

    use ::channel::{channel, Sender, PollReceiver};
    use ::proxy::{AttachControl};

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

        let mut layer = Layer::<Sender<Rx>, Rx>::new(txch, txbuf, rxbuf, 0);

        layer.connect(&mut ctrl, SocketAddr::new(LOCALHOST, port)).unwrap();

        thread::sleep(Duration::from_millis(10));

        layer.disconnect(&mut ctrl).unwrap();
        
        thr.join().unwrap();
    }
}