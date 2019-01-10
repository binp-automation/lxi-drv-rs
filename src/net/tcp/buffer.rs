use std::io::{self, Read, Write};


pub enum Buffer {
    Empty(EmptyBuffer),
    Occupied(OccupiedBuffer),
}

impl Buffer {
    pub fn new(capacity: usize) -> Buffer {
        Buffer::Empty(EmptyBuffer { data: Some(vec!(0; capacity)) })
    }
}

pub struct EmptyBuffer {
    data: Option<Vec<u8>>,
}

impl EmptyBuffer {
    pub fn read(mut self, r: &mut Read) -> io::Result<Buffer> {
        let n = r.read(self.data.as_mut().unwrap())?;
        if n > 0 {
            Ok(Buffer::Occupied(OccupiedBuffer {
                data: Some(self.data.take().unwrap()),
                begin: 0, end: n,
            }))
        } else {
            Ok(Buffer::Empty(self))
        }
    }
}

pub struct OccupiedBuffer {
    data: Option<Vec<u8>>,
    begin: usize,
    end: usize,
}

impl OccupiedBuffer {
    pub fn write(mut self, w: &mut Write) -> io::Result<Buffer> {
        let n = w.write(&self.data.as_ref().unwrap()[self.begin..self.end])?;
        if n > 0 {
            if n == self.end - self.begin {
                Ok(Buffer::Empty(EmptyBuffer {
                    data: Some(self.data.take().unwrap()),
                }))
            } else if n < self.end - self.begin {
                self.begin += n;
                Ok(Buffer::Occupied(self))
            } else {
                unreachable!()
            }
        } else {
            Ok(Buffer::Occupied(self))
        }
    }
}
