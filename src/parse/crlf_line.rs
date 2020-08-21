use std::io::{BufRead, ErrorKind};

use crate::parse::deframe::deframe::Deframe;
use crate::parse::deframe::line::LineDeframer;
use crate::parse::error::ParsingError;
use crate::parse::error_take::ReadExt;
use crate::parse::parse::{Parse, ParseStatus};
use crate::parse::parse::ParseStatus::{Done, IoErr};

const MAX_LINE_SIZE: usize = 512;

/// Parses a CRLF terminated line.
pub struct CrlfLineParser(LineDeframer);

impl CrlfLineParser {
    /// Creates a new CRLF line parser.
    pub fn new() -> CrlfLineParser {
        CrlfLineParser(LineDeframer::new())
    }

    /// Returns how many bytes this parser has read so far.
    pub fn read_so_far(&self) -> usize {
        self.0.read_so_far()
    }
}

impl Parse<String> for CrlfLineParser {
    fn parse(self, reader: &mut impl BufRead) -> Result<ParseStatus<String, Self>, ParsingError> {
        let mut reader = reader.error_take((MAX_LINE_SIZE - self.0.read_so_far()) as u64);
        Ok(match self.0.parse(&mut reader)? {
            Done(line) => Done(parse_crlf_line(line)?),
            IoErr(_, err) if err.kind() == ErrorKind::InvalidData => Err(ParsingError::InvalidUtf8)?,
            IoErr(parser, err) => IoErr(Self(parser), err)
        })
    }
}

/// Parses the given line as a CRLF terminated line. Assumes the given string already ends with \n.
fn parse_crlf_line(mut line: String) -> Result<String, ParsingError> {
    if let Some('\r') = line.pop() {
        Ok(line)
    } else {
        Err(ParsingError::BadSyntax)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use crate::parse::crlf_line::CrlfLineParser;
    use crate::parse::error::ParsingError::{BadSyntax, InvalidUtf8};
    use crate::parse::test_util;
    use crate::parse::test_util::TestParseResult::{IoErr, ParseErr, Value};
    use crate::parse::test_util::TestParseResult;

    fn test(tests: Vec<(Vec<&[u8]>, TestParseResult<&str>)>) {
        let tests = tests.into_iter()
            .map(|(data, exp)| {
                let exp = match exp {
                    Value(v) => Value(v.to_string()),
                    ParseErr(err) => ParseErr(err),
                    IoErr(err) => IoErr(err)
                };
                (data, exp)
            })
            .collect();
        test_util::test_blocking(CrlfLineParser::new(), tests);
    }

    #[test]
    fn full_line() {
        test(vec![
            (vec![b"hello there\r\n"], Value("hello there"))
        ]);
    }

    #[test]
    fn multiple_full_lines_all_at_once() {
        test(vec![
            (vec![b"hello there\r\n", b"hello there 2\r\n", b"hello there 3\r\n"], Value("hello there"))
        ]);
    }

    #[test]
    fn multiple_full_lines_fragmented_all_at_once() {
        test(vec![
            (vec![b"hello ", b"there\r", b"\n", b"hell", b"o the", b"re 2\r", b"\n", b"he", b"ll", b"o the", b"re 3", b"\r", b"\n"], Value("hello there")),
        ]);
    }

    #[test]
    fn full_line_in_fragments() {
        test(vec![
            (vec![b"he", b"llo", b" there", b"\r", b"\n"], Value("hello there"))
        ]);
    }

    #[test]
    fn partial_line() {
        test(vec![
            (vec![b"hello"], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![b" there"], ErrorKind::WouldBlock.into()),
            (vec![b"\r"], ErrorKind::WouldBlock.into()),
            (vec![b"\n"], Value("hello  there")),
        ]);
    }

    #[test]
    fn partial_line_multiple_fragments() {
        test(vec![
            (vec![b"hel", b"lo"], ErrorKind::WouldBlock.into()),
            (vec![b" ", b"t"], ErrorKind::WouldBlock.into()),
            (vec![b"he", b"r", b"e"], ErrorKind::WouldBlock.into()),
            (vec![b"\r", b"\n"], Value("hello there"))
        ]);
    }

    #[test]
    fn no_new_data_for_a_while() {
        test(vec![
            (vec![b"hel", b"lo"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"\r", b"\n"], Value("hello"))
        ]);
    }

    #[test]
    fn missing_cr() {
        test(vec![
            (vec![b"hello"], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![b" there"], ErrorKind::WouldBlock.into()),
            (vec![b"\n"], ParseErr(BadSyntax)),
        ]);
    }

    #[test]
    fn missing_lf() {
        test(vec![
            (vec![b"hello"], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![b" there"], ErrorKind::WouldBlock.into()),
            (vec![b"\r"], ErrorKind::WouldBlock.into()),
        ]);
    }

    #[test]
    fn missing_crlf_before_eof() {
        test(vec![
            (vec![b"hello"], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![b" there"], ErrorKind::WouldBlock.into()),
            (vec![b""], IoErr(Error::from(ErrorKind::UnexpectedEof)))
        ]);
    }

    #[test]
    fn no_data_eof() {
        test(vec![
            (vec![b""], IoErr(Error::from(ErrorKind::UnexpectedEof)))
        ]);
    }

    #[test]
    fn no_data() {
        test(vec![
            (vec![], ErrorKind::WouldBlock.into())
        ]);
    }

    #[test]
    fn invalid_utf8() {
        let data = vec![0, 255, 2, 127, 4, 5, 3, 8];
        test(vec![
            (vec![&data], ErrorKind::WouldBlock.into())
        ]);
    }

    #[test]
    fn invalid_utf8_with_crlf() {
        let data = vec![0, 255, 2, 127, 4, 5, 3, 8];
        test(vec![
            (vec![&data, b"\r\n"], ParseErr(InvalidUtf8))
        ]);
    }

    #[test]
    fn weird_line() {
        let data = b"r3984ty 98q39p8fuq p    9^\t%$\r%$@#!#@!%\r$%^%&%&*()_+|:{}>][/[\\/]3-062--=-9`~";
        test(vec![
            (vec![data], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], Value(String::from_utf8_lossy(data).to_string().as_str())),
        ]);
    }

    #[test]
    fn too_long() {
        let data = b" wrgiu hweiguhwepuiorgh w;eouirgh w;eoirugh ;weoug weroigj o;weirjg ;q\
        weroig pweoirg ;ewoirjhg; weoi";
        test(vec![
            (vec![data], ErrorKind::WouldBlock.into()),
            (vec![data, data], ErrorKind::WouldBlock.into()),
            (vec![data], ErrorKind::WouldBlock.into()),
            (vec![data], ErrorKind::WouldBlock.into()),
            (vec![data], IoErr(Error::new(ErrorKind::Other, "read limit reached"))),
        ]);
    }
}