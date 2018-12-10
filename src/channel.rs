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
pub enum Error {
    Io(io::Error),
    Disconnected,
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
    pub events: mio::Events,
}

impl<'a, T> PollReceiver<'a, T> {
    pub fn new(rx: &'a Receiver<T>) -> Result<PollReceiver<T>, io::Error> {
        let poll = mio::Poll::new()?;
        poll.register(
            rx,
            mio::Token(0),
            mio::Ready::readable(),
            mio::PollOpt::edge()
        )?;

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

#[cfg(test)]
mod tests {
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
        let mut prx = PollReceiver::new(&rx).unwrap();

        tx.send(42 as i32).unwrap();
        let n = prx.recv().unwrap();
        
        assert_eq!(n, 42);
    }

    #[test]
    fn send_close_pollrecv() {
        let (tx, rx) = channel();
        let mut prx = PollReceiver::new(&rx).unwrap();

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
        let mut prx = PollReceiver::new(&rx).unwrap();

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
