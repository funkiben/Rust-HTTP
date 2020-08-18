use std::io::{BufRead, Error, ErrorKind};

use crate::parse2::deframe::deframe::{Deframe, DeframerResult};

pub struct BytesDeframer {
    data: Vec<u8>,
    pos: usize,
}

impl BytesDeframer {
    pub fn new(size: usize) -> BytesDeframer {
        BytesDeframer { data: vec![0; size], pos: 0 }
    }
}

impl Deframe<Vec<u8>> for BytesDeframer {
    fn read(mut self, reader: &mut impl BufRead) -> DeframerResult<Vec<u8>, Self> {
        loop {
            let len = self.data.len();
            let mut buf = &mut self.data[self.pos..len];

            match reader.read(&mut buf) {
                Ok(0) if buf.len() != 0 => return Err((self, Error::from(ErrorKind::UnexpectedEof))),
                Ok(amt) => {
                    self.pos += amt;
                    if self.pos == len {
                        return Ok(self.data);
                    }
                }
                Err(err) => return Err((self, err.into()))
            }
        }
    }

    fn data_so_far(&self) -> &Vec<u8> {
        &self.data
    }
}