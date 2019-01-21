use std::time::{Duration};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use mio;

use ::error::{IdError};
use ::channel::{self, Receiver, TryRecvError};
use ::proxy::{
    Id, Eid, decode_ids,
    PollInfo,
    AttachControl, DetachControl, ProcessControl,
    RawProxy as Proxy,
};

use ::driver::{Tx as Rx};


struct Context {
    events: Option<mio::Events>,

    to_add: Vec<Box<dyn Proxy + Send>>,
    to_del: BTreeSet<Id>,
    to_commit: BTreeSet<Id>,

    exit: bool,
}

impl Context {
    fn new(capacity: usize) -> Self {
        Self {
            events: Some(mio::Events::with_capacity(capacity)),
            to_add: Vec::new(),
            to_del: BTreeSet::new(),
            to_commit: BTreeSet::new(),
            exit: false,
        }
    }

    fn add(&mut self, proxy: Box<dyn Proxy + Send>) {
        self.to_add.push(proxy);
    }

    fn del(&mut self, id: Id) {
        self.to_del.insert(id);
    }

    fn proc(&mut self, id: Id) {
        self.to_commit.insert(id);
    }
}

pub struct ProxyEntry {
    pub proxy: Box<dyn Proxy + Send>,
    pub poll_map: HashMap<Eid, PollInfo>,
}

impl ProxyEntry {
    pub fn new(proxy: Box<dyn Proxy + Send>) -> Self {
        Self { proxy, poll_map: HashMap::new() }
    }
}

pub struct EventLoop {
    rx: Receiver<Rx>,
    proxies: BTreeMap<Id, Option<ProxyEntry>>,
    poll: mio::Poll,
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

    fn attach(&mut self, id: Id, proxy: Box<dyn Proxy + Send>) -> ::Result<()> {
        if !self.proxies.contains_key(&id) {
            let mut entry = ProxyEntry::new(proxy);
            {
                let (proxy, poll_map) = (&mut entry.proxy, &mut entry.poll_map);
                let mut ctrl = AttachControl::new(id, &self.poll, poll_map); 
                proxy.attach(&mut ctrl).and_then(|_| {
                    if ctrl.is_closed() {
                        proxy.detach(&mut ctrl).map(|_| false)
                    } else {
                        Ok(true)
                    }
                })
            }.and_then(|should_add| {
                if should_add {
                    match self.proxies.insert(id, Some(entry)) {
                        Some(_) => unreachable!(),
                        None => Ok(()),
                    }
                } else {
                    Ok(())
                }
            })
        } else {
            Err(IdError::Present.into())
        }
    }

    fn detach(&mut self, id: Id) -> ::Result<()> {
        match self.proxies.remove(&id) {
            Some(entry_opt) => {
                let mut entry = entry_opt.unwrap();
                let mut proxy = entry.proxy;
                let mut ctrl = DetachControl::new(id, &self.poll, &mut entry.poll_map);
                proxy.detach(&mut ctrl)
            },
            None => Err(IdError::Missing.into()),
        }
    }

    fn process_proxy(&mut self, ctx: &mut Context, ready: mio::Ready, id: Id, eid: Eid) -> ::Result<()> {
        match self.proxies.get_mut(&id) {
            Some(entry_opt) => {
                let mut entry = entry_opt.take().unwrap();
                let res = {
                    let proxy = &mut entry.proxy;
                    let mut ctrl = ProcessControl::new(
                        id, &self.poll, &mut entry.poll_map,
                        eid, ready,
                    );
                    proxy.process(&mut ctrl).and_then(|_| {
                        if ctrl.is_closed() {
                            ctx.del(id);
                        } else {
                            ctx.proc(id);
                        }
                        Ok(())
                    })
                };
                if entry_opt.replace(entry).is_some() {
                    unreachable!();
                }
                res
            },
            None => Err(IdError::Missing.into()),
        }
    }

    fn process_self(&mut self, ctx: &mut Context, ready: mio::Ready, eid: Eid) -> ::Result<()> {
        assert_eq!(eid, 0);
        assert!(ready.is_readable());
        loop {
            match self.rx.try_recv() {
                Ok(evt) => match evt {
                    Rx::Terminate => {
                        ctx.exit = true;
                    },
                    Rx::Attach(proxy) => {
                        ctx.add(proxy);
                    },
                },
                Err(err) => match err {
                    TryRecvError::Empty => break Ok(()),
                    TryRecvError::Disconnected => break Err(::Error::Channel(channel::Error::Disconnected)),
                },
            }
        }
    }

