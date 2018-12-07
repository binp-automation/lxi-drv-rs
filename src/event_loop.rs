use std::collections::{BTreeMap};
use std::net::{TcpStream};
use std::sync::mpsc::{TryRecvError};

use mio::*;

use ::*;
use ::device::*;

#[derive(Debug)]
pub enum EvtTx {
    Term,
    Add(Device),
    Remove(DevId),
}

#[derive(Debug)]
pub enum EvtRx {
    Added(Result<DevId, Error>),
    Removed(Result<(), Error>),
}

const TOKEN_GRAN_BITS: usize = 1;
const TOKEN_GRAN_MASK: usize = (1<<TOKEN_GRAN_BITS) - 1;

#[allow(dead_code)]
struct DevExt {
    base: Device,
    sock: Option<TcpStream>,
}

impl DevExt {
    fn new(base: Device) -> DevExt {
        DevExt {
            base,
            sock: None,
        }
    }
}

pub struct EventLoop {
    chan: IoChan<EvtRx, EvtTx>,
    devs: BTreeMap<DevId, DevExt>,
    idcnt: DevId,
    poll: Poll,
}

enum LoopCmd {
    Break,
}

impl EventLoop {
    pub fn new(chan: IoChan<EvtRx, EvtTx>) -> Self {
        let poll = Poll::new().unwrap();
        poll.register(&chan.rx, Token(1), Ready::readable(), PollOpt::edge()).unwrap();
        EventLoop {
            chan,
            devs: BTreeMap::new(),
            idcnt: 1,
            poll,
        }
    }

    fn add_device(&mut self, device: Device) -> Result<DevId, Error> {
        let id = self.idcnt;
        self.idcnt += 1;
        let de = DevExt::new(device);
        if let Some(_) = self.devs.insert(id, de) {
            unreachable!();
        }
        Ok(id)
    }

    fn remove_device(&mut self, id: DevId) -> Result<(), Error> {
        match self.devs.remove(&id) {
            Some(_) => Ok(()),
            None => Err(Error::Other(format!("No such id: {}", id))),
        }
    }

    fn handle_self_chan(&mut self) -> Option<LoopCmd> {
        loop {
            let res = self.chan.rx.try_recv();
            match res {
                Ok(evt) => match evt {
                    EvtTx::Term => break Some(LoopCmd::Break),
                    EvtTx::Add(dev) => {
                        let res = self.add_device(dev);
                        self.chan.tx.send(EvtRx::Added(res)).unwrap();
                    },
                    EvtTx::Remove(id) => {
                        let res = self.remove_device(id);
                        self.chan.tx.send(EvtRx::Removed(res)).unwrap();
                    }
                },
                Err(err) => {
                    if let TryRecvError::Empty = err {
                        break None;
                    } else {
                        panic!("{:?}", err);
                    }
                }
            }
        }
    }

    fn handle_dev_chan(&mut self, id: DevId) {
        match self.devs.get(&id) {
            Some(_dev) => unimplemented!(),
            None => panic!("No device with such id: {}", id),
        }
    }

    fn handle_dev_sock(&mut self, _id: DevId) {
        unimplemented!();
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
                            },
                            None => {},
                        },
                        _ => unreachable!(),
                    }
                    id => match kind {
                        0 => self.handle_dev_chan(id),
                        1 => self.handle_dev_sock(id),
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use ::*;
    use ::event_loop::*;

    fn loop_wrap<F: FnOnce(&IoChan<EvtTx, EvtRx>)>(f: F) {
        let (chan, evtchan) = IoChan::new_pair();
        let jh = thread::spawn(move || {
            EventLoop::new(evtchan).run();
        });

        f(&chan);

        chan.tx.send(EvtTx::Term).unwrap();
        jh.join().unwrap();
    }

    #[test]
    fn run() {
        loop_wrap(|_| {});
    }
}



