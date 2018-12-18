extern crate mio;
extern crate mio_extras;

mod error;
mod result;

pub mod channel;
pub mod proxy;
pub mod wrapper;
pub mod driver;

pub use error::{Error};
pub use result::{Result};


#[cfg(test)]
#[macro_use]
extern crate matches;

#[cfg(test)]
mod test;
