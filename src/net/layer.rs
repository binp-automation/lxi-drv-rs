use ::proxy::{AttachControl, DetachControl, ProcessControl};

pub trait Pop: Sized {
    type Elem;
    type Inner: Push<Elem=Self::Elem, Outer=Self>;
    fn pop(self) -> (Self::Elem, Self::Inner);
}

pub trait Push: Sized {
    type Elem;
    type Outer: Pop<Elem=Self::Elem, Inner=Self>;
    fn push(self, elem: Self::Elem) -> Self::Outer;
}

pub trait Layer {
    type Addr: Clone;
    type Opt: Clone;

    fn opt(&self) -> Self::Opt;
    fn set_opt(&mut self, opt: Self::Opt);

    fn connect(&mut self, ctrl: &mut AttachControl, addr: Self::Addr) -> ::Result<()>;
    fn disconnect(&mut self, ctrl: &mut DetachControl) -> ::Result<()>;

    fn addr(&self) -> Option<&Self::Addr>;

    fn reconnect(&mut self, ctrl: &mut AttachControl, addr: Option<Self::Addr>) -> ::Result<()> {
        match addr {
            Some(addr) => Ok(addr),
            None => match self.addr() {
                Some(addr) => Ok((*addr).clone()),
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

    fn try_disconnect(&mut self, ctrl: &mut DetachControl) -> ::Result<()> {
        match self.is_connected() {
            true => self.disconnect(ctrl),
            false => Ok(()),
        }
    }

    fn process(&mut self, ctrl: &mut ProcessControl) -> ::Result<()>;
    fn commit(&mut self, ctrl: &mut ProcessControl) -> ::Result<()>;
}

pub trait Notifier {
    type Msg;
    fn notify(&mut self, msg: Self::Msg) -> ::Result<()>;
}

impl<T> Notifier for ::channel::Sender<T> {
    type Msg = T;
    fn notify(&mut self, msg: T) -> ::Result<()> {
        self.send(msg).map_err(|e| ::channel::Error::from(e).into())
    }
}