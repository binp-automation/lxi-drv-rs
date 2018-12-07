use std::collections::{BTreeMap};
use std::net::{TcpStream};
use std::sync::mpsc::{TryRecvError};

use mio::*;
use mio_extras::channel::{Sender, Receiver};

use ::device::*;
use ::driver::{DrvCmd};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
}

#[derive(Debug)]
pub enum LoopError {
    IdPresent(usize),
    IdMissing(usize),
    TryRecv(TryRecvError),
}


const TOKEN_GRAN_BITS: usize = 1;
const TOKEN_GRAN_MASK: usize = (1<<TOKEN_GRAN_BITS) - 1;

#[allow(dead_code)]
struct DevExt {
    base: Device,
    tx: Sender<DevRx>,
    rx: Receiver<DevTx>,
    sock: Option<TcpStream>,
}

impl DevExt {
    fn new(base: Device, tx: Sender<DevRx>, rx: Receiver<DevTx>) -> DevExt {
        DevExt {
            base, tx, rx,
            sock: None,
        }
    }
}

pub struct EventLoop {
    rx: Receiver<DrvCmd>,
    devs: BTreeMap<usize, DevExt>,
    idcnt: usize,
    poll: Poll,
}

enum LoopCmd {
    Break,
    Error(LoopError),
}

impl EventLoop {
    pub fn new(rx: Receiver<DrvCmd>) -> Result<Self, Error> {
        let poll = Poll::new().map_err(|e| Error::Io(e))?;
        poll.register(&rx, Token(1), Ready::readable(), PollOpt::edge()).unwrap();
        Ok(EventLoop {
            rx,
            devs: BTreeMap::new(),
            idcnt: 1,
            poll,
        })
    }

    fn add_device(&mut self, dev: DevExt) -> Result<usize, LoopError> {
        let id = self.idcnt;
        self.idcnt += 1;
        if let Some(_) = self.devs.insert(id, dev) {
            Err(LoopError::IdPresent(id))
        } else {
            Ok(id)
        }
    }

    fn remove_device(&mut self, id: usize) -> Result<(), LoopError> {
        match self.devs.remove(&id) {
            Some(_) => Ok(()),
            None => Err(LoopError::IdMissing(id)),
        }
    }

    fn handle_self_chan(&mut self) -> Option<LoopCmd> {
        loop {
            match self.rx.try_recv() {
                Ok(evt) => match evt {
                    DrvCmd::Terminate => break Some(LoopCmd::Break),
                    DrvCmd::Attach(dev, tx, rx) => match self.add_device(DevExt::new(dev, tx, rx)) {
                        Ok(_) => continue,
                        Err(err) => break Some(LoopCmd::Error(err)),
                    },
                },
                Err(err) => match err {
                    TryRecvError::Empty => break None,
                    x => break Some(LoopCmd::Error(LoopError::TryRecv(x))),
                },
            }
        }
    }

    fn handle_dev_chan(&mut self, id: usize) -> Result<(), LoopError> {
        match self.devs.get(&id) {
            Some(_dev) => unimplemented!(),
            None => Err(LoopError::IdMissing(id)),
        }
    }

    fn handle_dev_sock(&mut self, id: usize) -> Result<(), LoopError> {
        match self.devs.get(&id) {
            Some(_dev) => unimplemented!(),
            None => Err(LoopError::IdMissing(id)),
        }
    }

    pub fn run(mut self) {
        let mut events = Events::with_capacity(1024);
        'main: loop {
            self.poll.poll(&mut events, None).unwrap();

            for event in events.iter() {
                let token = event.token();
                let id = token.0 >> TOKEN_GRAN_BITS;
                let kind = token.0 & TOKEN_GRAN_MASK;
                match id {
                    0 => match kind {
                        0 => panic!("No such token: {}", 0),
                        1 => match self.handle_self_chan() {
                            Some(cmd) => match cmd {
                                LoopCmd::Break => break 'main,
                                LoopCmd::Error(err) => panic!("{:?}", err),
                            },
                            None => {},
                        },
                        _ => unreachable!(),
                    }
                    id => match kind {
                        0 => self.handle_dev_chan(id).unwrap(),
                        1 => self.handle_dev_sock(id).unwrap(),
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}
/*
#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    fn loop_wrap<F: FnOnce(&IoChan<DrvCmd, EvtRx>)>(f: F) {
        let (chan, evtchan) = IoChan::new_pair();
        let jh = thread::spawn(move || {
            EventLoop::new(evtchan).run();
        });

        f(&chan);

        chan.tx.send(DrvCmd::Term).unwrap();
        jh.join().unwrap();
    }

    #[test]
    fn run() {
        loop_wrap(|_| {});
    }
}
*/
