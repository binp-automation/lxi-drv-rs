use std::net::IpAddr;

use mio_extras::channel::Sender;

use ::proxy::{control, Proxy, BaseTx, BaseRx, UserTx, UserRx};

use super::{layer as tcp, dns};


pub enum Tx {
    Base(BaseTx),
    Connect(dns::Addr),
    Disconnect,
    SetOpt(dns::Opt),
}

impl From<BaseTx> for Tx {
    fn from(base: BaseTx) -> Self {
        Tx::Base(base)
    }
}
impl Into<Result<BaseTx, Tx>> for Tx {
    fn into(self) -> Result<BaseTx, Tx> {
        match self {
            Tx::Base(base) => Ok(base),
            other => Err(other),
        }
    }
}
impl UserTx for Tx {}

pub enum Rx {
    Base(BaseRx),
    DnsResolved(tcp::Addr),
    Connected(IpAddr),
    Disconnected,
}

impl From<BaseRx> for Rx {
    fn from(base: BaseRx) -> Self {
        Rx::Base(base)
    }
}
impl Into<Result<BaseRx, Rx>> for Rx {
    fn into(self) -> Result<BaseRx, Rx> {
        match self {
            Rx::Base(base) => Ok(base),
            other => Err(other),
        }
    }
}
impl UserRx for Rx {}


impl Proxy<Tx, Rx> for dns::Layer {
    fn set_send_channel(&mut self, tx: Sender<Rx>) {

    }
    fn process_recv_channel(&mut self, ctrl: &mut control::Process, msg: Tx) -> ::Result<()> {
        Ok(())
    }
}
