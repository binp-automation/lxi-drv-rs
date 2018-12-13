use mio::*;

use ::error::*;


pub type ProxyId = usize;

pub const PROXY_RANGE_BITS: usize = 8;
pub const PROXY_RANGE_MASK: usize = (1<<PROXY_RANGE_BITS) - 1;

pub fn proxy_id(token: &Token) -> ProxyId {
    token.0 >> PROXY_RANGE_BITS
}

#[derive(Debug)]
pub enum ProxyAction {
    Detach,
    Nope,
}

pub trait Proxy {
    fn register(&mut self, poll: &Poll) -> Result<()>;
    fn deregister(&mut self, poll: &Poll) -> Result<()>;
    fn process(&mut self, poll: &Poll, event: &Event) -> Result<ProxyAction>;
}
