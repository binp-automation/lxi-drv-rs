use std::collections::{VecDeque};

use ::proxy::{Proxy, Control, Eid};
use ::proxy_handle::{self, ProxyWrapper, Handle, UserProxy, UserHandle};

pub use proxy_handle::{Tx, Rx};

pub struct DummyProxy {}

impl DummyProxy {
	fn new() -> Self {
		Self {}
	}
}

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

pub struct DummyHandle {
	messages: VecDeque<Rx>,
}

impl DummyHandle {
	fn new() -> Self {
		Self {
			messages: VecDeque::new(),
		}
	}
}

impl UserHandle<Tx, Rx> for DummyHandle {
	fn process_channel(&mut self, msg: Rx) -> ::Result<()> {
		self.messages.push_back(msg);
		Ok(())
	}
}

pub fn create() -> ::Result<(ProxyWrapper<DummyProxy, Tx, Rx>, Handle<DummyHandle, Tx, Rx>)> {
	proxy_handle::create(DummyProxy::new(), DummyHandle::new())
}
