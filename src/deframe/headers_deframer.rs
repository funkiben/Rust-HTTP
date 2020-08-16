use std::io::BufRead;

use crate::common::header::{Header, HeaderMap, HeaderMapOps};
use crate::deframe::crlf_line_deframer::CrlfLineDeframer;
use crate::deframe::deframe::Deframe;
use crate::deframe::error::DeframingError;
use crate::deframe::error_take::ReadExt;
use crate::header_map;

/// The max number of bytes read by a headers deframer.
const MAX_HEADERS_SIZE: usize = 4096;

/// Deframer for headers.
pub struct HeadersDeframer {
    header_deframer: HeaderDeframer,
    headers: HeaderMap,
    read: usize,
}

impl HeadersDeframer {
    /// Creates a new headers deframer.
    pub fn new() -> HeadersDeframer {
        HeadersDeframer {
            header_deframer: HeaderDeframer::new(),
            headers: header_map![],
            read: 0,
        }
    }
}

impl Deframe<HeaderMap> for HeadersDeframer {
    fn read(mut self, reader: &mut impl BufRead) -> Result<HeaderMap, (Self, DeframingError)> {
        let mut reader = reader.error_take((MAX_HEADERS_SIZE - self.read) as u64);

        loop {
            match self.header_deframer.read(&mut reader) {
                Ok(None) => return Ok(self.headers),
                Ok(Some((header, value))) => {
                    self.read += header.as_str().len() + value.len() + 2;
                    self.headers.add_header(header, value);
                    self.header_deframer = HeaderDeframer::new();
                }
                Err((header_deframer, err)) => {
                    self.header_deframer = header_deframer;
                    return Err((self, err));
                }
            }
        }
    }
}

struct HeaderDeframer {
    line_deframer: CrlfLineDeframer
}

impl HeaderDeframer {
    fn new() -> HeaderDeframer {
        HeaderDeframer { line_deframer: CrlfLineDeframer::new() }
    }
}

impl Deframe<Option<(Header, String)>> for HeaderDeframer {
    fn read(mut self, reader: &mut impl BufRead) -> Result<Option<(Header, String)>, (Self, DeframingError)> {
        match self.line_deframer.read(reader) {
            Ok(line) if line.is_empty() => return Ok(None),
            Ok(line) => parse_header(line).map(|val| Some(val)).map_err(|err| (HeaderDeframer::new(), err)),
            Err((line_deframer, err)) => {
                self.line_deframer = line_deframer;
                return Err((self, err));
            }
        }
    }
}

/// Parses the given line as a header. Splits the line at the first ": " pattern.
fn parse_header(raw: String) -> Result<(Header, String), DeframingError> {
    let mut split = raw.splitn(2, ": ");

    let header_raw = split.next().ok_or(DeframingError::BadSyntax)?;
    let value = split.next().ok_or(DeframingError::BadSyntax)?;

    Ok((Header::from(header_raw), String::from(value)))
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use crate::common::header;
    use crate::common::header::HeaderMap;
    use crate::deframe::error::DeframingError;
    use crate::deframe::error::DeframingError::BadSyntax;
    use crate::deframe::headers_deframer::HeadersDeframer;
    use crate::deframe::test_util::test_blocking;
    use crate::header_map;

    fn test_read(tests: Vec<(Vec<&[u8]>, Result<HeaderMap, DeframingError>)>) {
        test_blocking(HeadersDeframer::new(), tests)
    }

    #[test]
    fn one_full_header() {
        test_read(vec![
            (vec![b"header: value\r\n\r\n"], Ok(header_map![("header", "value")]))
        ])
    }

    #[test]
    fn multiple_full_headers_all_at_once() {
        test_read(vec![
            (vec![b"header: value\r\nheader2: value2\r\ncontent-length: 5\r\n\r\n"],
             Ok(header_map![("header", "value"), ("header2", "value2"), (header::CONTENT_LENGTH, "5")]))
        ])
    }

    #[test]
    fn multiple_full_headers_all_at_once_fragmented() {
        test_read(vec![
            (vec![b"head", b"er: va", b"l", b"ue\r", b"\nhead", b"e", b"r2: val", b"ue2", b"\r", b"\ncon", b"ten", b"t-le", b"ngth: 5\r", b"\n", b"\r", b"\n"],
             Ok(header_map![("header", "value"), ("header2", "value2"), (header::CONTENT_LENGTH, "5")]))
        ])
    }

    #[test]
    fn partial_header() {
        test_read(vec![
            (vec![b"head"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"er"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b":"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"val"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"ue\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n"], Ok(header_map![("header", "value")]))
        ])
    }

    #[test]
    fn partial_headers() {
        test_read(vec![
            (vec![b"head"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"er"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b":"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"val"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"ue\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"head"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"er2"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b":"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"val"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"ue2\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"header3:", b" value3"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Ok(header_map![
                ("header", "value"),
                ("header2", "value2"),
                ("header3", "value3"),
            ])),
        ])
    }

    #[test]
    fn eof_in_middle_of_header() {
        test_read(vec![
            (vec![b"header: v", b""], Err(BadSyntax))
        ])
    }

    #[test]
    fn eof_after_header() {
        test_read(vec![
            (vec![b"header: value\r\n", b""], Err(Error::from(ErrorKind::UnexpectedEof).into()))
        ])
    }

    #[test]
    fn no_data_for_a_while() {
        test_read(vec![
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"header: "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"value"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Ok(header_map![("header", "value")])),
        ])
    }

    #[test]
    fn no_data_eof() {
        test_read(vec![
            (vec![b""], Err(Error::from(ErrorKind::UnexpectedEof).into()))
        ])
    }

    #[test]
    fn header_too_large() {
        let data = b"oergoeiwglieuhrglieuwhrgoiebuhrgoibeusrghobsie\
        urghobsiuerghosejtgihleiurthglertiughlreitugherthrhtrt";
        test_read(vec![
            (vec![data, b":", data], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![data, data], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![data], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![data], Err(Error::new(ErrorKind::Other, "read limit reached").into())),
        ])
    }

    #[test]
    fn too_many_headers() {
        let header = b"oergoeiwglieuhrglieuwhrg: ebuhrgoibeusrghobsie\
        urghobsiuerghosejtgihleiurthglertiughlreitugherthrhtrt\r\n";
        test_read(vec![
            (vec![header, header, header, header, header, header], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![header, header, header, header, header, header], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![header, header, header, header, header, header], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![header, header, header, header, header, header], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![header, header, header, header, header, header], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![header, header, header, header, header, header], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![header, header, header, header, header, header], Err(Error::new(ErrorKind::Other, "read limit reached").into())),
        ])
    }
}