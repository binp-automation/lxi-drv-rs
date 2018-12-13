extern crate mio;
extern crate mio_extras;

pub mod channel;
pub mod proxy;

//pub mod device;
//pub mod driver;
//mod event_loop;

pub mod dummy;

#[cfg(test)]
#[macro_use]
extern crate matches;

