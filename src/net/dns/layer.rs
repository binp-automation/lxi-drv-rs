use std::net::{IpAddr};
use std::time::{Duration};

use ::proxy::{
    RawProxy,
    AttachControl, DetachControl, ProcessControl,
};

use ::net::layer::{self as l};


#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Host {
    Dns(String),
    Ip(IpAddr),
}

pub trait Addr {
    type Inner;
    fn host(&self) -> &Host;
    fn resolve(&self, ip_addr: &IpAddr) -> Self::Inner;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RecnOpt {
    pub resolve_once: bool,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Opt {
    pub reconnect: Option<RecnOpt>,
}

impl Default for Opt {
    fn default() -> Self {
        Self { reconnect: None }
    }
}


pub struct Layer<
    IA: Clone,
    IO: l::Push<Elem=Opt, Outer=O> + Clone,
    IL: l::Layer<Addr=IA, Opt=IO>,
    A: Addr<Inner=IA>,
    O: l::Pop<Elem=Opt, Inner=IO>,
> {
    pub opt: Opt,
    pub addr: Option<A>,
    pub inner: IL,
}

impl<
    IA: Clone,
    IO: l::Push<Elem=Opt, Outer=O> + Clone,
    IL: l::Layer<Addr=IA, Opt=IO>,
    A: Addr<Inner=IA>,
    O: l::Pop<Elem=Opt, Inner=IO>,
> Layer<IA, IO, IL, A, O> {
    pub fn new(inner: IL) -> Self {
        Self {
            opt: Opt::default(),
            addr: None,
            inner,
        }
    }
}

impl<
    IA: Clone,
    IO: l::Push<Elem=Opt, Outer=O> + Clone,
    IL: l::Layer<Addr=IA, Opt=IO>,
    A: Addr<Inner=IA>,
    O: l::Pop<Elem=Opt, Inner=IO>,
> RawProxy for Layer<IA, IO, IL, A, O> {
    fn attach(&mut self, _ctrl: &mut AttachControl) -> ::Result<()> {
        if self.addr.is_none() || self.inner.is_connected() {
            Err(::net::Error::AlreadyConnected.into())
        } else {
            Ok(())
        }
    }

    fn detach(&mut self, ctrl: &mut DetachControl) -> ::Result<()> {
        self.inner.try_disconnect(ctrl)
    }

    fn process(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        self.inner.process(ctrl)
    }

    fn commit(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        self.inner.commit(ctrl)
    }
}

impl<
    IA: Clone,
    IO: l::Push<Elem=Opt, Outer=O> + Clone,
    IL: l::Layer<Addr=IA, Opt=IO>,
    A: Addr<Inner=IA> + Clone,
    O: l::Pop<Elem=Opt, Inner=IO> + Clone,
> l::Layer for Layer<IA, IO, IL, A, O> {
    type Addr = A;
    type Opt = O;

    fn opt(&self) -> Self::Opt {
        self.inner.opt().push(self.opt.clone())
    }

    fn set_opt(&mut self, opt: Self::Opt) {
        let (o, io) = opt.pop();
        self.opt = o;
        self.inner.set_opt(io);
    }
    
    fn connect(&mut self, ctrl: &mut AttachControl, addr: A) -> ::Result<()> {
        if self.is_connected() {
            return Err(::net::Error::AlreadyConnected.into());
        }

        let res = match addr.host() {
            Host::Dns(_) => unimplemented!(),
            Host::Ip(ip_addr) => self.inner.connect(ctrl, addr.resolve(ip_addr)),
        };

        self.addr = Some(addr);

        res
    }

    fn disconnect(&mut self, ctrl: &mut DetachControl) -> ::Result<()> {
        self.addr = None;
        self.inner.try_disconnect(ctrl)
    }

    fn addr(&self) -> Option<&Self::Addr> {
        self.addr.as_ref()
    }

    fn process(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        Ok(())
    }

    fn commit(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        Ok(())
    }
}
