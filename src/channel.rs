#[cfg(test)]
#[path = "./tests/channel.rs"]
mod tests;


use std::io;
use std::sync::mpsc::{self as std_chan};

use mio;
use mio_extras::channel::{self as mio_chan};


pub use self::mio_chan::SendError;
pub use self::std_chan::TryRecvError;

#[derive(Debug)]
pub enum RecvError {
    Io(io::Error),
    Disconnected,
}

#[derive(Debug)]
pub enum ChanError {
    Io(io::Error),
    Disconnected,
}

impl<T> From<SendError<T>> for ChanError {
    fn from(err: SendError<T>) -> Self {
        match err {
            SendError::Io(io_err) => ChanError::Io(io_err),
            SendError::Disconnected(_) => ChanError::Disconnected,
        }
    }
}

impl From<RecvError> for ChanError {
    fn from(err: RecvError) -> Self {
        match err {
            RecvError::Io(io_err) => ChanError::Io(io_err),
            RecvError::Disconnected => ChanError::Disconnected,
        }
    }
}

impl RecvError {
    pub fn from_try(err: TryRecvError) -> Option<Self> {
        match err {
            TryRecvError::Disconnected => Some(RecvError::Disconnected),
            TryRecvError::Empty => None,
        }
    }
}

pub use self::mio_chan::{Sender, Receiver};
pub use self::mio_chan::channel;

pub struct PollReceiver<'a, T: 'a> {
    pub rx: &'a Receiver<T>,
    pub poll: mio::Poll,
    pub events: mio::Events,
}

impl<'a, T> PollReceiver<'a, T> {
    pub fn new(rx: &'a Receiver<T>) -> Result<PollReceiver<T>, ChanError> {
        let poll = mio::Poll::new().map_err(|e| ChanError::Io(e))?;
        poll.register(
            rx,
            mio::Token(0),
            mio::Ready::readable(),
            mio::PollOpt::edge()
        ).map_err(|e| ChanError::Io(e))?;

        Ok(PollReceiver {
            rx, poll,
            events: mio::Events::with_capacity(1),
        })
    }

    pub fn recv(&mut self) -> Result<T, RecvError> {
        match self.rx.try_recv() {
            Ok(msg) => Ok(msg),
            Err(err) => match err {
                TryRecvError::Empty => {
                    self.poll.poll(&mut self.events, None).map_err(|e| RecvError::Io(e))?;
                    match self.events.iter().next() {
                        Some(res) => assert!(res.token() == mio::Token(0) && res.readiness().is_readable()),
                        None => unreachable!(),
                    };
                    
                    match self.rx.try_recv() {
                        Ok(msg) => Ok(msg),
                        Err(err) => match err {
                            TryRecvError::Empty => unreachable!(),
                            TryRecvError::Disconnected => Err(RecvError::Disconnected),
                        }
                    }
                }
                TryRecvError::Disconnected => Err(RecvError::Disconnected),
            }
        }
    }
}
