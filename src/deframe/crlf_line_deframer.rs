use std::io::{BufRead, Error, ErrorKind};

use crate::deframe::deframe::Deframe;
use crate::deframe::error::DeframingError;
use crate::deframe::error_take::ReadExt;

/// The maximum bytes that will be read for a CRLF line.
const MAX_LINE_SIZE: usize = 512;

/// Deframer for a CRLF terminated line.
pub struct CrlfLineDeframer {
    line: String
}

impl CrlfLineDeframer {
    /// Creates a new CRLF line deframer.
    pub fn new() -> CrlfLineDeframer {
        CrlfLineDeframer { line: String::new() }
    }
}

impl Deframe for CrlfLineDeframer {
    type Output = String;

    fn read(mut self, reader: &mut impl BufRead) -> Result<String, (Self, DeframingError)> {
        let mut reader = reader.error_take((MAX_LINE_SIZE - self.line.len()) as u64);

        match read_crlf_line(&mut reader, &mut self.line) {
            Ok(()) => Ok(self.line),
            Err(err) => Err((self, err))
        }
    }
}

/// Reads a CRLF line from the given reader, and writes it into line.
/// Data maybe be partially written into the line argument even if an error is encountered.
fn read_crlf_line(reader: &mut impl BufRead, line: &mut String) -> Result<(), DeframingError> {
    reader.read_line(line)?;

    if line.is_empty() {
        Err(Error::from(ErrorKind::UnexpectedEof).into())
    } else if let (Some('\n'), Some('\r')) = (line.pop(), line.pop()) { // pop the last two characters off and verify they're CRLF
        Ok(())
    } else {
        Err(DeframingError::BadSyntax)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Error, ErrorKind};

    use crate::deframe::crlf_line_deframer::CrlfLineDeframer;
    use crate::deframe::deframe::Deframe;
    use crate::deframe::error::DeframingError;
    use crate::deframe::error::DeframingError::{BadSyntax, Reading};
    use crate::util::mock::MockReader;

    fn test(tests: Vec<(Vec<&[u8]>, Result<&str, DeframingError>)>) {
        let mut reader = MockReader::from_bytes(vec![]);
        reader.return_would_block_when_empty = true;
        let mut reader = BufReader::new(reader);

        let mut deframer = Some(CrlfLineDeframer::new());

        for (new_data, expected_result) in tests {
            reader.get_mut().data.extend(new_data.into_iter().map(|v| v.to_vec()));

            deframer = match (deframer.take().unwrap().read(&mut reader), expected_result) {
                (Err((new, act)), Err(exp)) => {
                    assert_eq!(format!("{:?}", act), format!("{:?}", exp));
                    Some(new)
                }
                (act, exp) => {
                    assert_eq!(format!("{:?}", act.map_err(|(_, err)| err)), format!("{:?}", exp));
                    None
                }
            };
        }
    }

    #[test]
    fn full_line() {
        test(vec![
            (vec![b"hello there\r\n"], Ok("hello there"))
        ]);
    }

    #[test]
    fn multiple_full_lines_all_at_once() {
        test(vec![
            (vec![b"hello there\r\n", b"hello there 2\r\n", b"hello there 3\r\n"], Ok("hello there"))
        ]);
    }

    #[test]
    fn multiple_full_lines_fragmented_all_at_once() {
        test(vec![
            (vec![b"hello ", b"there\r", b"\n", b"hell", b"o the", b"re 2\r", b"\n", b"he", b"ll", b"o the", b"re 3", b"\r", b"\n"], Ok("hello there")),
        ]);
    }

    #[test]
    fn full_line_in_fragments() {
        test(vec![
            (vec![b"he", b"llo", b" there", b"\r", b"\n"], Ok("hello there"))
        ]);
    }

    #[test]
    fn partial_line() {
        test(vec![
            (vec![b"hello"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" there"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n"], Ok("hello  there")),
        ]);
    }

    #[test]
    fn partial_line_multiple_fragments() {
        test(vec![
            (vec![b"hel", b"lo"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" ", b"t"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"he", b"r", b"e"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r", b"\n"], Ok("hello there"))
        ]);
    }

    #[test]
    fn no_new_data_for_a_while() {
        test(vec![
            (vec![b"hel", b"lo"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r", b"\n"], Ok("hello"))
        ]);
    }

    #[test]
    fn missing_cr() {
        test(vec![
            (vec![b"hello"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" there"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n"], Err(BadSyntax)),
        ]);
    }

    #[test]
    fn missing_lf() {
        test(vec![
            (vec![b"hello"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" there"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
        ]);
    }

    #[test]
    fn missing_crlf_before_eof() {
        test(vec![
            (vec![b"hello"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" there"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b""], Err(BadSyntax))
        ]);
    }

    #[test]
    fn no_data_eof() {
        test(vec![
            (vec![b""], Err(Error::from(ErrorKind::UnexpectedEof).into()))
        ]);
    }

    #[test]
    fn no_data() {
        test(vec![
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into()))
        ]);
    }

    #[test]
    fn invalid_utf8() {
        let data = vec![0, 255, 2, 127, 4, 5, 3, 8];
        test(vec![
            (vec![&data], Err(Error::from(ErrorKind::WouldBlock).into()))
        ]);
    }

    #[test]
    fn invalid_utf8_with_crlf() {
        let data = vec![0, 255, 2, 127, 4, 5, 3, 8];
        test(vec![
            (vec![&data, b"\r\n"], Err(Error::new(ErrorKind::InvalidData, "stream did not contain valid UTF-8").into()))
        ]);
    }

    #[test]
    fn weird_line() {
        let data = b"r3984ty 98q39p8fuq p    9^\t%$\r%$@#!#@!%\r$%^%&%&*()_+|:{}>][/[\\/]3-062--=-9`~";
        test(vec![
            (vec![data], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Ok(String::from_utf8_lossy(data).to_string().as_str())),
        ]);
    }

    #[test]
    fn too_long() {
        let data = b" wrgiu hweiguhwepuiorgh w;eouirgh w;eoirugh ;weoug weroigj o;weirjg ;q\
        weroig pweoirg ;ewoirjhg; weoi";
        test(vec![
            (vec![data], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![data, data], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![data], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![data], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![data], Err(Error::new(ErrorKind::Other, "read limit reached").into())),
        ]);
    }
}