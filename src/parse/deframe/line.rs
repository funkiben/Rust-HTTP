use std::io::{BufRead, Error, ErrorKind};

use crate::parse::deframe::deframe::{Deframe, DeframerResult};

/// A parser for a '\n' terminated line.
/// If no data is read and EOF is reached then None is returned.
/// If EOF is returned before '\n' then an UnexpectedEof error is returned.
pub struct LineOrEofDeframer {
    line: String
}

impl LineOrEofDeframer {
    pub fn new() -> LineOrEofDeframer {
        LineOrEofDeframer { line: String::new() }
    }
}

impl Deframe<Option<String>> for LineOrEofDeframer {
    fn read(mut self, reader: &mut impl BufRead) -> DeframerResult<Option<String>, Self> {
        match reader.read_line(&mut self.line) {
            Ok(0) if self.line.is_empty() => Ok(None),
            Ok(_) =>
                if let Some('\n') = self.line.pop() {
                    Ok(Some(self.line))
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