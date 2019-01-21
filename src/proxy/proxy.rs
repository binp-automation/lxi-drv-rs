use super::control;


pub trait Proxy {
    fn attach(&mut self, ctrl: &mut control::Attach) -> ::Result<()>;
    fn detach(&mut self, ctrl: &mut control::Detach) -> ::Result<()>;

    fn process(&mut self, ctrl: &mut control::Process) -> ::Result<()>;
    fn commit(&mut self, ctrl: &mut control::Process) -> ::Result<()>;
}


#[cfg(test)]
mod test {
	struct A {
		a: i32,
	}

	impl A {
		fn get(&self) -> i32 {
			self.a
		}
	}

	#[test]
	fn store_method() {
		let a = A { a: 12 };
		let b = A { a: 34 };
		assert_eq!(a.get(), 12);
		assert_eq!(A::get(&a), 12);
		assert_eq!(A::get(&b), 34);
		let get = A::get;
		assert_eq!(get(&b), 34);
	}
}