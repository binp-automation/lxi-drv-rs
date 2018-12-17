use proxy::{Proxy, Control, Eid};
use channel_proxy::{self, Tx, Rx, UserProxy, UserHandle, ChannelProxy, ChannelHandle};


pub struct DummyProxy {}

impl Proxy for DummyProxy {
	fn attach(&mut self, _ctrl: &mut Control) -> ::Result<()> {
		Ok(())
	}

    fn detach(&mut self, _ctrl: &mut Control) -> ::Result<()> {
    	Ok(())
    }

    fn process(&mut self, _ctrl: &mut Control, _readiness: mio::Ready, _eid: Eid) -> ::Result<()> {
    	Ok(())
    }
}

impl UserProxy<Tx, Rx> for DummyProxy {
	fn process_channel(&mut self, _ctrl: &mut Control, _msg: Tx) -> ::Result<()> {
		Ok(())
	}
}

pub struct DummyHandle {}

impl UserHandle<Tx, Rx> for DummyHandle {}

pub fn create() -> ::Result<(ChannelProxy<DummyProxy, Tx, Rx>, ChannelHandle<DummyHandle, Tx, Rx>)> {
	channel_proxy::create(DummyProxy {}, DummyHandle {})
}
