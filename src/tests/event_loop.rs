use super::*;

use std::thread;
use std::time::{Duration};
use std::sync::{Arc, Mutex};


fn dummy_device() -> Device {
    Device {
        addr: Addr::Dns(String::from("localhost"), 8000),
    }
}

fn loop_wrap<F: FnOnce(Arc<Mutex<EventLoop>>, &Sender<DrvCmd>)>(f: F) {
    let (tx, rx) = channel();
    let el = Arc::new(Mutex::new(EventLoop::new(rx).unwrap()));
    let elc = el.clone();
    let jh = thread::spawn(move || {
        let mut events = Events::with_capacity(16);
        loop {
            if elc.lock().unwrap().run_once(&mut events).unwrap() {
                break;
            }
        }
    });

    f(el, &tx);

    match tx.send(DrvCmd::Terminate) {
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
    	tx.send(DrvCmd::Terminate).unwrap();
    });
}

#[test]
fn attach_one() {
    loop_wrap(|el, tx| {
    	let (dtx, hrx) = channel();
    	let (htx, drx) = channel();
    	let mut phrx = PollReceiver::new(&hrx).unwrap();

    	tx.send(DrvCmd::Attach(dummy_device(), (dtx, drx))).unwrap();
    	//thread::sleep(Duration::from_millis(10));

    	if let DevRx::Attached = phrx.recv().unwrap() {
    		// ok
    	} else {
    		panic!();
    	}
    	assert_eq!(el.lock().unwrap().devs.len(), 1);
    });
}
