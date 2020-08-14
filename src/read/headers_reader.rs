use std::io::BufRead;

use crate::common::header::{Header, HeaderMap, HeaderMapOps};
use crate::header_map;
use crate::read::crlf_line_reader::CrlfLineReader;
use crate::read::error::ParsingError;
use crate::read::error_take::ErrorTake;

const MAX_HEADERS_SIZE: usize = 4096;

pub struct HeadersReader {
    line_reader: CrlfLineReader,
    headers: Option<HeaderMap>,
    read: usize,
}

impl HeadersReader {
    pub fn new() -> HeadersReader {
        HeadersReader {
            line_reader: CrlfLineReader::new(),
            headers: Some(header_map![]),
            read: 0,
        }
    }

    pub fn read(&mut self, reader: &mut impl BufRead) -> Result<Option<HeaderMap>, ParsingError> {
        let mut reader = reader.error_take((MAX_HEADERS_SIZE - self.read) as u64);

        while let Some(line) = self.line_reader.read(&mut reader)? {
            if line.is_empty() {
                return Ok(self.headers.replace(header_map![]));
            }

            self.read += line.len();

            let (header, value) = parse_header(line)?;
            self.headers.as_mut().unwrap().add_header(header, value);
        }

        Ok(None)
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
    use std::io::{BufReader, Error, ErrorKind};

    use crate::common::header;
    use crate::common::header::HeaderMap;
    use crate::header_map;
    use crate::read::error::ParsingError;
    use crate::read::error::ParsingError::BadSyntax;
    use crate::read::headers_reader::HeadersReader;
    use crate::util::mock::MockReader;

    fn test_read(tests: Vec<(Vec<&[u8]>, Result<Option<HeaderMap>, ParsingError>)>) {
        let mut reader = MockReader::from_bytes(vec![]);
        reader.return_would_block_when_empty = true;
        let mut reader = BufReader::new(reader);
        let mut headers_reader = HeadersReader::new();
        for (new_data, expected_result) in tests {
            reader.get_mut().data.extend(new_data.into_iter().map(|v| v.to_vec()));
            let actual_result = headers_reader.read(&mut reader);
            match (actual_result, expected_result) {
                (Ok(Some(act)), Ok(Some(exp))) => assert_eq!(act, exp),
                (act, exp) => assert_eq!(format!("{:?}", act), format!("{:?}", exp))
            }
        }
    }

    #[test]
    fn one_full_header() {
        test_read(vec![
            (vec![b"header: value\r\n\r\n"], Ok(Some(header_map![("header", "value")])))
        ])
    }

    #[test]
    fn multiple_full_headers_all_at_once() {
        test_read(vec![
            (vec![b"header: value\r\nheader2: value2\r\ncontent-length: 5\r\n\r\n"],
             Ok(Some(header_map![("header", "value"), ("header2", "value2"), (header::CONTENT_LENGTH, "5")])))
        ])
    }

    #[test]
    fn multiple_full_headers_all_at_once_fragmented() {
        test_read(vec![
            (vec![b"head", b"er: va", b"l", b"ue\r", b"\nhead", b"e", b"r2: val", b"ue2", b"\r", b"\ncon", b"ten", b"t-le", b"ngth: 5\r", b"\n", b"\r", b"\n"],
             Ok(Some(header_map![("header", "value"), ("header2", "value2"), (header::CONTENT_LENGTH, "5")])))
        ])
    }

    #[test]
    fn partial_header() {
        test_read(vec![
            (vec![b"head"], Ok(None)),
            (vec![b"er"], Ok(None)),
            (vec![b":"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b"val"], Ok(None)),
            (vec![b"ue\r"], Ok(None)),
            (vec![b"\n\r"], Ok(None)),
            (vec![b"\n"], Ok(Some(header_map![("header", "value")])))
        ])
    }

    #[test]
    fn partial_headers() {
        test_read(vec![
            (vec![b"head"], Ok(None)),
            (vec![b"er"], Ok(None)),
            (vec![b":"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b"val"], Ok(None)),
            (vec![b"ue\r"], Ok(None)),
            (vec![b"\n"], Ok(None)),
            (vec![b"head"], Ok(None)),
            (vec![b"er2"], Ok(None)),
            (vec![b":"], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b"val"], Ok(None)),
            (vec![b"ue2\r"], Ok(None)),
            (vec![b"\n"], Ok(None)),
            (vec![b"header3:", b" value3"], Ok(None)),
            (vec![b"\r\n"], Ok(None)),
            (vec![b"\r\n"], Ok(Some(header_map![
                ("header", "value"),
                ("header2", "value2"),
                ("header3", "value3"),
            ]))),
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
            (vec![], Ok(None)),
            (vec![b"header: "], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![b"value"], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![b"\r\n"], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![b"\r\n"], Ok(Some(header_map![("header", "value")]))),
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
            (vec![data, b":", data], Ok(None)),
            (vec![data, data], Ok(None)),
            (vec![data], Ok(None)),
            (vec![data], Err(Error::new(ErrorKind::Other, "read limit reached").into())),
        ])
    }

    #[test]
    fn too_many_headers() {
        let header = b"oergoeiwglieuhrglieuwhrg: ebuhrgoibeusrghobsie\
        urghobsiuerghosejtgihleiurthglertiughlreitugherthrhtrt\r\n";
        test_read(vec![
            (vec![header, header, header, header, header, header], Ok(None)),
            (vec![header, header, header, header, header, header], Ok(None)),
            (vec![header, header, header, header, header, header], Ok(None)),
            (vec![header, header, header, header, header, header], Ok(None)),
            (vec![header, header, header, header, header, header], Ok(None)),
            (vec![header, header, header, header, header, header], Ok(None)),
            (vec![header, header, header, header, header, header], Err(Error::new(ErrorKind::Other, "read limit reached").into())),
        ])
    }
}