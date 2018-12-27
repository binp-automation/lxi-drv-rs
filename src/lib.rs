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
//! [`Mio`]: https://docs.rs/mio/
//!
//! [`Driver`]: driver/struct.Driver.html
//! [`Proxy`]: proxy/trait.Proxy.html
//! [`wrapper::Handle`]: proxy/wrapper/struct.Handle.html
//! [`wrapper::Proxy`]: proxy/wrapper/struct.Proxy.html
//! [`create()`]: proxy/wrapper/fn.create.html
//! 
//! [`user::Proxy`]: proxy/user/trait.Proxy.html
//! [`user::Handle`]: proxy/user/trait.Handle.html
//! [`Tx`]: proxy/user/trait.Tx.html
//! [`Rx`]: proxy/user/trait.Rx.html
//! 
//! [`dummy`]: proxy/dummy/index.html
//! 

extern crate mio;
extern crate mio_extras;


pub mod error;
pub mod result;
pub mod channel;

pub mod proxy;
pub mod driver;
//pub mod net;

pub use error::{Error};
pub use result::{Result};


#[cfg(test)]
#[macro_use]
extern crate matches;
#[cfg(test)]
extern crate rand;
