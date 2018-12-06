use std::collections::{BTreeMap};
use std::time::{Duration};
use std::thread;
//use std::net::{IpAddr, TcpStream};
use std::sync::mpsc::{TryRecvError};

use ::*;
use ::device::*;

pub enum EvtTx {
    Term,
    Add(Device),
    Remove(DevId),
}

pub enum EvtRx {
    Added(Result<DevId, Error>),
    Removed(Result<(), Error>),
}

pub struct EventLoop {
    chan: IoChan<EvtRx, EvtTx>,
    devs: BTreeMap<DevId, Device>,
    idcnt: DevId,
}

impl EventLoop {
    pub fn new(chan: IoChan<EvtRx, EvtTx>) -> Self {
        EventLoop {
            chan,
            devs: BTreeMap::<DevId, Device>::new(),
            idcnt: 1,
        }
    }

    fn add_device(&mut self, dev: Device) -> Result<DevId, Error> {
        let id = self.idcnt;
        self.idcnt += 1;
        if let Some(_) = self.devs.insert(id, dev) {
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

    pub fn run(mut self) {
        'outer: loop {
            'chan: loop {
                match self.chan.rx.try_recv() {
                    Ok(evt) => match evt {
                        EvtTx::Term => break 'outer,
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
                            break 'chan;
                        } else {
                            panic!("{:?}", err);
                        }
                    }
                }
            }

            thread::sleep(Duration::from_millis(100));
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

    #[test]
    fn add_remove() {
        loop_wrap(|chan| {
            let dev = Device {
                addr: Addr::Dns(String::from("localhost"), 8000),
                chan: IoChan::new_pair().0,
            };

            let devid;
            chan.tx.send(EvtTx::Add(dev)).unwrap();
            match chan.rx.recv().unwrap() {
                EvtRx::Added(res) => {
                    match res {
                        Ok(id) => devid = id,
                        Err(_) => panic!("add failed"),
                    }
                },
                _ => unreachable!(),
            }

            chan.tx.send(EvtTx::Remove(devid + 1)).unwrap();
            match chan.rx.recv().unwrap() {
                EvtRx::Removed(res) => {
                    match res {
                        Ok(_) => panic!("remove must fail"),
                        Err(_) => (),
                    }
                },
                _ => unreachable!(),
            }

            chan.tx.send(EvtTx::Remove(devid)).unwrap();
            match chan.rx.recv().unwrap() {
                EvtRx::Removed(res) => {
                    match res {
                        Ok(_) => (),
                        Err(_) => panic!("remove failed"),
                    }
                },
                _ => unreachable!(),
            }
        });
    }
}



