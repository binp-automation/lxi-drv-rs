use std::io;
use std::time::{Duration};
use std::sync::mpsc::{self as std_chan};
use std::error::{Error as StdError};
use std::fmt;

use mio;
use mio_extras::channel::{self as mio_chan};


pub use self::mio_chan::SendError;
pub use self::std_chan::TryRecvError;

pub use self::mio_chan::{Sender, Receiver};
pub use self::mio_chan::channel;

#[derive(Debug)]
pub enum RecvError {
    Io(io::Error),
    Disconnected,
    Empty,
}

impl StdError for RecvError {
    fn description(&self) -> &str {
        match self {
            RecvError::Io(e) => e.description(),
            RecvError::Disconnected => "Channel disconnected",
            RecvError::Empty => "Channel empty",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            RecvError::Io(e) => Some(e),
            RecvError::Disconnected => None,
            RecvError::Empty => None,
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
    Empty,
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::Io(e) => e.description(),
            Error::Disconnected => "Channel disconnected",
            Error::Empty => "Channel empty",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            Error::Io(e) => Some(e),
            Error::Disconnected => None,
            Error::Empty => None,
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
            RecvError::Empty => Error::Empty,
        }
    }
}

impl From<TryRecvError> for Error {
    fn from(err: TryRecvError) -> Self {
        match err {
            TryRecvError::Disconnected => Error::Disconnected,
            TryRecvError::Empty => Error::Empty,
        }
    }
}


pub struct SinglePoll {
    pub poll: mio::Poll,
    pub events: mio::Events,
}

impl SinglePoll {
    pub fn new<T>(rx: &Receiver<T>) -> Result<Self, Error> {
        let poll = mio::Poll::new().map_err(|e| Error::Io(e))?;
        poll.register(
            rx,
            mio::Token(0),
            mio::Ready::readable(),
            mio::PollOpt::edge()
        ).map_err(|e| Error::Io(e))?;
        let events = mio::Events::with_capacity(1);

        Ok(Self { poll, events })
    }

    pub fn wait(&mut self, timeout: Option<Duration>) -> Result<(), RecvError> {
        self.poll.poll(&mut self.events, timeout).map_err(|e| RecvError::Io(e)).and_then(|_| {
            match self.events.iter().next() {
                Some(res) => {
                    assert!(res.token() == mio::Token(0) && res.readiness().is_readable());
                    Ok(())
                },
                None => Err(RecvError::Empty),
            }
        })
    }
}

pub struct PollReceiver<'a, T: 'a> {
    pub rx: &'a Receiver<T>,
    pub poll: SinglePoll,
}

impl<'a, T> PollReceiver<'a, T> {
    pub fn new(rx: &'a Receiver<T>) -> Result<Self, Error> {
        Ok(Self { rx, poll: SinglePoll::new(&rx)? })
    }

    pub fn wait(&mut self, timeout: Option<Duration>) -> Result<(), RecvError> {
        self.poll.wait(timeout)
    }

    pub fn recv(&mut self, timeout: Option<Duration>) -> Result<T, RecvError> {
        match self.rx.try_recv() {
            Ok(msg) => Ok(msg),
            Err(err) => match err {
                TryRecvError::Empty => self.wait(timeout).and_then(|_| {
                    match self.rx.try_recv() {
                        Ok(msg) => Ok(msg),
                        Err(err) => match err {
                            TryRecvError::Empty => unreachable!(),
                            TryRecvError::Disconnected => Err(RecvError::Disconnected),
                        }
                    }
                }),
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

    use mio::{Poll, Events, Token, Ready, PollOpt};


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
        let mut prx = PollReceiver::new(&rx).unwrap();

        tx.send(42 as i32).unwrap();
        let n = prx.recv(None).unwrap();
        
        assert_eq!(n, 42);
    }

    #[test]
    fn send_close_poll_recv() {
        let (tx, rx) = channel();
        let mut prx = PollReceiver::new(&rx).unwrap();

        thread::spawn(move || {
            tx.send(42 as i32).unwrap();
        });
        thread::sleep(Duration::from_millis(10));

        let n = prx.recv(None).unwrap();
        assert_eq!(n, 42);

        assert_matches!(prx.recv(None), Err(RecvError::Disconnected));
    }

    #[test]
    fn send_poll_recv_close() {
        let (tx, rx) = channel();
        let mut prx = PollReceiver::new(&rx).unwrap();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            tx.send(42 as i32).unwrap();
        });

        let n = prx.recv(None).unwrap();
        assert_eq!(n, 42);

        assert_matches!(prx.recv(None), Err(RecvError::Disconnected));
    }

