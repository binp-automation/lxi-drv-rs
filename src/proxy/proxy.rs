use super::control;


pub trait Proxy {
    fn attach(&mut self, ctrl: &mut control::Attach) -> ::Result<()>;
    fn detach(&mut self, ctrl: &mut control::Detach) -> ::Result<()>;

    fn process(&mut self, ctrl: &mut control::Process) -> ::Result<()>;
}
