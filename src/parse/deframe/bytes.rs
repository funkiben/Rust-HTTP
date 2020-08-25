use std::io::{BufRead, Error, ErrorKind};

use crate::parse::deframe::deframe::{Deframe, DeframerResult};

/// Deframer for a specified number of bytes.
pub struct BytesDeframer {
    data: Vec<u8>,
    pos: usize,
}

impl BytesDeframer {
    /// Creates a new deframer for deframing the specified number of bytes.
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

    fn read_so_far(&self) -> usize {
        self.data.len()
    }
}

/// Deframer for all remaining bytes until EOF is found.
pub struct BytesUntilEofDeframer {
    data: Vec<u8>
}

impl BytesUntilEofDeframer {
    /// Creates a bytes-until-EOF deframer.
    pub fn new() -> BytesUntilEofDeframer {
        BytesUntilEofDeframer { data: vec![] }
    }
}

impl Deframe<Vec<u8>> for BytesUntilEofDeframer {
    fn read(mut self, reader: &mut impl BufRead) -> DeframerResult<Vec<u8>, Self> {
        match reader.read_to_end(&mut self.data) {
            Ok(_) => Ok(self.data),
            Err(err) => Err((self, err))
        }
    }

    fn read_so_far(&self) -> usize {
        self.data.len()
    }
}