use std::io::{BufRead, ErrorKind};

use crate::parse::deframe::deframe::Deframe;
use crate::parse::deframe::line::LineOrEofDeframer;
use crate::parse::error::ParsingError;
use crate::parse::error_take::ReadExt;
use crate::parse::parse::{Parse, ParseResult, ParseStatus};
use crate::parse::parse::ParseStatus::{Blocked, Done};

const MAX_LINE_SIZE: usize = 512;

/// Parses a CRLF terminated line.
pub struct CrlfLineParser(CrlfLineOrEofParser);

impl CrlfLineParser {
    /// Creates a new CRLF line parser.
    pub fn new() -> CrlfLineParser {
        CrlfLineParser(CrlfLineOrEofParser::new())
    }
}

impl Parse<String> for CrlfLineParser {
    fn parse(self, reader: &mut impl BufRead) -> Result<ParseStatus<String, Self>, ParsingError> {
        match self.0.parse(reader)? {
            Done(None) => Err(ErrorKind::UnexpectedEof.into()),
            Done(Some(line)) => Ok(Done(line)),
            Blocked(parser) => Ok(Blocked(Self(parser)))
        }
    }
}

/// Parses either a CRLF terminated line or an EOF with no prior data.
/// If an EOF is found before any data is read, then None is returned.
pub struct CrlfLineOrEofParser(LineOrEofDeframer);

impl CrlfLineOrEofParser {
    /// Creates a new CRLF line or EOF parser.
    pub fn new() -> CrlfLineOrEofParser {
        CrlfLineOrEofParser(LineOrEofDeframer::new())
    }
}

impl Parse<Option<String>> for CrlfLineOrEofParser {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<Option<String>, Self> {
        let mut reader = reader.error_take((MAX_LINE_SIZE - self.0.read_so_far()) as u64);

        Ok(match self.0.parse(&mut reader)? {
            Done(Some(line)) => Done(Some(parse_crlf_line(line)?)),
            Done(None) => Done(None),
            Blocked(inner) => Blocked(Self(inner))
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
    use crate::parse::error::ParsingError;
    use crate::parse::error::ParsingError::BadSyntax;
    use crate::parse::test_util;

    fn test(tests: Vec<(Vec<&[u8]>, Result<Option<&str>, ParsingError>)>) {
        let tests = tests.into_iter()
            .map(|(data, exp)| (data, exp.map(|s| s.map(|s| s.to_string()))))
            .collect();
        test_util::test_blocking(CrlfLineParser::new(), tests);
    }

    #[test]
    fn full_line() {
        test(vec![
            (vec![b"hello there\r\n"], Ok(Some("hello there")))
        ]);
    }

    #[test]
    fn multiple_full_lines_all_at_once() {
        test(vec![
            (vec![b"hello there\r\n", b"hello there 2\r\n", b"hello there 3\r\n"], Ok(Some("hello there")))
        ]);
    }

    #[test]
    fn multiple_full_lines_fragmented_all_at_once() {
        test(vec![
            (vec![b"hello ", b"there\r", b"\n", b"hell", b"o the", b"re 2\r", b"\n", b"he", b"ll", b"o the", b"re 3", b"\r", b"\n"], Ok(Some("hello there"))),
        ]);
    }

    #[test]
    fn full_line_in_fragments() {
        test(vec![
            (vec![b"he", b"llo", b" there", b"\r", b"\n"], Ok(Some("hello there")))
        ]);
    }

    #[test]
    fn partial_line() {
        test(vec![
            (vec![b"hello"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b" there"], Ok(None)),
            (vec![b"\r"], Ok(None)),
            (vec![b"\n"], Ok(Some("hello  there"))),
        ]);
    }

    #[test]
    fn partial_line_multiple_fragments() {
        test(vec![
            (vec![b"hel", b"lo"], Ok(None)),
            (vec![b" ", b"t"], Ok(None)),
            (vec![b"he", b"r", b"e"], Ok(None)),
            (vec![b"\r", b"\n"], Ok(Some("hello there")))
        ]);
    }

    #[test]
    fn no_new_data_for_a_while() {
        test(vec![
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
        test(vec![
            (vec![b"hello"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b" there"], Ok(None)),
            (vec![b"\n"], Err(BadSyntax)),
        ]);
    }

    #[test]
    fn missing_lf() {
        test(vec![
            (vec![b"hello"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b" there"], Ok(None)),
            (vec![b"\r"], Ok(None)),
        ]);
    }

    #[test]
    fn missing_crlf_before_eof() {
        test(vec![
            (vec![b"hello"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b" there"], Ok(None)),
            (vec![b""], Err(Error::from(ErrorKind::UnexpectedEof).into()))
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
            (vec![], Ok(None))
        ]);
    }

    #[test]
    fn invalid_utf8() {
        let data = vec![0, 255, 2, 127, 4, 5, 3, 8];
        test(vec![
            (vec![&data], Ok(None))
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
            (vec![data], Ok(None)),
            (vec![b"\r\n"], Ok(Some(String::from_utf8_lossy(data).to_string().as_str()))),
        ]);
    }

    #[test]
    fn too_long() {
        let data = b" wrgiu hweiguhwepuiorgh w;eouirgh w;eoirugh ;weoug weroigj o;weirjg ;q\
        weroig pweoirg ;ewoirjhg; weoi";
        test(vec![
            (vec![data], Ok(None)),
            (vec![data, data], Ok(None)),
            (vec![data], Ok(None)),
            (vec![data], Ok(None)),
            (vec![data], Err(Error::new(ErrorKind::Other, "read limit reached").into())),
        ]);
    }
}