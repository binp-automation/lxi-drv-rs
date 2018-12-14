#[cfg(test)]
#[path = "./tests/event_loop.rs"]
mod tests;


use std::mem;
use std::io;
use std::time::{Duration};
use std::collections::{BTreeMap};
//use std::net::{TcpStream};

use mio::*;

use ::channel::*;
use ::device::*;
use ::driver::{DrvCmd};


#[derive(Debug)]
pub enum LoopError {
    Io(io::Error),
    Chan(ChanError),
    IdPresent(usize),
    IdMissing(usize),
}

type DevId = usize;

const TOKEN_GRAN_BITS: usize = 1;
const TOKEN_GRAN_MASK: usize = (1<<TOKEN_GRAN_BITS) - 1;

enum Kind {
    Sock,
    Chan
}

fn encode_token(id: DevId, kind: Kind) -> Token {
    let base = id << TOKEN_GRAN_BITS;
    Token(match kind {
        Kind::Chan => base | 0,
        Kind::Sock => base | 1,
    })
}

fn decode_token(token: Token) -> (DevId, Kind) {
    let id = token.0 >> TOKEN_GRAN_BITS;
    let kind = token.0 & TOKEN_GRAN_MASK;
    (id, match kind {
        0 => Kind::Chan,
        1 => Kind::Sock,
        _ => unreachable!(),
    })
}

/*
#[derive(Debug)]
enum PollError {
    Io(io::Error),
    AlreadyPolling,
}

struct PollGuard<'a, E> where E: Evented {
    handle: E,
    poll: Option<&'a Poll>,
}

impl<'a, E> PollGuard<'a, E> where E: Evented {
    fn new(handle: E) -> Self {
        Self { handle, poll: None }
    }

    fn register(&'a mut self, poll: &'a Poll, token: Token, interest: Ready, opts: PollOpt) -> Result<(), PollError> {
        if let None = self.poll {
            poll.register(&self.handle, token, interest, opts).map_err(|e| PollError::Io(e))?;
            self.poll = Some(poll);
            Ok(())
        } else {
            Err(PollError::AlreadyPolling)
        }
    }

    fn deregister(&mut self) -> Result<(), PollError> {
        if let Some(poll) = self.poll {
            poll.deregister(&self.handle).map_err(|e| PollError::Io(e))?;
            self.poll = None;
            Ok(())
        } else {
            Err(PollError::AlreadyPolling)
        }
    }
}
*/

struct DevWrap {
    base: Option<DevProxy>,
    tx: Sender<DevRx>,
    rx: Receiver<DevTx>,
    //sock: Option<TcpStream>,
}

impl DevWrap {
    fn new(base: DevProxy, tx: Sender<DevRx>, rx: Receiver<DevTx>) -> Result<DevWrap, LoopError> {
        tx.send(DevRx::Attached).map_err(|e| LoopError::Chan(e.into()))?;
        Ok(DevWrap {
            base: Some(base),
            tx, rx,
            //sock: None,
        })
    }
}

impl Drop for DevWrap {
    fn drop(&mut self) {
        let dev = mem::replace(&mut self.base, None).unwrap();
        self.tx.send(DevRx::Detached(dev)).unwrap();
    }
}

pub struct EventLoop {
    rx: Receiver<DrvCmd>,
    devs: BTreeMap<DevId, DevWrap>,
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
        poll.register(
            &rx, 
            encode_token(0, Kind::Chan), 
            Ready::readable(), 
            PollOpt::edge()
        ).map_err(|e| LoopError::Io(e))?;
        Ok(EventLoop {
            rx,
            devs: BTreeMap::new(),
            idcnt: 1,
            poll,
            rmlist: Vec::new(),
        })
    }

    fn add_device(&mut self, dev: DevProxy, tx: Sender<DevRx>, rx: Receiver<DevTx>) -> Result<DevId, LoopError> {
        let dw = DevWrap::new(dev, tx, rx)?;

        let id = self.idcnt;
        self.idcnt += 1;

        if !self.devs.contains_key(&id) {
            self.poll.register(
                &dw.rx, 
                encode_token(id, Kind::Chan), 
                Ready::readable(), 
                PollOpt::edge()
            ).map_err(|e| LoopError::Io(e))?;

            if let Some(_) = self.devs.insert(id, dw) {
                unreachable!();
            }
            Ok(id)
        } else {
            Err(LoopError::IdPresent(id))
        }
    }

    fn remove_device(&mut self, id: DevId) -> Result<DevWrap, LoopError> {
        match self.devs.remove(&id) {
            Some(dw) => {
                self.poll.deregister(&dw.rx).map_err(|e| LoopError::Io(e))?;
                Ok(dw)
            },
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

    fn run_once(&mut self, events: &mut Events, timeout: Option<Duration>) -> Result<bool, LoopError> {
        self.poll.poll(events, timeout).map_err(|e| LoopError::Io(e))?;

        for event in events.iter() {
            let token = event.token();
            let (id, kind) = decode_token(token);
            match id {
                0 => match kind {
                    Kind::Chan => match self.handle_self_chan()? {
                        LoopCmd::Break => return Ok(true),
                        LoopCmd::Nop => (),
                    },
                    Kind::Sock => return Err(LoopError::IdMissing(0)),
                }
                id => match kind {
                    Kind::Chan => self.handle_dev_chan(id)?,
                    Kind::Sock => self.handle_dev_sock(id)?,
                }
            }
        }

        loop {
            match self.rmlist.pop() {
                Some(id) => { self.remove_device(id).unwrap(); },
                None => break,
            }
        }

        Ok(false)
    }

    pub fn run_forever(mut self, cap: usize, timeout: Option<Duration>) {
        let mut events = Events::with_capacity(cap);
        loop {
            if self.run_once(&mut events, timeout).unwrap() {
                break;
            }
        }
    }
}
