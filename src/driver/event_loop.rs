use std::cell::{Cell};
use std::time::{Duration};
use std::collections::{BTreeMap, BTreeSet};

use mio;

use ::error::{IdError};
use ::channel::{self, Receiver, TryRecvError};
use ::proxy::{self, Id, Eid, Proxy, Control};
use ::driver::{Tx as Rx};


pub struct EventLoop {
    rx: Receiver<Rx>,
    proxies: BTreeMap<Id, Cell<Option<Box<Proxy>>>>,
    poll: mio::Poll,
}

struct Context {
    events: Cell<Option<mio::Events>>,

    to_add: Vec<Box<Proxy>>,
    to_del: BTreeSet<Id>,

    exit: bool,
}

impl Context {
    fn new(capacity: usize) -> Self {
        Self {
            events: Cell::new(Some(mio::Events::with_capacity(capacity))),
            to_add: Vec::new(),
            to_del: BTreeSet::new(),
            exit: false,
        }
    }

    fn apply(&mut self, ctrl: &Control) -> ::Result<()> {
        if ctrl.closed {
            self.del(ctrl.id)
        } else {
            Ok(())
        }
    }

    fn add(&mut self, proxy: Box<Proxy>) -> ::Result<()> {
        self.to_add.push(proxy);
        Ok(())
    }

    fn del(&mut self, id: Id) -> ::Result<()> {
        self.to_del.insert(id);
        Ok(())
    }
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
            poll,
        })
    }

    fn next_id(&self) -> Id {
        if self.proxies.is_empty() {
            1
        } else {
            *self.proxies.range(..).next_back().unwrap().0 + 1
        }
    }

    fn control(&self, id: Id) -> Control {
        Control::new(id, &self.poll)
    }

    fn attach(&mut self, id: Id, mut proxy: Box<Proxy>) -> ::Result<()> {
        if !self.proxies.contains_key(&id) {
            proxy.attach(&self.control(id)).and_then(|_| {
                match self.proxies.insert(id, Cell::new(Some(proxy))) {
                    Some(_) => unreachable!(),
                    None => Ok(()),
                }
            })
        } else {
            Err(IdError::Present.into())
        }
    }

    fn detach(&mut self, id: Id) -> ::Result<Box<Proxy>> {
        match self.proxies.remove(&id) {
            Some(proxy_cell) => {
                let mut proxy = proxy_cell.into_inner().unwrap();
                match proxy.detach(&self.control(id)) {
                    Ok(()) => Ok(proxy),
                    Err(e) => Err(e),
                }
            },
            None => Err(IdError::Missing.into()),
        }
    }

    fn process_proxy(&self, ctx: &mut Context, ready: mio::Ready, id: Id, eid: Eid) -> ::Result<()> {
        match self.proxies.get(&id) {
            Some(ref proxy_cell) => {
                let mut proxy = proxy_cell.take().unwrap();
                let mut ctrl = self.control(id);
                proxy.process(&mut ctrl, ready, eid).and_then(|_| {
                    ctx.apply(&ctrl)
                }).or_else(|e| {
                    proxy_cell.set(Some(proxy));
                    Err(e)
                })
            },
            None => Err(IdError::Missing.into()),
        }
    }
    
    fn process_self(&self, ctx: &mut Context, ready: mio::Ready, eid: Eid) -> ::Result<()> {
        assert_eq!(eid, 0);
        assert!(ready.is_readable());
        loop {
            match self.rx.try_recv() {
                Ok(evt) => match evt {
                    Rx::Terminate => {
                        ctx.exit = true;
                    },
                    Rx::Attach(proxy) => {
                        match ctx.add(proxy) {
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

    fn process(&self, ctx: &mut Context) -> ::Result<()> {
        let events = ctx.events.take().unwrap();
        let mut result = Ok(());
        for event in events.iter() {
            let token = event.token();
            let (id, eid) = proxy::decode_ids(token);
            let ready = event.readiness();
            match id {
                0 => self.process_self(ctx, ready, eid),
                proxy_id => self.process_proxy(ctx, ready, proxy_id, eid),
            }.unwrap_or_else(|e| {
                result = Err(e);
            });
        }
        ctx.events.set(Some(events));
        result
    }

    fn commit(&mut self, ctx: &mut Context) -> ::Result<()> {
        let mut result = Ok(());
        for id in ctx.to_del.iter() {
            self.detach(*id).map(|_| ()).unwrap_or_else(|e| {
                result = Err(e);
            });
        }
        ctx.to_del.clear();

        let mut id_cnt = self.next_id();
        for proxy in ctx.to_add.drain(..) {
            let id = id_cnt;
            id_cnt += 1;
            self.attach(id, proxy).unwrap_or_else(|e| {
                result = Err(e);
            });
        }

        Ok(())
    }

    fn run_once(&mut self, ctx: &mut Context, timeout: Option<Duration>) -> ::Result<()> {
        self.poll.poll(ctx.events.get_mut().as_mut().unwrap(), timeout).map_err(|e| ::Error::Io(e))?;

        self.process(ctx).unwrap();

        self.commit(ctx).unwrap();

        Ok(())
    }

    pub fn run_forever(&mut self, capacity: usize, timeout: Option<Duration>) -> ::Result<()> {
        let mut ctx = Context::new(capacity);
        while !ctx.exit {
            self.run_once(&mut ctx, timeout)?;
        }
        Ok(())
    }
}
