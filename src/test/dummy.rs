use ::proxy::{Proxy, Control, Eid};
use ::proxy_handle::{self, Tx, Rx, ProxyWrapper, Handle, UserProxy, UserHandle};


pub struct DummyProxy {}

impl Proxy for DummyProxy {
	fn attach(&mut self, _ctrl: &Control) -> ::Result<()> {
		Ok(())
	}

    fn detach(&mut self, _ctrl: &Control) -> ::Result<()> {
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

pub fn create() -> ::Result<(ProxyWrapper<DummyProxy, Tx, Rx>, Handle<DummyHandle, Tx, Rx>)> {
	proxy_handle::create(DummyProxy {}, DummyHandle {})
}
