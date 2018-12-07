use std::sync::mpsc::TryRecvError;

use mio::*;
use mio_extras::channel::*;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    TryRecv(TryRecvError)
}

pub struct PollReceiver<T> {
    pub rx: Receiver<T>,
    pub poll: Poll,
    pub events: Events,
}

impl<T> PollReceiver<T> {
    pub fn new(rx: Receiver<T>) -> Result<PollReceiver<T>, Error> {
        let mut poll = Poll::new().map_err(|e| Error::Io(e))?;
        poll.register(
            &rx,
            Token(0),
            Ready::readable(),
            PollOpt::edge()
        ).map_err(|e| Error::Io(e))?;

        Ok(PollReceiver {
            rx, poll,
            events: Events::with_capacity(1),
        })
    }

    pub fn recv(&mut self) -> Result<T, Error> {
        match self.rx.try_recv() {
            Ok(msg) => Ok(msg),
            Err(err) => match err {
                TryRecvError::Empty => {
                    self.poll.poll(&mut self.events, None).map_err(|e| Error::Io(e))?;
                    match self.events.iter().next() {
                        Some(res) => assert!(res.token() == Token(0) && res.readiness().is_readable()),
                        None => unreachable!(),
                    };
                    
                    match self.rx.try_recv() {
                        Ok(msg) => Ok(msg),
                        Err(err) => match err {
                            TryRecvError::Empty => unreachable!(),
                            x => Err(Error::TryRecv(x)),
                        }
                    }
                }
                x => Err(Error::TryRecv(x)),
            }
        }
    }

    pub fn restore(self) -> Receiver<T> {
        self.rx
    }
}
