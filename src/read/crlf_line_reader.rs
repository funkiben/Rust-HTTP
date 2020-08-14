use std::io::{BufRead, Error, ErrorKind};

use crate::read::error::ParsingError;
use crate::read::error_take::ErrorTake;

const MAX_LINE_SIZE: usize = 512;

pub struct CrlfLineReader {
    line: Option<String>
}

impl CrlfLineReader {
    pub fn new() -> CrlfLineReader {
        CrlfLineReader {
            line: Some(String::new())
        }
    }

    pub fn read(&mut self, reader: &mut impl BufRead) -> Result<Option<String>, ParsingError> {
        let line = self.line.as_mut().unwrap();
        let mut reader = reader.error_take((MAX_LINE_SIZE - line.len()) as u64);

        if read_crlf_line(&mut reader, line)? {
            return Ok(self.line.replace(String::new()));
        }

        Ok(None)
    }
}

fn read_crlf_line(reader: &mut impl BufRead, line: &mut String) -> Result<bool, ParsingError> {
    match reader.read_line(line) {
        Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(false),
        Err(err) => Err(err.into()),
        Ok(_) => {
            if line.is_empty() {
                Err(Error::from(ErrorKind::UnexpectedEof).into())
            } else if let (Some('\n'), Some('\r')) = (line.pop(), line.pop()) { // pop the last two characters off and verify they're CRLF
                Ok(true)
            } else {
                Err(ParsingError::BadSyntax)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Error, ErrorKind};

    use crate::read::crlf_line_reader::CrlfLineReader;
    use crate::read::error::ParsingError;
    use crate::read::error::ParsingError::BadSyntax;
    use crate::util::mock::MockReader;

    fn test_read(tests: Vec<(Vec<&[u8]>, Result<Option<&str>, ParsingError>)>) {
        let mut reader = MockReader::from_bytes(vec![]);
        reader.return_would_block_when_empty = true;
        let mut reader = BufReader::new(reader);
        let mut line_reader = CrlfLineReader::new();
        for (new_data, expected_result) in tests {
            reader.get_mut().data.extend(new_data.into_iter().map(|v| v.to_vec()));
            let actual_result = line_reader.read(&mut reader);
            assert_eq!(format!("{:?}", actual_result), format!("{:?}", expected_result));
        }
    }

    #[test]
    fn full_line() {
        test_read(vec![
            (vec![b"hello there\r\n"], Ok(Some("hello there")))
        ]);
    }

    #[test]
    fn multiple_full_lines_all() {
        test_read(vec![
            (vec![b"hello there\r\n"], Ok(Some("hello there"))),
            (vec![b"hello there 2\r\n"], Ok(Some("hello there 2"))),
            (vec![b"hello there 3\r\n"], Ok(Some("hello there 3"))),
            (vec![], Ok(None))
        ]);
    }

    #[test]
    fn multiple_full_lines_all_at_once() {
        test_read(vec![
            (vec![b"hello there\r\n", b"hello there 2\r\n", b"hello there 3\r\n"], Ok(Some("hello there"))),
            (vec![], Ok(Some("hello there 2"))),
            (vec![], Ok(Some("hello there 3"))),
            (vec![], Ok(None))
        ]);
    }

    #[test]
    fn multiple_full_lines_fragmented_all_at_once() {
        test_read(vec![
            (vec![b"hello ", b"there\r", b"\n", b"hell", b"o the", b"re 2\r", b"\n", b"he", b"ll", b"o the", b"re 3", b"\r", b"\n"], Ok(Some("hello there"))),
            (vec![], Ok(Some("hello there 2"))),
            (vec![], Ok(Some("hello there 3"))),
            (vec![], Ok(None)),
        ]);
    }

    #[test]
    fn full_line_in_fragments() {
        test_read(vec![
            (vec![b"he", b"llo", b" there", b"\r", b"\n"], Ok(Some("hello there")))
        ]);
    }

    #[test]
    fn partial_line() {
        test_read(vec![
            (vec![b"hello"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b" there"], Ok(None)),
            (vec![b"\r"], Ok(None)),
            (vec![b"\n"], Ok(Some("hello  there"))),
        ]);
    }

    #[test]
    fn partial_line_multiple_fragments() {
        test_read(vec![
            (vec![b"hel", b"lo"], Ok(None)),
            (vec![b" ", b"t"], Ok(None)),
            (vec![b"he", b"r", b"e"], Ok(None)),
            (vec![b"\r", b"\n"], Ok(Some("hello there")))
        ]);
    }

    #[test]
    fn no_new_data_for_a_while() {
        test_read(vec![
            (vec![b"hel", b"lo"], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![b"\r", b"\n"], Ok(Some("hello")))
        ]);
    }

    #[test]
    fn missing_cr() {
        test_read(vec![
            (vec![b"hello"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b" there"], Ok(None)),
            (vec![b"\n"], Err(BadSyntax)),
        ]);
    }

    #[test]
    fn missing_lf() {
        test_read(vec![
            (vec![b"hello"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b" there"], Ok(None)),
            (vec![b"\r"], Ok(None)),
        ]);
    }

    #[test]
    fn missing_crlf_before_eof() {
        test_read(vec![
            (vec![b"hello"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b" there"], Ok(None)),
            (vec![b""], Err(BadSyntax))
        ]);
    }

    #[test]
    fn no_data_eof() {
        test_read(vec![
            (vec![b""], Err(Error::from(ErrorKind::UnexpectedEof).into()))
        ]);
    }

    #[test]
    fn no_data() {
        test_read(vec![
            (vec![], Ok(None))
        ]);
    }

    #[test]
    fn invalid_utf8() {
        let data = vec![0, 255, 2, 127, 4, 5, 3, 8];
        test_read(vec![
            (vec![&data], Ok(None))
        ]);
    }

    #[test]
    fn invalid_utf8_with_crlf() {
        let data = vec![0, 255, 2, 127, 4, 5, 3, 8];
        test_read(vec![
            (vec![&data, b"\r\n"], Err(Error::new(ErrorKind::InvalidData, "stream did not contain valid UTF-8").into()))
        ]);
    }

    #[test]
    fn weird_line() {
        let data = b"r3984ty 98q39p8fuq p    9^\t%$\r%$@#!#@!%\r$%^%&%&*()_+|:{}>][/[\\/]3-062--=-9`~";
        test_read(vec![
            (vec![data], Ok(None)),
            (vec![b"\r\n"], Ok(Some(String::from_utf8_lossy(data).to_string().as_str()))),
        ]);
    }

    #[test]
    fn too_long() {
        let data = b" wrgiu hweiguhwepuiorgh w;eouirgh w;eoirugh ;weoug weroigj o;weirjg ;q\
        weroig pweoirg ;ewoirjhg; weoi";
        test_read(vec![
            (vec![data], Ok(None)),
            (vec![data, data], Ok(None)),
            (vec![data], Ok(None)),
            (vec![data], Ok(None)),
            (vec![data], Err(Error::new(ErrorKind::Other, "read limit reached").into())),
        ]);
    }
}