use std::io::BufRead;

use crate::parse2::deframe::deframe::{Deframe, DeframerResult};

pub struct BytesUntilEofDeframer {
    data: Vec<u8>
}

impl BytesUntilEofDeframer {
    /// Creates a new line deframer.
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

    fn data_so_far(&self) -> &Vec<u8> {
        &self.data
    }
}