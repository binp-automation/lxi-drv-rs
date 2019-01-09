use std::net::{IpAddr};

use ::proxy::{BaseTx, BaseRx, UserTx, UserRx};

use ::net::layer::{self};
use ::net::dns::layer::{self as dns, Host};

use super::layer::{self as tcp};


#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Addr {
    pub host: Host,
    pub port: u16,
}

impl dns::Addr for Addr {
    type Inner = tcp::Addr;
    fn host(&self) -> &Host {
        &self.host
    }
    fn resolve(&self, ip_addr: &IpAddr) -> Self::Inner {
        tcp::Addr::new(ip_addr.clone(), self.port)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Opt {
    dns: dns::Opt,
    tcp: tcp::Opt,
}

impl layer::Push for tcp::Opt {
    type Elem = dns::Opt;
    type Outer = Opt;
    fn push(self, elem: Self::Elem) -> Self::Outer {
        Opt { dns: elem, tcp: self }
    }
}
impl layer::Pop for Opt {
    type Elem = dns::Opt;
    type Inner = tcp::Opt;
    fn pop(self) -> (Self::Elem, Self::Inner) {
        (self.dns, self.tcp)
    }
}

pub type Layer = dns::Layer<tcp::Addr, tcp::Opt, tcp::Layer, Addr, Opt>;


pub enum Tx {
    Base(BaseTx),
    Connect(Addr),
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
    Connected(Addr),
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
