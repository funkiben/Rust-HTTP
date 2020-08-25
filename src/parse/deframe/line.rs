use std::io::{BufRead, Error, ErrorKind};

use crate::parse::deframe::deframe::{Deframe, DeframerResult};

/// A parser for a '\n' terminated line.
/// If EOF is returned before '\n' then an UnexpectedEof error is returned.
pub struct LineDeframer {
    line: String
}

impl LineDeframer {
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


    fn read_so_far(&self) -> usize {
        self.line.len()
    }
}