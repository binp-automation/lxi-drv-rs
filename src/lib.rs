extern crate mio;
extern crate mio_extras;

pub mod error;
pub mod result;

pub mod channel;
pub mod proxy;
pub mod proxy_handle;

mod event_loop;
pub mod driver;

pub use error::{Error};
pub use result::{Result};


#[cfg(test)]
#[macro_use]
extern crate matches;

#[cfg(test)]
mod test;
