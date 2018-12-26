//! `Mdrv` is a modular driver based on [`Mio`] for managing multiple connections over different protocols
//!
//! # Overview
//! 
//! The core of the `Mdrv` funtionality is the [`Driver`] structure.
//! In maintains an event loop which manages entities called proxies.
//! 
//! [`Proxy`] instances are passed to the driver to be inserted in event loop.
//! They can maintain different connections and reads and writes data over them.
//! 
//! Because of the proxy is moved to the driver we cannot directly access it.
//! The [`wrapper::Handle`] is the way to communicate with the proxy by sending commands and receiving responces.
//! 
//! The pair of [`wrapper::Handle`] and [`wrapper::Proxy`] (the [`Proxy`] instance, that implements communication with the handle)
//! is usually constructed by [`create()`] function. 
//! This function takes user-defined structs that should implement [`user::Proxy`] and [`user::Handle`] traits.
//! Also a command set should be defined for communication between proxy and handle.
//! These commands have to implement [`Tx`] and [`Rx`] traits.
//! 
//! The example dummy implementation of user structures could be found in [`dummy`] module.
//! 
//! # Simple example
//!
//! ```rust
//! use mdrv::{channel, driver, dummy};
//! 
//! // create driver instance
//! let mut driver = driver::Driver::new().unwrap();
//! // create dummy proxy and handle pair
//! let (proxy, mut handle) = dummy::create().unwrap();
//!
//! // We need a simple poll to wait for messages on handle
//! // A regular mio::Poll also can be used for that
//! let mut poll = channel::SinglePoll::new(&handle.rx).unwrap();
//! 
//! driver.attach(Box::new(proxy)).unwrap();
//!
//! // wait for one message from proxy to arrive
//! dummy::wait_msgs(&mut handle, &mut poll, 1).unwrap();
//! 
//! // read message received
//! match handle.user.msgs.pop_front().unwrap() {
//!     dummy::Rx::Attached => println!("attached to the driver"),
//!     other => panic!("{:?}", other),
//! }
//!
//! // now we don't need our proxy anymore
//! handle.close().unwrap(); // this also called on handle drop
//! 
//! // wait for proxy to be closed
//! dummy::wait_close(&mut handle, &mut poll).unwrap();
//! 
//! // read messages again
//! match handle.user.msgs.pop_front().unwrap() {
//!     dummy::Rx::Detached => println!("detached from the driver"),
//!     other => panic!("{:?}", other),
//! }
//! match handle.user.msgs.pop_front().unwrap() {
//!     dummy::Rx::Closed => println!("proxy has been dropped"),
//!     other => panic!("{:?}", other),
//! }
//! ```
//!
//! [`Mio`]: https://docs.rs/mio/
//!
//! [`Driver`]: driver/struct.Driver.html
//! [`Proxy`]: proxy/trait.Proxy.html
//! [`wrapper::Handle`]: wrapper/struct.Handle.html
//! [`wrapper::Proxy`]: wrapper/struct.Proxy.html
//! [`create()`]: wrapper/fn.create.html
//! 
//! [`user::Proxy`]: user/trait.Proxy.html
//! [`user::Handle`]: user/trait.Handle.html
//! [`Tx`]: user/trait.Tx.html
//! [`Rx`]: user/trait.Rx.html
//! 
//! [`dummy`]: dummy/index.html
//! 

extern crate mio;
extern crate mio_extras;


pub mod error;
pub mod result;

pub mod channel;
pub mod proxy;
pub mod wrapper;
pub mod user;

mod event_loop;
pub mod driver;

pub mod dummy;

pub mod net;

pub use error::{Error};
pub use result::{Result};


#[cfg(test)]
#[macro_use]
extern crate matches;
#[cfg(test)]
extern crate rand;
