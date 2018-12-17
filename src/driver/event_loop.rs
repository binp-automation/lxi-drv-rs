use std::time::{Duration};
use std::collections::{BTreeMap, BTreeSet};

use mio;

use ::error::{IdError};
use ::channel::{self, Receiver, TryRecvError};
use ::proxy::{self, Id, Eid, Proxy, Control};
use ::driver::{Tx as Rx};


pub struct EventLoop {
    rx: Receiver<Rx>,
    proxies: BTreeMap<Id, Box<Proxy>>,
    idcnt: Id,
    poll: mio::Poll,
}

struct Context {
    events: mio::Events,
    rmlist: BTreeSet<Id>,
    exit: bool,
}

impl EventLoop {
    pub fn new(rx: Receiver<Rx>) -> ::Result<Self> {
        let poll = mio::Poll::new().map_err(|e| ::Error::Io(e))?;
        poll.register(
            &rx, mio::Token(0),
            mio::Ready::readable(),
            mio::PollOpt::edge()
        ).map_err(|e| ::Error::Io(e))?;
        Ok(EventLoop {
            rx,
            proxies: BTreeMap::new(),
            idcnt: 1,
            poll,
        })
    }

    fn control(&self, id: Id) -> Control {
        Control::new(id, &self.poll)
    }

    fn check_control(&mut self, context: &mut Context, ctrl: &Control) {
        if ctrl.closed {
            context.rmlist.insert(ctrl.id);
        }
    }

    fn attach_proxy(&mut self, context: &mut Context, proxy: Box<Proxy>) -> ::Result<Id> {
        let id = self.idcnt;
        self.idcnt += 1;

        if !self.proxies.contains_key(&id) {
            let mut ctrl = self.control(id);
            proxy.attach(&mut ctrl).and_then(|_| {
                match self.proxies.insert(id, proxy) {
                    Some(_) => unreachable!(),
                    None => {
                        self.check_control(context, &ctrl);
                        Ok(id)
                    },
                }
            })
        } else {
            Err(IdError::Present.into())
        }
    }

    fn detach_proxy(&mut self, context: &mut Context, id: Id) -> ::Result<Box<Proxy>> {
        match self.proxies.remove(&id) {
            Some(proxy) => match proxy.detach(&mut self.control(id)) {
                Ok(()) => Ok(proxy),
                Err(e) => Err(e),
            },
            None => Err(IdError::Missing.into()),
        }
    }


    fn process_proxy(&mut self, context: &mut Context, ready: mio::Ready, id: Id, eid: Eid) -> Result<(), ::Error> {
        match self.proxies.get(&id) {
            Some(&proxy) => {
                let mut ctrl = self.control(id);
                proxy.process(&mut ctrl, ready, eid)?;
                self.check_control(context, &ctrl);
                Ok(())
            },
            None => Err(IdError::Missing.into()),
        }
    }
    
    fn process_self(&mut self, context: &mut Context, ready: mio::Ready, eid: Eid) -> Result<(), ::Error> {
        assert!(ready.is_readable());
        loop {
            match self.rx.try_recv() {
                Ok(evt) => match evt {
                    Rx::Terminate => {
                        context.exit = true;
                    },
                    Rx::Attach(proxy) => {
                        match self.attach_proxy(context, proxy) {
                            Ok(_) => continue,
                            Err(err) => break Err(err),
                        }
                    },
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(()),
                    TryRecvError::Disconnected => break Err(::Error::Channel(channel::Error::Disconnected)),
                },
            }
        }
    }

    fn run_once(&mut self, context: &mut Context, timeout: Option<Duration>) -> ::Result<()> {
        let exit = false;

        self.poll.poll(&mut context.events, timeout).map_err(|e| ::Error::Io(e))?;

        for event in context.events.iter() {
            let token = event.token();
            let (id, eid) = proxy::decode_ids(token);
            let ready = event.readiness();
            match id {
                0 => self.process_self(context, ready, eid),
                proxy_id => self.process_proxy(context, ready, proxy_id, eid),
            }?;
        }

        for id in context.rmlist.iter() {
            self.detach_proxy(context, *id)?;
        }
        context.rmlist.clear();

        Ok(())
    }

    pub fn run_forever(&mut self, capacity: usize, timeout: Option<Duration>) -> ::Result<()> {
        let mut context = Context {
            events: mio::Events::with_capacity(capacity),
            rmlist: BTreeSet::new(),
            exit: false,
        };
        while !context.exit {
            self.run_once(&mut context, timeout)?;
        }
        Ok(())
    }
}
