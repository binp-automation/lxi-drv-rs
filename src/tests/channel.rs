use super::*;
use std::thread;
use std::time::{Duration};


#[test]
fn send_recv() {
    let (tx, rx) = channel();

    tx.send(42 as i32).unwrap();

    let mut n = None;
    for _ in 0..100 {
        let r = rx.try_recv();
        if let Err(TryRecvError::Empty) = r {
            thread::sleep(Duration::from_millis(1));
        } else {
            n = Some(r.unwrap());
            break;
        }
    }

    assert_eq!(n.unwrap(), 42);
}

#[test]
fn send_pollrecv() {
    let (tx, rx) = channel();
    let mut prx = PollReceiver::new(&rx).unwrap();

    tx.send(42 as i32).unwrap();
    let n = prx.recv().unwrap();
    
    assert_eq!(n, 42);
}

#[test]
fn send_close_pollrecv() {
    let (tx, rx) = channel();
    let mut prx = PollReceiver::new(&rx).unwrap();

    thread::spawn(move || {
        tx.send(42 as i32).unwrap();
    });
    thread::sleep(Duration::from_millis(10));

    let n = prx.recv().unwrap();
    assert_eq!(n, 42);

    if let Err(RecvError::Disconnected) = prx.recv() {
        // ok
    } else {
        panic!();
    }
}

#[test]
fn send_pollrecv_close() {
    let (tx, rx) = channel();
    let mut prx = PollReceiver::new(&rx).unwrap();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        tx.send(42 as i32).unwrap();
    });

    let n = prx.recv().unwrap();
    assert_eq!(n, 42);

    if let Err(RecvError::Disconnected) = prx.recv() {
        // ok
    } else {
        panic!();
    }
}

#[test]
fn close_send() {
    let tx = channel().0;

    if let Err(SendError::Disconnected(n)) = tx.send(42 as i32) {
        assert_eq!(n, 42);
    } else {
        panic!();
    }
}
