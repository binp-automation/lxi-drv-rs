use ::channel::{Sender};
use super::proxy::{self as p};
use super::control::{Control};
use super::wrapper::{self as w};


pub trait Tx: From<w::Tx> + Into<Result<w::Tx, Self>> {}
pub trait Rx: From<w::Rx> + Into<Result<w::Rx, Self>> {}

pub trait Proxy<T: Tx, R: Rx>: p::Proxy {
    fn set_send_channel(&mut self, tx: Sender<R>);
    fn process_recv_channel(&mut self, ctrl: &mut Control, msg: T) -> ::Result<()>;
}

pub trait Handle<T: Tx, R: Rx> {
    fn set_send_channel(&mut self, tx: Sender<T>);
    fn process_recv_channel(&mut self, msg: R) -> ::Result<()>;
}
