use std::io::BufRead;

use crate::common::header::{Header, HeaderMap, HeaderMapOps};
use crate::header_map;
use crate::deframe::crlf_line_deframer::CrlfLineDeframer;
use crate::deframe::error::DeframingError;
use crate::deframe::error_take::ErrorTake;

/// The max number of bytes read by a headers deframer.
const MAX_HEADERS_SIZE: usize = 4096;

/// Deframer for headers.
pub struct HeadersDeframer {
    line_deframer: CrlfLineDeframer,
    headers: Option<HeaderMap>,
    read: usize,
}

impl HeadersDeframer {
    /// Creates a new headers deframer.
    pub fn new() -> HeadersDeframer {
        HeadersDeframer {
            line_deframer: CrlfLineDeframer::new(),
            headers: Some(header_map![]),
            read: 0,
        }
    }

    /// Reads from the given reader and tries to deframe headers.
    pub fn read(&mut self, reader: &mut impl BufRead) -> Result<HeaderMap, DeframingError> {
        let mut reader = reader.error_take((MAX_HEADERS_SIZE - self.read) as u64);

        loop {
            let line = self.line_deframer.read(&mut reader)?;

            if line.is_empty() {
                return Ok(self.headers.replace(header_map![]).unwrap());
            }

            self.read += line.len();

            let (header, value) = parse_header(line)?;
            self.headers.as_mut().unwrap().add_header(header, value);
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
    use std::io::{BufReader, Error, ErrorKind};

    use crate::common::header;
    use crate::common::header::HeaderMap;
    use crate::header_map;
    use crate::deframe::error::DeframingError;
    use crate::deframe::error::DeframingError::BadSyntax;
    use crate::deframe::headers_deframer::HeadersDeframer;
    use crate::util::mock::MockReader;

    fn test_read(tests: Vec<(Vec<&[u8]>, Result<HeaderMap, DeframingError>)>) {
        let mut reader = MockReader::from_bytes(vec![]);
        reader.return_would_block_when_empty = true;
        let mut reader = BufReader::new(reader);
        let mut headers_reader = HeadersDeframer::new();
        for (new_data, expected_result) in tests {
            reader.get_mut().data.extend(new_data.into_iter().map(|v| v.to_vec()));
            let actual_result = headers_reader.read(&mut reader);
            match (actual_result, expected_result) {
                (Ok(act), Ok(exp)) => assert_eq!(act, exp),
                (act, exp) => assert_eq!(format!("{:?}", act), format!("{:?}", exp))
            }
        }
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