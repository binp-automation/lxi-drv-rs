use ::proxy::{Proxy, Control};


pub trait Addr: Clone {

}

pub trait Opt: Clone {

}

pub trait OptPop: Opt {
    type Opt: Opt;
    type Inner: OptPush<Opt=Self::Opt, Outer=Self>;
    fn pop(self) -> (Self::Opt, Self::Inner);
}

pub trait OptPush: Opt {
    type Opt: Opt;
    type Outer: OptPop<Opt=Self::Opt, Inner=Self>;
    fn push(self, opt: Self::Opt) -> Self::Outer;
}


pub trait Layer: Proxy {
    type Addr: Addr;
    type Opt: Opt;

    fn opt(&self) -> Self::Opt;
    fn set_opt(&mut self, opt: Self::Opt);

    fn connect(&mut self, ctrl: &mut Control, addr: Self::Addr) -> ::Result<()>;
    fn disconnect(&mut self, ctrl: &Control) -> ::Result<()>;

    fn addr(&self) -> Option<&Self::Addr>;

    fn reconnect(&mut self, ctrl: &mut Control, addr: Option<Self::Addr>) -> ::Result<()> {
        match addr {
            Some(addr) => Ok(addr),
            None => match self.addr() {
                Some(addr) => Ok(addr.clone()),
                None => Err(super::Error::NotConnected.into()),
            },
        }.and_then(|addr| {
            self.disconnect(ctrl).map(|_| addr)
        }).and_then(|addr| {
            self.connect(ctrl, addr)
        })
    }

    fn is_connected(&self) -> bool {
        self.addr().is_some()
    }

    fn try_disconnect(&mut self, ctrl: &Control) -> ::Result<()> {
        match self.is_connected() {
            true => self.disconnect(ctrl),
            false => Ok(()),
        }
    }
}
