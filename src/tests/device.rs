use super::*;

use std::thread;
use std::time::{Duration};
use std::sync::{Arc, atomic::{Ordering, AtomicBool}};


fn dummy_device() -> Device {
    Device {
        addr: Addr::Dns(String::from("localhost"), 8000),
    }
}

#[test]
fn detach_after() {
    let (tx, rxh) = channel();
    let (txh, rx) = channel();
    let mut prx = PollReceiver::new(&rx).unwrap();

    let dh = DevHandle::new(txh, rxh);
    
    tx.send(DevRx::Detached(dummy_device())).unwrap();

    let dev = dh.detach().unwrap();
    assert_eq!(dev.addr, dummy_device().addr);

    if let Err(RecvError::Disconnected) = prx.recv() {
        // ok
    } else {
        panic!();
    }
}

#[test]
fn detach_before() {
    let (tx, rxh) = channel();
    let (txh, rx) = channel();

    thread::spawn(move || {
        let mut prx = PollReceiver::new(&rx).unwrap();
        
        match prx.recv().unwrap() {
            DevTx::Detach => tx.send(DevRx::Detached(dummy_device())).unwrap(),
            x => panic!("{:?}", x),
        };
        match prx.recv() {
            Err(RecvError::Disconnected) => (),
            x => panic!("{:?}", x),
        };
    });
    thread::sleep(Duration::from_millis(10));

    let dh = DevHandle::new(txh, rxh);
    let dev = dh.detach().unwrap();

    assert_eq!(dev.addr, dummy_device().addr);
}

#[test]
fn detach_txclose() {
    let rxh = channel().1;
    let (txh, _rx) = channel();

    if let Err(DevError::Chan(ChanError::Disconnected)) = DevHandle::new(txh, rxh).detach() {
        // ok
    } else {
        panic!();
    }
}

#[test]
fn detach_rxclose() {
    let (_tx, rxh) = channel();
    let txh = channel().0;

    if let Err(DevError::Chan(ChanError::Disconnected)) = DevHandle::new(txh, rxh).detach() {
        // ok
    } else {
        panic!();
    }
}

#[test]
fn drop() {
    let (tx, rxh) = channel();
    let (txh, rx) = channel();
    let mut prx = PollReceiver::new(&rx).unwrap();
    let af = Arc::new(AtomicBool::new(false));
    let afc = af.clone();

    thread::spawn(move || {
        {
            DevHandle::new(txh, rxh);
        }
        afc.store(true, Ordering::SeqCst);
    });

    thread::sleep(Duration::from_millis(10));
    assert_eq!(af.load(Ordering::SeqCst), false);

    if let DevTx::Detach = prx.recv().unwrap() {
        tx.send(DevRx::Detached(dummy_device())).unwrap();
    } else {
        panic!();
    }

    thread::sleep(Duration::from_millis(10));
    assert_eq!(af.load(Ordering::SeqCst), true);
}
