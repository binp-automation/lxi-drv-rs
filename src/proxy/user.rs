use super::control;
use super::proxy::{self as p};
use super::wrapper::{self as w};


pub trait Tx: From<w::Tx> + Into<Result<w::Tx, Self>> {}
pub trait Rx: From<w::Rx> + Into<Result<w::Rx, Self>> {}

pub trait Proxy<T: Tx, R: Rx>: p::Proxy {
    fn process_recv_channel(&mut self, ctrl: &mut control::Process, msg: T) -> ::Result<()>;
}

pub trait Handle<T: Tx, R: Rx> {
    fn process_recv_channel(&mut self, msg: R) -> ::Result<()>;
}
