//! [`Mdrv`] is a modular driver based on [`Mio`] for managing multiple connections over different protocols
//!
//! # Overview
//! 
//! The core of the [`Mdrv`] funtionality is the [`Driver`] structure.
//! In maintains an event loop which manages entities called proxies.
//! 
//! [`Proxy`] instances are passed to the driver to be inserted in event loop.
//! They can maintain different connections and reads and writes data over them.
//! 
//! Because of the proxy is moved to the driver we cannot directly access it.
//! The [`Handle`] is the way to communicate with the proxy by sending commands and receiving responces.
//! 
//! The pair of [`Handle`] and [`ProxyWrapper`] (the [`Proxy`] instance, that implements communication with the handle)
//! is usually constructed by [`create()`] function. 
//! This function takes user-defined structs that should implement [`UserProxy`] and [`UserHandle`] traits.
//! Also a command set should be defined for communication between proxy and handle.
//! These commands have to be able to convert from and into the basic [`Tx`] and [`Rx`] commands
//! by implementing [`From`] and [`Into`] traits.
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
//! [`Mdrv`]: https://github.com/binp-automation/mdrv
//! [`Mio`]: https://github.com/carllerche/mio
//!
//! [`Driver`]: driver/struct.Driver.html
//! [`Proxy`]: proxy/trait.Proxy.html
//! [`Handle`]: proxy_handle/struct.Handle.html
//! [`ProxyWrapper`]: proxy_handle/struct.ProxyWrapper.html
//! [`create()`]: proxy_handle/fn.create.html
//! 
//! [`UserProxy`]: proxy_handle/trait.UserProxy.html
//! [`UserHandle`]: proxy_handle/trait.UserHandle.html
//! [`Tx`]: proxy_handle/enum.Tx.html
//! [`Rx`]: proxy_handle/enum.Rx.html
//! 
//! [`From`]: https://doc.rust-lang.org/nightly/core/convert/trait.From.html
//! [`Into`]: https://doc.rust-lang.org/nightly/core/convert/trait.Into.html
//! 
//! [`dummy`]: dummy/index.html
//! 

extern crate mio;
extern crate mio_extras;

pub mod error;
pub mod result;

pub mod channel;
pub mod proxy;
pub mod proxy_handle;
pub mod dummy;

mod event_loop;
pub mod driver;

pub use error::{Error};
pub use result::{Result};


#[cfg(test)]
#[macro_use]
extern crate matches;
