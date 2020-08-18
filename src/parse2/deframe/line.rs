use std::io::{BufRead, Error, ErrorKind};

use crate::parse2::deframe::deframe::{Deframe, DeframerResult};

/// Deframer for a '\n' sequence of UTF-8 bytes.
pub struct LineDeframer {
    line: String
}

impl LineDeframer {
    /// Creates a new line deframer.
    pub fn new() -> LineDeframer {
        LineDeframer { line: String::new() }
    }
}

impl Deframe<String> for LineDeframer {
    fn read(mut self, reader: &mut impl BufRead) -> DeframerResult<String, Self> {
        match reader.read_line(&mut self.line) {
            Ok(_) =>
                if let Some('\n') = self.line.pop() {
                    Ok(self.line)
                } else {
                    Err((self, Error::from(ErrorKind::UnexpectedEof)))
                },
            Err(err) => Err((self, err))
        }
    }

    fn data_so_far(&self) -> &String {
        &self.line
    }
}