    #[test]
    fn send_close_poll_wait() {
        let (tx, rx) = channel();
        let mut prx = PollReceiver::new(&rx).unwrap();

        thread::spawn(move || {
            tx.send(42 as i32).unwrap();
        });
        thread::sleep(Duration::from_millis(10));

        let n = prx.recv(None).unwrap();
        assert_eq!(n, 42);

        assert_matches!(prx.wait(None), Ok(()));
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

    #[test]
    fn multiple_polls() {
        let (_tx, rx) = channel::<i32>();

        PollReceiver::new(&rx).unwrap();
        assert_matches!(PollReceiver::new(&rx).err(), Some(Error::Io(_)));
    }

    #[test]
    fn poll_before() {
        let (tx, rx) = channel::<u32>();
        let poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(16);

        tx.send(1).unwrap();
        thread::sleep(Duration::from_millis(10));

        poll.register(&rx, Token(0), Ready::readable(), PollOpt::edge()).unwrap();

        poll.poll(&mut events, Some(Duration::from_millis(10))).unwrap();
        let mut hdl = false;
        for e in events.iter() {
            assert_eq!(e.token().0, 0);
            assert!(e.readiness().is_readable());
            assert_eq!(rx.try_recv().unwrap(), 1);
            hdl = true;
        }
        assert!(hdl);
    }

    #[test]
    fn poll_after() {
        let (tx, rx) = channel::<u32>();
        let poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(16);

        poll.register(&rx, Token(0), Ready::readable(), PollOpt::edge()).unwrap();

        tx.send(1).unwrap();
        thread::sleep(Duration::from_millis(10));

        poll.poll(&mut events, Some(Duration::from_millis(10))).unwrap();
        assert!(events.iter().next().is_some());

        poll.poll(&mut events, Some(Duration::from_millis(10))).unwrap();
        assert!(events.iter().next().is_none());

        assert_eq!(rx.try_recv().unwrap(), 1);
        thread::sleep(Duration::from_millis(10));

        poll.poll(&mut events, Some(Duration::from_millis(10))).unwrap();
        assert!(events.iter().next().is_none());

        tx.send(2).unwrap();
        thread::sleep(Duration::from_millis(10));

        poll.poll(&mut events, Some(Duration::from_millis(10))).unwrap();
        assert!(events.iter().next().is_some());
    }

    #[test]
    fn poll_double() {
        let (tx, rx) = channel::<u32>();
        let poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(16);

        tx.send(1).unwrap();
        thread::sleep(Duration::from_millis(10));

        poll.register(&rx, Token(0), Ready::readable(), PollOpt::edge()).unwrap();

        poll.poll(&mut events, Some(Duration::from_millis(10))).unwrap();

        tx.send(2).unwrap();
        thread::sleep(Duration::from_millis(10));

        let mut hdl = false;
        for e in events.iter() {
            assert_eq!(e.token().0, 0);
            assert!(e.readiness().is_readable());
            assert_eq!(rx.try_recv().unwrap(), 1);
            assert_eq!(rx.try_recv().unwrap(), 2);
            hdl = true;
        }
        assert!(hdl);

        tx.send(3).unwrap();
        thread::sleep(Duration::from_millis(10));

        poll.poll(&mut events, Some(Duration::from_millis(10))).unwrap();
        assert!(events.iter().next().is_some());

        tx.send(4).unwrap();
        thread::sleep(Duration::from_millis(10));

        poll.poll(&mut events, Some(Duration::from_millis(10))).unwrap();
        assert!(events.iter().next().is_none());
    }

    #[test]
    fn poll_oneshot() {
        let (tx, rx) = channel::<u32>();
        let poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(16);

        tx.send(1).unwrap();
        thread::sleep(Duration::from_millis(10));

        poll.register(
            &rx, Token(0), Ready::readable(), 
            PollOpt::edge() | PollOpt::oneshot()
        ).unwrap();

        poll.poll(&mut events, Some(Duration::from_millis(10))).unwrap();
        let mut hdl = false;
        for e in events.iter() {
            assert_eq!(e.token().0, 0);
            assert!(e.readiness().is_readable());
            assert_eq!(rx.try_recv().unwrap(), 1);
            hdl = true;
        }
        assert!(hdl);

        tx.send(2).unwrap();
        thread::sleep(Duration::from_millis(10));

        poll.reregister(
            &rx, Token(0), Ready::readable(), 
            PollOpt::edge() | PollOpt::oneshot()
        ).unwrap();

        let mut hdl = false;
        for e in events.iter() {
            assert_eq!(e.token().0, 0);
            assert!(e.readiness().is_readable());
            assert_eq!(rx.try_recv().unwrap(), 2);
            hdl = true;
        }
        assert!(hdl);
    }
}
