use ::channel::{Sender};
use ::error::{IdError};
use ::proxy::{
    RawProxy,
    AttachControl, DetachControl, ProcessControl,
    BaseTx, BaseRx, UserTx, UserRx,
    Eid, Evented, EventedWrapper as Ew, EID_CHAN_RX,
};



impl RawProxy for Layer {
    fn attach(&mut self, ctrl: &mut AttachControl) -> ::Result<()> {
        match self.sock {
            Some(ref sock) => ctrl.register(sock, Ready::all()),
            None => Ok(()),
        }
    }

    fn detach(&mut self, ctrl: &mut DetachControl) -> ::Result<()> {
        match self.sock {
            Some(ref sock) => ctrl.deregister(sock),
            None => Ok(()),
        }
    }
    fn process(&mut self, ctrl: &mut ProcessControl) -> ::Result<()> {
        match ctrl.id() {
            EID_CHAN_RX => self.process_channel(ctrl),
            EID_TCP_SOCK => self.process_socket(ctrl),
            other => Err(IdError::BadId.into()),
        }
    }
}

pub enum Tx {
    Base(BaseTx),
    Connect(Addr),
    Disconnect,
    SetOpt(Opt),
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

impl UserProxy for Layer {
    fn set_send_channel(&mut self, tx: Sender<R>) {

    }

    fn process_recv_channel(&mut self, ctrl: &mut control::Process, msg: T) -> ::Result<()> {

    }
}