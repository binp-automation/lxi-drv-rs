extern crate mdrv;

use mdrv::{channel, driver, proto::dummy};


fn main() {

    // create driver instance
    let mut driver = driver::Driver::new().unwrap();
    // create dummy proxy and handle pair
    let (proxy, mut handle) = dummy::create().unwrap();

    // We need a simple poll to wait for messages on handle
    // A regular mio::Poll also can be used for that
    let mut poll = channel::SinglePoll::new(&handle.rx).unwrap();

    driver.attach(Box::new(proxy)).unwrap();

    // wait for one message from proxy to arrive
    dummy::wait_msgs(&mut handle, &mut poll, 1).unwrap();

    // read message received
    match handle.user.msgs.pop_front().unwrap() {
        dummy::Rx::Attached => println!("attached to the driver"),
        other => panic!("{:?}", other),
    }

    // now we don't need our proxy anymore
    handle.close().unwrap(); // this also called on handle drop

    // wait for proxy to be closed
    dummy::wait_close(&mut handle, &mut poll).unwrap();

    // read messages again
    match handle.user.msgs.pop_front().unwrap() {
        dummy::Rx::Detached => println!("detached from the driver"),
        other => panic!("{:?}", other),
    }
    match handle.user.msgs.pop_front().unwrap() {
        dummy::Rx::Closed => println!("proxy has been dropped"),
        other => panic!("{:?}", other),
    }
}
