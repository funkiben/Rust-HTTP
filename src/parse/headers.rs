use std::io::BufRead;

use crate::common::header::{Header, HeaderMap, HeaderMapOps};
use crate::header_map;
use crate::parse::crlf_line::CrlfLineParser;
use crate::parse::error::ParsingError;
use crate::parse::error_take::ReadExt;
use crate::parse::parse::{Parse, ParseResult};
use crate::parse::parse::ParseStatus::{Done, IoErr};

const MAX_HEADERS_SIZE: usize = 4096;

/// Parser for headers.
pub struct HeadersParser {
    inner: CrlfLineParser,
    headers: HeaderMap,
    read: usize,
}

impl HeadersParser {
    /// Creates a new headers parser.
    pub fn new() -> HeadersParser {
        HeadersParser { inner: CrlfLineParser::new(), headers: header_map![], read: 0 }
    }
}

impl Parse<HeaderMap> for HeadersParser {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<HeaderMap, Self> {
        let Self { mut headers, mut inner, mut read } = self;

        let mut reader = reader.error_take((MAX_HEADERS_SIZE - read) as u64);

        loop {
            match inner.parse(&mut reader)? {
                Done(line) if line.is_empty() => return Ok(Done(headers)),
                Done(line) => {
                    read += line.len();
                    let (header, value) = parse_header(line)?;
                    headers.add_header(header, value);
                    inner = CrlfLineParser::new()
                }
                IoErr(inner, err) => return Ok(IoErr(HeadersParser { headers, inner, read }, err))
            }
        }
    }
}

/// Parses the given line as a header. Splits the line at the first ": " pattern.
fn parse_header(raw: String) -> Result<(Header, String), ParsingError> {
    let mut split = raw.splitn(2, ": ");

    let header_raw = split.next().ok_or(ParsingError::BadSyntax)?;
    let value = split.next().ok_or(ParsingError::BadSyntax)?;

    Ok((Header::from(header_raw), String::from(value)))
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use crate::common::header;
    use crate::common::header::HeaderMap;
    use crate::header_map;
    use crate::parse::headers::HeadersParser;
    use crate::parse::test_util::{test_blocking, TestParseResult};
    use crate::parse::test_util::TestParseResult::{IoErr, Value};

    fn test_read(tests: Vec<(Vec<&[u8]>, TestParseResult<HeaderMap>)>) {
        test_blocking(HeadersParser::new(), tests)
    }

    #[test]
    fn one_full_header() {
        test_read(vec![
            (vec![b"header: value\r\n\r\n"], Value(header_map![("header", "value")]))
        ])
    }

    #[test]
    fn multiple_full_headers_all_at_once() {
        test_read(vec![
            (vec![b"header: value\r\nheader2: value2\r\ncontent-length: 5\r\n\r\n"],
             Value(header_map![("header", "value"), ("header2", "value2"), (header::CONTENT_LENGTH, "5")]))
        ])
    }

    #[test]
    fn multiple_full_headers_all_at_once_fragmented() {
        test_read(vec![
            (vec![b"head", b"er: va", b"l", b"ue\r", b"\nhead", b"e", b"r2: val", b"ue2", b"\r", b"\ncon", b"ten", b"t-le", b"ngth: 5\r", b"\n", b"\r", b"\n"],
             Value(header_map![("header", "value"), ("header2", "value2"), (header::CONTENT_LENGTH, "5")]))
        ])
    }

    #[test]
    fn partial_header() {
        test_read(vec![
            (vec![b"head"], ErrorKind::WouldBlock.into()),
            (vec![b"er"], ErrorKind::WouldBlock.into()),
            (vec![b":"], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![b"val"], ErrorKind::WouldBlock.into()),
            (vec![b"ue\r"], ErrorKind::WouldBlock.into()),
            (vec![b"\n\r"], ErrorKind::WouldBlock.into()),
            (vec![b"\n"], Value(header_map![("header", "value")]))
        ])
    }

    #[test]
    fn partial_headers() {
        test_read(vec![
            (vec![b"head"], ErrorKind::WouldBlock.into()),
            (vec![b"er"], ErrorKind::WouldBlock.into()),
            (vec![b":"], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![b"val"], ErrorKind::WouldBlock.into()),
            (vec![b"ue\r"], ErrorKind::WouldBlock.into()),
            (vec![b"\n"], ErrorKind::WouldBlock.into()),
            (vec![b"head"], ErrorKind::WouldBlock.into()),
            (vec![b"er2"], ErrorKind::WouldBlock.into()),
            (vec![b":"], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![b"val"], ErrorKind::WouldBlock.into()),
            (vec![b"ue2\r"], ErrorKind::WouldBlock.into()),
            (vec![b"\n"], ErrorKind::WouldBlock.into()),
            (vec![b"header3:", b" value3"], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], Value(header_map![
                ("header", "value"),
                ("header2", "value2"),
                ("header3", "value3"),
            ])),
        ])
    }

    #[test]
    fn eof_in_middle_of_header() {
        test_read(vec![
            (vec![b"header: v", b""], IoErr(Error::from(ErrorKind::UnexpectedEof)))
        ])
    }

    #[test]
    fn eof_after_header() {
        test_read(vec![
            (vec![b"header: value\r\n", b""], IoErr(Error::from(ErrorKind::UnexpectedEof)))
        ])
    }

    #[test]
    fn no_data_for_a_while() {
        test_read(vec![
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"header: "], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"value"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], Value(header_map![("header", "value")])),
        ])
    }

    #[test]
    fn no_data_eof() {
        test_read(vec![
            (vec![b""], IoErr(Error::from(ErrorKind::UnexpectedEof)))
        ])
    }

    #[test]
    fn header_too_large() {
        let data = b"oergoeiwglieuhrglieuwhrgoiebuhrgoibeusrghobsie\
        urghobsiuerghosejtgihleiurthglertiughlreitugherthrhtrt";
        test_read(vec![
            (vec![data, b":", data], ErrorKind::WouldBlock.into()),
            (vec![data, data], ErrorKind::WouldBlock.into()),
            (vec![data], ErrorKind::WouldBlock.into()),
            (vec![data], IoErr(Error::new(ErrorKind::Other, "read limit reached"))),
        ])
    }

    #[test]
    fn too_many_headers() {
        let header = b"oergoeiwglieuhrglieuwhrg: ebuhrgoibeusrghobsie\
        urghobsiuerghosejtgihleiurthglertiughlreitugherthrhtrt\r\n";
        test_read(vec![
            (vec![header, header, header, header, header, header], ErrorKind::WouldBlock.into()),
            (vec![header, header, header, header, header, header], ErrorKind::WouldBlock.into()),
            (vec![header, header, header, header, header, header], ErrorKind::WouldBlock.into()),
            (vec![header, header, header, header, header, header], ErrorKind::WouldBlock.into()),
            (vec![header, header, header, header, header, header], ErrorKind::WouldBlock.into()),
            (vec![header, header, header, header, header, header], ErrorKind::WouldBlock.into()),
            (vec![header, header, header, header, header, header], IoErr(Error::new(ErrorKind::Other, "read limit reached"))),
        ])
    }
}
