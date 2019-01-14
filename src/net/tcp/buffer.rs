use std::io::{self, Read, Write};


pub struct Buffer {
    pub data: Vec<u8>,
    pub begin: usize,
    pub end: usize,
}

impl Buffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec!(0; capacity),
            begin: 0,
            end: 0,
        }
    }
}

impl Buffer {
    pub fn read(&mut self, r: &mut Read) -> io::Result<usize> {
        match r.read(&mut self.data[self.end..self.data.len()]) {
            Ok(n) => {
                self.end += n;
                debug_assert!(self.end <= self.data.len());
                Ok(n)
            },
            e @ Err(_) => e,
        }

    }

    pub fn write(&mut self, w: &mut Write) -> io::Result<usize> {
        match w.write(&self.data[self.begin..self.end]) {
            Ok(n) => {
                self.begin += n;
                debug_assert!(self.begin <= self.end);
                if self.begin == self.end {
                    self.begin = 0;
                    self.end = 0;
                }
                Ok(n)
            },
            e @ Err(_) => e,
        }
    }
}
