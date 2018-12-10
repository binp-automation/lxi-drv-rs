use std::mem;
use std::io;
use std::collections::{BTreeMap};
use std::net::{TcpStream};

use mio::*;

use ::channel::{*, Error as ChanError};
use ::device::*;
use ::driver::{DrvCmd};


#[derive(Debug)]
pub enum Error {
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
    fn new(base: Device, tx: Sender<DevRx>, rx: Receiver<DevTx>) -> Result<DevExt, Error> {
        tx.send(DevRx::Attached).map_err(|e| Error::Chan(e.into()))?;
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
    Nop
}

impl EventLoop {
    pub fn new(rx: Receiver<DrvCmd>) -> Result<Self, Error> {
        let poll = Poll::new().map_err(|e| Error::Io(e))?;
        poll.register(&rx, Token(1), Ready::readable(), PollOpt::edge()).map_err(|e| Error::Io(e))?;
        Ok(EventLoop {
            rx,
            devs: BTreeMap::new(),
            idcnt: 1,
            poll,
            rmlist: Vec::new(),
        })
    }

    fn add_device(&mut self, dev: Device, tx: Sender<DevRx>, rx: Receiver<DevTx>) -> Result<DevId, Error> {
        let dev_ext = DevExt::new(dev, tx, rx)?;

        let id = self.idcnt;
        self.idcnt += 1;

        if let Some(_) = self.devs.insert(id, dev_ext) {
            Err(Error::IdPresent(id))
        } else {
            Ok(id)
        }
    }

    fn remove_device(&mut self, id: DevId) -> Result<(), Error> {
        match self.devs.remove(&id) {
            Some(_) => Ok(()),
            None => Err(Error::IdMissing(id)),
        }
    }

    fn handle_self_chan(&mut self) -> Result<LoopCmd, Error> {
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
                    TryRecvError::Disconnected => break Err(Error::Chan(ChanError::Disconnected)),
                },
            }
        }
    }

    fn handle_dev_chan(&mut self, id: DevId) -> Result<(), Error> {
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
                        TryRecvError::Disconnected => break Err(Error::Chan(ChanError::Disconnected)),
                    }
                }
            },
            None => Err(Error::IdMissing(id)),
        }
    }

    fn handle_dev_sock(&mut self, id: DevId) -> Result<(), Error> {
        match self.devs.get(&id) {
            Some(_dev) => unimplemented!(),
            None => Err(Error::IdMissing(id)),
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
                        1 => match self.handle_self_chan().unwrap() {
                            LoopCmd::Break => break 'main,
                            LoopCmd::Nop => continue,
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

            loop {
                match self.rmlist.pop() {
                    Some(id) => self.remove_device(id).unwrap(),
                    None => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    fn loop_wrap<F: FnOnce(&Sender<DrvCmd>)>(f: F) {
        let (tx, rx) = channel();
        let jh = thread::spawn(move || {
            EventLoop::new(rx).unwrap().run();
        });

        f(&tx);

        tx.send(DrvCmd::Terminate).unwrap();
        jh.join().unwrap();
    }

    #[test]
    fn run() {
        loop_wrap(|_| {});
    }
}
