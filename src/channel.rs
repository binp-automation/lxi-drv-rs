use std::io;
use std::cell::{Cell};
use std::sync::mpsc::{self as std_chan};
use std::error::{Error as StdError};
use std::fmt;

use mio;
use mio_extras::channel::{self as mio_chan};


pub use self::mio_chan::SendError;
pub use self::std_chan::TryRecvError;

#[derive(Debug)]
pub enum RecvError {
    Io(io::Error),
    Disconnected,
}

impl StdError for RecvError {
    fn description(&self) -> &str {
        match self {
            RecvError::Io(e) => e.description(),
            RecvError::Disconnected => "Channel disconnected",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            RecvError::Io(e) => Some(e),
            RecvError::Disconnected => None,
        }
    }
}

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", (self as &StdError).description())
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Disconnected,
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::Io(e) => e.description(),
            Error::Disconnected => "Channel disconnected",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            Error::Io(e) => Some(e),
            Error::Disconnected => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", (self as &StdError).description())
    }
}

impl<T> From<SendError<T>> for Error {
    fn from(err: SendError<T>) -> Self {
        match err {
            SendError::Io(io_err) => Error::Io(io_err),
            SendError::Disconnected(_) => Error::Disconnected,
        }
    }
}

impl From<RecvError> for Error {
    fn from(err: RecvError) -> Self {
        match err {
            RecvError::Io(io_err) => Error::Io(io_err),
            RecvError::Disconnected => Error::Disconnected,
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
    pub events: Cell<Option<mio::Events>>,
}

impl<'a, T> PollReceiver<'a, T> {
    pub fn new(rx: &'a Receiver<T>) -> Result<PollReceiver<T>, Error> {
        let poll = mio::Poll::new().map_err(|e| Error::Io(e))?;
        poll.register(
            rx,
            mio::Token(0),
            mio::Ready::readable(),
            mio::PollOpt::edge()
        ).map_err(|e| Error::Io(e))?;

        Ok(PollReceiver {
            rx, poll,
            events: Cell::new(Some(mio::Events::with_capacity(1))),
        })
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        match self.rx.try_recv() {
            Ok(msg) => Ok(msg),
            Err(err) => match err {
                TryRecvError::Empty => {
                    let mut events = self.events.replace(None).unwrap();

                    let res = self.poll.poll(&mut events, None).map_err(|e| RecvError::Io(e)).and_then(|_| {
                        match events.iter().next() {
                            Some(res) => assert!(res.token() == mio::Token(0) && res.readiness().is_readable()),
                            None => unreachable!(),
                        }
                        match self.rx.try_recv() {
                            Ok(msg) => Ok(msg),
                            Err(err) => match err {
                                TryRecvError::Empty => unreachable!(),
                                TryRecvError::Disconnected => Err(RecvError::Disconnected),
                            }
                        }
                    });

                    self.events.replace(Some(events));

                    res
                }
                TryRecvError::Disconnected => Err(RecvError::Disconnected),
            }
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use std::thread;
    use std::time::{Duration};


    #[test]
    fn send_recv() {
        let (tx, rx) = channel();

        tx.send(42 as i32).unwrap();

        let mut n = None;
        for _ in 0..100 {
            let r = rx.try_recv();
            if let Err(TryRecvError::Empty) = r {
                thread::sleep(Duration::from_millis(1));
            } else {
                n = Some(r.unwrap());
                break;
            }
        }

        assert_eq!(n.unwrap(), 42);
    }

    #[test]
    fn send_pollrecv() {
        let (tx, rx) = channel();
        let prx = PollReceiver::new(&rx).unwrap();

        tx.send(42 as i32).unwrap();
        let n = prx.recv().unwrap();
        
        assert_eq!(n, 42);
    }

    #[test]
    fn send_close_pollrecv() {
        let (tx, rx) = channel();
        let prx = PollReceiver::new(&rx).unwrap();

        thread::spawn(move || {
            tx.send(42 as i32).unwrap();
        });
        thread::sleep(Duration::from_millis(10));

        let n = prx.recv().unwrap();
        assert_eq!(n, 42);

        if let Err(RecvError::Disconnected) = prx.recv() {
            // ok
        } else {
            panic!();
        }
    }

    #[test]
    fn send_pollrecv_close() {
        let (tx, rx) = channel();
        let prx = PollReceiver::new(&rx).unwrap();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            tx.send(42 as i32).unwrap();
        });

        let n = prx.recv().unwrap();
        assert_eq!(n, 42);

        if let Err(RecvError::Disconnected) = prx.recv() {
            // ok
        } else {
            panic!();
        }
    }

    #[test]
    fn close_send() {
        let tx = channel().0;

        if let Err(SendError::Disconnected(n)) = tx.send(42 as i32) {
            assert_eq!(n, 42);
        } else {
            panic!();
        }
    }

}
