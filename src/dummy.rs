use mio::*;

use ::channel::*;
use ::proxy::*;


#[derive(Debug)]
pub enum DummyError {
    Chan(ChanError),
    Detached,
}

#[derive(Debug)]
pub enum DummyTx {
    Detach,
}

#[derive(Debug)]
pub enum RxError {}

#[derive(Debug)]
pub enum DummyRx {
    Attached,
    Detached,
}

pub struct DummyProxy {
    pub tx: Sender<DummyRx>,
    pub rx: Receiver<DummyTx>,
}

impl DummyProxy {
    pub fn new(tx: Sender<DummyRx>, rx: Receiver<DummyTx>) -> DummyProxy {
        DummyProxy { tx, rx }
    }
}

impl Proxy for DummyProxy {
    fn register(&mut self, poll: &Poll, id: ProxyId) -> Result<(), ProxyError> {
        poll.register(
            &self.rx, 
            encode_token(id, 0).map_err(|e| ProxyError::from(e))?, 
            Ready::readable(), 
            PollOpt::edge()
        ).map_err(|e| ProxyError::from(e))
    }
    fn deregister(&mut self, poll: &Poll) -> Result<(), ProxyError> {
        poll.deregister(&self.rx).map_err(|e| ProxyError::from(e))
    }

    fn process(&mut self, _poll: &Poll, event: &Event) -> Result<ProxyAction, ProxyError> {
        let token = event.token();
        let (_, port) = decode_token(&token);
        assert_eq!(port, 0);
        assert!(event.readiness().is_readable());

        loop {
            match self.rx.try_recv() {
                Ok(msg) => match msg {
                    DummyTx::Detach => {
                        break Ok(ProxyAction::Detach);
                    }
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(ProxyAction::Nope),
                    TryRecvError::Disconnected => break Err(ProxyError::Chan(ChanError::Disconnected)),
                }
            }
        }
    }
}

pub struct DummyHandle {
    pub tx: Sender<DummyTx>,
    pub rx: Receiver<DummyRx>,
    attached: bool, 
}


impl DummyHandle {
    pub fn new(tx: Sender<DummyTx>, rx: Receiver<DummyRx>) -> Self {
        DummyHandle { tx, rx, attached: true }
    }
    pub fn detach_ref(&mut self) -> Result<(), DummyError> {
        if !self.attached {
            return Err(DummyError::Detached);
        }
        self.attached = false;

        let detached = loop {
            match self.rx.try_recv() {
                Ok(msg) => match msg {
                    DummyRx::Detached => break Ok(true),
                    _ => continue,
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(false),
                    TryRecvError::Disconnected => break Err(DummyError::Chan(ChanError::Disconnected)),
                },
            };
        }?;


        if detached {
            return Ok(());
        }
        
        self.tx.send(DummyTx::Detach).map_err(|e| DummyError::Chan(e.into()))?;

        let mut prx = PollReceiver::new(&self.rx).map_err(|e| DummyError::Chan(e))?;
        loop {
            match prx.recv() {
                Ok(msg) => match msg {
                    DummyRx::Detached => break Ok(()),
                    _ => continue,
                },
                Err(err) => break Err(DummyError::Chan(err.into())),
            }
        }
    }

    pub fn detach(mut self) -> Result<(), DummyError> {
        self.detach_ref()
    }
}

impl Drop for DummyHandle {
    fn drop(&mut self) {
        match self.detach_ref() {
            Ok(_) => (),
            Err(err) => match err {
                DummyError::Detached => (),
                other => panic!("{:?}", other),
            },
        }
    }
}

pub fn dummy() -> (DummyProxy, DummyHandle) {
    let (ptx, hrx) = channel();
    let (htx, prx) = channel();
    let proxy = DummyProxy::new(ptx, prx);
    let handle = DummyHandle::new(htx, hrx);
    (proxy, handle)
}
