use std::mem;
use std::thread::{self, JoinHandle};

use ::channel::{channel, Sender};
use ::proxy::{Proxy};

use super::event_loop::{EventLoop};


#[derive(Debug)]
pub enum Error {}

pub enum Tx {
    Attach(Box<dyn Proxy + Send>),
    Terminate,
}

pub struct Driver {
    thr: Option<JoinHandle<()>>,
    tx: Sender<Tx>,
}

impl Driver {
    pub fn new() -> Result<Self, ::Error> {
        let (tx, rx) = channel();
        let thr = thread::spawn(move || {
            EventLoop::new(rx).unwrap().run_forever(1024, None).unwrap();
        });

        Ok(Driver {
            thr: Some(thr),
            tx: tx,
        })
    }

    pub fn attach(&mut self, proxy: Box<dyn Proxy + Send>) -> ::Result<()> {
        match self.tx.send(Tx::Attach(proxy)) {
            Ok(_) => Ok(()),
            Err(err) => Err(::Error::Channel(err.into())),
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.tx.send(Tx::Terminate).unwrap();
        let thr = mem::replace(&mut self.thr, None).unwrap();
        thr.join().unwrap();
    }
}


#[cfg(test)]
mod test {
    use super::*;

    use ::channel::{SinglePoll};
    use ::proxy::wrapper::{self};
    use ::proxy::dummy::{self, wait_msgs, wait_close};

    fn create_dummy() -> (
        wrapper::Proxy<dummy::Proxy, dummy::Tx, dummy::Rx>,
        wrapper::Handle<dummy::Handle, dummy::Tx, dummy::Rx>,
        SinglePoll,
    ) {
        let (p, h) = dummy::create().unwrap();
        let sp = SinglePoll::new(&h.rx).unwrap();
        (p, h, sp)
    }

    fn test_attach(
        h: &mut wrapper::Handle<dummy::Handle, dummy::Tx, dummy::Rx>,
        sp: &mut SinglePoll,
    ) {
        wait_msgs(h, sp, 1).unwrap();
        assert_matches!(h.user.msgs.pop_front(), Some(dummy::Rx::Attached));
        assert_matches!(h.user.msgs.pop_front(), None);
    }

    fn test_detach(
        h: &mut wrapper::Handle<dummy::Handle, dummy::Tx, dummy::Rx>,
        sp: &mut SinglePoll,
    ) {
        wait_close(h, sp).unwrap();
        assert_matches!(h.user.msgs.pop_front(), Some(dummy::Rx::Detached));
        assert_matches!(h.user.msgs.pop_front(), Some(dummy::Rx::Closed));
        assert_matches!(h.user.msgs.pop_front(), None);
    }

    #[test]
    fn add_remove() {
        let mut drv = Driver::new().unwrap();
        let (p, mut h, mut sp) = create_dummy();

        drv.attach(Box::new(p)).unwrap();
        test_attach(&mut h, &mut sp);

        h.close().unwrap();
        test_detach(&mut h, &mut sp);
    }

    #[test]
    fn add_remove_multiple() {
        let mut drv = Driver::new().unwrap();
        let phs = (0..16).map(|_| create_dummy());
        let mut hs = Vec::new();

        for (p, h, sp) in phs {
            drv.attach(Box::new(p)).unwrap();
            hs.push((h, sp));
        }

        for (h, sp) in hs.iter_mut() {
            test_attach(h, sp);
        }

        for (h, _) in hs.iter_mut() {
            h.close().unwrap();
        }

        for (h, sp) in hs.iter_mut() {
            test_detach(h, sp);
        }

        for (h, _) in hs.iter() {
            assert_eq!(h.is_closed(), true);
        }
    }
}