    fn process(&mut self, ctx: &mut Context) -> ::Result<()> {
        let events = ctx.events.take().unwrap();
        let mut result = Ok(());
        for event in events.iter() {
            let token = event.token();
            let (id, eid) = decode_ids(token);
            let ready = event.readiness();
            match id {
                0 => self.process_self(ctx, ready, eid),
                proxy_id => self.process_proxy(ctx, ready, proxy_id, eid),
            }.unwrap_or_else(|e| {
                result = Err(e);
            });
        }
        if ctx.events.replace(events).is_some() {
            unreachable!();
        }
        result
    }

    fn commit(&mut self, ctx: &mut Context) -> ::Result<()> {
        let mut result = Ok(());

        let mut id_cnt = self.next_id();
        for proxy in ctx.to_add.drain(..) {
            let id = id_cnt;
            id_cnt += 1;
            self.attach(id, proxy).unwrap_or_else(|e| {
                result = Err(e);
            });
        }

        for id in ctx.to_commit.iter() {
            self.commit_proxy(*id).map(|_| ()).unwrap_or_else(|e| {
                result = Err(e);
            });
        }
        ctx.to_commit.clear();

        for id in ctx.to_del.iter() {
            self.detach(*id).map(|_| ()).unwrap_or_else(|e| {
                result = Err(e);
            });
        }
        ctx.to_del.clear();

        result
    }

    fn run_once(&mut self, ctx: &mut Context, timeout: Option<Duration>) -> ::Result<()> {
        self.poll.poll(ctx.events.as_mut().unwrap(), timeout).map_err(|e| ::Error::Io(e))?;

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

impl Drop for EventLoop {
    fn drop(&mut self) {
        let mut res = Ok(());
        for (id, entry_opt) in self.proxies.iter_mut() {
            let mut entry = entry_opt.take().unwrap();
            let mut proxy = entry.proxy;
            let mut ctrl = DetachControl::new(*id, &self.poll, &mut entry.poll_map);
            if let Err(e) = proxy.detach(&mut ctrl) {
                res = Err(e);
            }
        }
        res.unwrap();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::thread;
    use std::sync::{Arc, Mutex};

    use ::channel::{channel, Sender, SendError, SinglePoll};

    use ::proxy::dummy::{self, wait_msgs, wait_close};


    fn loop_wrap<F: FnOnce(Arc<Mutex<EventLoop>>, &Sender<Rx>)>(f: F) {
        let (tx, rx) = channel();
        let el = Arc::new(Mutex::new(EventLoop::new(rx).unwrap()));
        let elc = el.clone();
        let jh = thread::spawn(move || {
            let mut ctx = Context::new(16);
            while !ctx.exit {
                elc.lock().unwrap().run_once(&mut ctx, Some(Duration::from_millis(10))).unwrap();
                thread::sleep(Duration::from_millis(1));
            }
        });

        f(el, &tx);

        match tx.send(Rx::Terminate) {
            Ok(_) => (),
            Err(err) => match err {
                SendError::Disconnected(_) => (),
                xe => panic!("{:?}", xe),
            }
        }
        jh.join().unwrap();
    }

    #[test]
    fn run() {
        loop_wrap(|_, _| {});
    }

    #[test]
    fn terminate() {
        loop_wrap(|_, tx| {
            tx.send(Rx::Terminate).unwrap();
        });
    }
    
    #[test]
    fn attach_detach() {
        loop_wrap(|el, tx| {
            let (p, mut h) = dummy::create().unwrap();
            let mut sp = SinglePoll::new(&h.rx).unwrap();

            tx.send(Rx::Attach(Box::new(p))).unwrap();

            wait_msgs(&mut h, &mut sp, 1).unwrap();
            assert_matches!(h.user.msgs.pop_front(), Some(dummy::Rx::Attached));
            assert_matches!(h.user.msgs.pop_front(), None);
            assert_eq!(el.lock().unwrap().proxies.len(), 1);

            h.close().unwrap();

            wait_close(&mut h, &mut sp).unwrap();
            assert_matches!(h.user.msgs.pop_front(), Some(dummy::Rx::Detached));
            assert_matches!(h.user.msgs.pop_front(), Some(dummy::Rx::Closed));
            assert_matches!(h.user.msgs.pop_front(), None);
            assert_eq!(h.is_closed(), true);
            assert_eq!(el.lock().unwrap().proxies.len(), 0);
        });
    }
}