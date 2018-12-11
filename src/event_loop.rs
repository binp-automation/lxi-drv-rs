#[cfg(test)]
#[path = "./tests/event_loop.rs"]
mod tests;


use std::mem;
use std::io;
use std::collections::{BTreeMap};
use std::net::{TcpStream};

use mio::*;

use ::channel::*;
use ::device::*;
use ::driver::{DrvCmd};


#[derive(Debug)]
pub enum LoopError {
    IdPresent(usize),
    IdMissing(usize),
    Io(io::Error),
    Chan(ChanError),
}

type DevId = usize;

const TOKEN_GRAN_BITS: usize = 1;
const TOKEN_GRAN_MASK: usize = (1<<TOKEN_GRAN_BITS) - 1;

#[allow(dead_code)]
struct DevExt {
    base: Option<Device>,
    tx: Sender<DevRx>,
    rx: Receiver<DevTx>,
    sock: Option<TcpStream>,
}

impl DevExt {
    fn new(base: Device, tx: Sender<DevRx>, rx: Receiver<DevTx>) -> Result<DevExt, LoopError> {
        tx.send(DevRx::Attached).map_err(|e| LoopError::Chan(e.into()))?;
        Ok(DevExt {
            base: Some(base),
            tx, rx,
            sock: None,
        })
    }
}

impl Drop for DevExt {
    fn drop(&mut self) {
        let dev = mem::replace(&mut self.base, None).unwrap();
        self.tx.send(DevRx::Detached(dev)).unwrap();
    }
}

pub struct EventLoop {
    rx: Receiver<DrvCmd>,
    devs: BTreeMap<DevId, DevExt>,
    idcnt: DevId,
    poll: Poll,
    rmlist: Vec<DevId>,
}

enum LoopCmd {
    Break,
    Nop,
}

impl EventLoop {
    pub fn new(rx: Receiver<DrvCmd>) -> Result<Self, LoopError> {
        let poll = Poll::new().map_err(|e| LoopError::Io(e))?;
        poll.register(&rx, Token(1), Ready::readable(), PollOpt::edge()).map_err(|e| LoopError::Io(e))?;
        Ok(EventLoop {
            rx,
            devs: BTreeMap::new(),
            idcnt: 1,
            poll,
            rmlist: Vec::new(),
        })
    }

    fn add_device(&mut self, dev: Device, tx: Sender<DevRx>, rx: Receiver<DevTx>) -> Result<DevId, LoopError> {
        let dev_ext = DevExt::new(dev, tx, rx)?;

        let id = self.idcnt;
        self.idcnt += 1;

        // TODO: more convenient way of transaction
        self.poll.register(&dev_ext.rx, Token(2*id + 1), Ready::readable(), PollOpt::edge()).map_err(|e| LoopError::Io(e))?;
        match self.devs.insert(id, dev_ext) {
            Some(_) => {
                self.poll.deregister(Token(2*id + 1)).map_err(|e| LoopError::Io(e))?;
                LoopError::IdPresent(id)
            },
            None => Ok(id),
        }
    }

    fn remove_device(&mut self, id: DevId) -> Result<(), LoopError> {
        match self.devs.remove(&id) {
            Some(_) => Ok(()),
            None => Err(LoopError::IdMissing(id)),
        }
    }

    fn handle_self_chan(&mut self) -> Result<LoopCmd, LoopError> {
        loop {
            match self.rx.try_recv() {
                Ok(evt) => match evt {
                    DrvCmd::Terminate => break Ok(LoopCmd::Break),
                    DrvCmd::Attach(dev, (tx, rx)) => {
                        match self.add_device(dev, tx, rx) {
                            Ok(_) => continue,
                            Err(err) => break Err(err),
                        }
                    },
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(LoopCmd::Nop),
                    TryRecvError::Disconnected => break Err(LoopError::Chan(ChanError::Disconnected)),
                },
            }
        }
    }

    fn handle_dev_chan(&mut self, id: DevId) -> Result<(), LoopError> {
        match self.devs.get(&id) {
            Some(dev) => loop {
                match dev.rx.try_recv() {
                    Ok(msg) => match msg {
                        DevTx::Send(_data) => unimplemented!(),
                        DevTx::Detach => {
                            self.rmlist.push(id);
                            break Ok(());
                        }
                    },
                    Err(err) => match err {
                        TryRecvError::Empty => break Ok(()),
                        TryRecvError::Disconnected => break Err(LoopError::Chan(ChanError::Disconnected)),
                    }
                }
            },
            None => Err(LoopError::IdMissing(id)),
        }
    }

    fn handle_dev_sock(&mut self, id: DevId) -> Result<(), LoopError> {
        match self.devs.get(&id) {
            Some(_dev) => unimplemented!(),
            None => Err(LoopError::IdMissing(id)),
        }
    }

    fn run_once(&mut self, events: &mut Events) -> Result<bool, LoopError> {
        self.poll.poll(events, None).map_err(|e| LoopError::Io(e))?;

        for event in events.iter() {
            let token = event.token();
            let id = token.0 >> TOKEN_GRAN_BITS;
            let kind = token.0 & TOKEN_GRAN_MASK;
            match id {
                0 => match kind {
                    0 => return Err(LoopError::IdMissing(0)),
                    1 => match self.handle_self_chan()? {
                        LoopCmd::Break => return Ok(true),
                        LoopCmd::Nop => (),
                    },
                    _ => unreachable!(),
                }
                id => match kind {
                    0 => self.handle_dev_chan(id)?,
                    1 => self.handle_dev_sock(id)?,
                    _ => unreachable!(),
                }
            }
        }

        loop {
            match self.rmlist.pop() {
                Some(id) => self.remove_device(id).unwrap(),
                None => break,
            }
        }

        Ok(false)
    }

    pub fn run_forever(mut self, cap: usize) {
        let mut events = Events::with_capacity(cap);
        loop {
            if self.run_once(&mut events).unwrap() {
                break;
            }
        }
    }
}
