use std::io::{BufRead, Error, ErrorKind, Read};

use crate::common::header::{CONTENT_LENGTH, HeaderMap, HeaderMapOps, TRANSFER_ENCODING};
use crate::deframe::crlf_line_deframer::CrlfLineDeframer;
use crate::deframe::error::DeframingError;
use crate::deframe::error_take::ErrorTake;

const MAX_BODY_SIZE: usize = 3 * 1024 * 1024; // 3 megabytes

/// Deframer for the body of an HTTP request or responses.
pub struct BodyDeframer {
    kind: Kind,
    body: Option<Vec<u8>>,
}

/// Different kinds of bodies.
enum Kind {
    Sized(usize),
    UntilEof,
    Chunked(ChunkState),
    Empty,
}

impl BodyDeframer {
    /// Creates a new body deframer from the given headers.
    /// If "read_if_no_content_length" is true and there is no content length, then the body will be read until EOF is reached.
    pub fn new(read_if_no_content_length: bool, headers: &HeaderMap) -> Result<BodyDeframer, DeframingError> {
        if let Some(size) = get_content_length(headers) {
            let size = size?;
            if size > MAX_BODY_SIZE {
                return Err(DeframingError::ContentLengthTooLarge);
            }
            return Ok(BodyDeframer { kind: Kind::Sized(0), body: Some(vec![0; size]) });
        }

        let kind = if is_chunked_transfer_encoding(headers) {
            Kind::Chunked(ChunkState::Size(CrlfLineDeframer::new()))
        } else if read_if_no_content_length {
            Kind::UntilEof
        } else {
            Kind::Empty
        };

        Ok(BodyDeframer { kind, body: Some(vec![]) })
    }

    /// Reads data from the reader and tries to deframes a body.
    pub fn read(&mut self, reader: &mut impl BufRead) -> Result<Vec<u8>, DeframingError> {
        let body = self.body.as_mut().unwrap();
        let mut reader = reader.error_take((MAX_BODY_SIZE - body.len()) as u64);

        match &mut self.kind {
            Kind::Sized(pos) => read_sized(&mut reader, body, pos),
            Kind::Chunked(state) => read_chunked(&mut reader, body, state),
            Kind::UntilEof => read_until_end(&mut reader, body),
            Kind::Empty => Ok(())
        }?;

        self.kind = Kind::Empty;

        Ok(self.body.replace(vec![]).unwrap())
    }
}

/// The different states of deframing a chunked body.
enum ChunkState {
    Size(CrlfLineDeframer),
    Body(usize, Option<Vec<u8>>),
    TailingCrlf(CrlfLineDeframer, bool),
}

/// Reads a sized body from the reader.
/// Writes into the given body, and uses the given "pos" as the location in body for writing new data.
/// Data maybe be partially written into body even if an error is encountered.
fn read_sized(reader: &mut impl Read, body: &mut Vec<u8>, pos: &mut usize) -> Result<(), DeframingError> {
    loop {
        let size = body.len();
        let mut buf = &mut body[*pos..size];

        match reader.read(&mut buf) {
            Ok(0) if buf.len() != 0 => return Err(Error::from(ErrorKind::UnexpectedEof).into()),
            Ok(amt) => {
                *pos += amt;
                if *pos == size {
                    return Ok(());
                }
            }
            Err(err) => return Err(err.into())
        }
    }
}

/// Writes data from the given reader into body until EOF is reached.
/// Data maybe be partially written into body even if an error is encountered.
fn read_until_end(reader: &mut impl Read, body: &mut Vec<u8>) -> Result<(), DeframingError> {
    return match reader.read_to_end(body) {
        Ok(_) => Ok(()),
        Err(err) => Err(err.into())
    };
}

/// Reads chunks from the given reader. Appends the chunks to the given body.
/// Data maybe be partially written into body even if an error is encountered.
fn read_chunked(reader: &mut impl BufRead, body: &mut Vec<u8>, state: &mut ChunkState) -> Result<(), DeframingError> {
    loop {
        match state {
            ChunkState::Size(line_deframer) => {
                let line = line_deframer.read(reader)?;
                let size = usize::from_str_radix(&line, 16).map_err(|_| DeframingError::InvalidChunkSize)?;
                if size > MAX_BODY_SIZE {
                    return Err(DeframingError::ContentLengthTooLarge);
                }
                *state = ChunkState::Body(0, Some(vec![0; size]));
                continue;
            }
            ChunkState::Body(pos, chunk) => {
                let chunk_mut = chunk.as_mut().unwrap();
                read_sized(reader, chunk_mut, pos)?;
                let is_last = chunk_mut.is_empty();
                body.append(chunk_mut);
                *state = ChunkState::TailingCrlf(CrlfLineDeframer::new(), is_last);
                continue;
            }
            ChunkState::TailingCrlf(line_deframer, is_last) => {
                let line = line_deframer.read(reader)?;
                let is_last = *is_last;
                *state = ChunkState::Size(CrlfLineDeframer::new());
                if !line.is_empty() {
                    return Err(DeframingError::BadSyntax);
                } else if is_last {
                    return Ok(());
                } else {
                    continue;
                }
            }
        }
    }
}

/// Gets the value of a content-length header from the given header map. May return None if there's
/// no content-length header, or an error if the content-length value can not be parsed.
fn get_content_length(headers: &HeaderMap) -> Option<Result<usize, DeframingError>> {
    headers.get_first_header_value(&CONTENT_LENGTH)
        .map(|value| value.parse().map_err(|_| DeframingError::InvalidHeaderValue))
}

/// Checks if the header map has chunked transfer encoding header value.
fn is_chunked_transfer_encoding(headers: &HeaderMap) -> bool {
    headers.get_first_header_value(&TRANSFER_ENCODING).map(|v| v.eq("chunked")).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Error, ErrorKind};

    use crate::deframe::body_deframer::BodyDeframer;
    use crate::deframe::error::DeframingError;
    use crate::deframe::error::DeframingError::{BadSyntax, ContentLengthTooLarge};
    use crate::header_map;
    use crate::util::mock::{EndlessMockReader, MockReader};

    fn test_sized(size: usize, tests: Vec<(Vec<&[u8]>, Result<Vec<u8>, DeframingError>)>) {
        let body_reader = BodyDeframer::new(false, &header_map![("content-length", size.to_string())]).unwrap();
        test(body_reader, tests);
    }

    fn test_until_eof(tests: Vec<(Vec<&[u8]>, Result<Vec<u8>, DeframingError>)>) {
        let body_reader = BodyDeframer::new(true, &header_map![]).unwrap();
        test(body_reader, tests);
    }

    fn test_chunked(tests: Vec<(Vec<&[u8]>, Result<Vec<u8>, DeframingError>)>) {
        let body_reader = BodyDeframer::new(false, &header_map![("transfer-encoding", "chunked")]).unwrap();
        test(body_reader, tests);
    }

    fn test(mut body_reader: BodyDeframer, tests: Vec<(Vec<&[u8]>, Result<Vec<u8>, DeframingError>)>) {
        let mut reader = MockReader::from_bytes(vec![]);
        reader.return_would_block_when_empty = true;
        let mut reader = BufReader::new(reader);
        for (new_data, expected_result) in tests {
            reader.get_mut().data.extend(new_data.into_iter().map(|v| v.to_vec()));
            let actual_result = body_reader.read(&mut reader);
            assert_eq!(format!("{:?}", actual_result), format!("{:?}", expected_result));
        }
    }

    fn test_endless(mut body_reader: BodyDeframer, start: Vec<&[u8]>, sequence: &[u8], expected: Result<Vec<u8>, DeframingError>) {
        let reader = EndlessMockReader::from_bytes(start, sequence);
        let mut reader = BufReader::new(reader);
        let actual = body_reader.read(&mut reader);
        assert_eq!(format!("{:?}", actual), format!("{:?}", expected));
    }

    #[test]
    fn sized_body_all_at_once() {
        test_sized(11, vec![
            (vec![b"hello world"], Ok(b"hello world".to_vec()))
        ])
    }

    #[test]
    fn multiple_sized_bodies() {
        test_sized(11, vec![
            (vec![b"hello worldhello world"], Ok(b"hello world".to_vec())),
            (vec![], Ok(vec![]))
        ])
    }

    #[test]
    fn sized_body_all_at_once_fragmented() {
        test_sized(11, vec![
            (vec![b"h", b"el", b"lo", b" w", b"or", b"ld"], Ok(b"hello world".to_vec()))
        ])
    }

    #[test]
    fn sized_body_partial() {
        test_sized(11, vec![
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"h", b"ell"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"o"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" w", b"o", b"rl"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"d"], Ok(b"hello world".to_vec())),
        ])
    }

    #[test]
    fn sized_body_eof_before_size_reached() {
        test_sized(11, vec![
            (vec![b"h", b"ell"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"o"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" w", b"o", b"rl"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b""], Err(Error::from(ErrorKind::UnexpectedEof).into())),
        ])
    }

    #[test]
    fn sized_body_more_data_than_size() {
        test_sized(11, vec![
            (vec![b"h", b"ell"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"o"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" w", b"o", b"rl"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"dblahblahblah"], Ok(b"hello world".to_vec())),
        ])
    }

    #[test]
    fn sized_body_eof_before_any_data() {
        test_sized(11, vec![
            (vec![b""], Err(Error::from(ErrorKind::UnexpectedEof).into())),
        ])
    }

    #[test]
    fn sized_body_too_big() {
        let res = BodyDeframer::new(false, &header_map![("content-length", usize::max_value().to_string())]);
        assert_eq!(format!("{:?}", res.err().unwrap()), format!("{:?}", DeframingError::ContentLengthTooLarge))
    }

    #[test]
    fn until_eof_all_at_once_with_eof() {
        test_until_eof(vec![
            (vec![b"hello ", b"blah ", b"blah", b" blah", b""], Ok(b"hello blah blah blah".to_vec()))
        ])
    }

    #[test]
    fn until_eof_partial() {
        test_until_eof(vec![
            (vec![b"hello "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"he", b"l", b"lo"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"hello"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b""], Ok(b"hello hello hello".to_vec()))
        ])
    }

    #[test]
    fn until_eof_endless() {
        let body_reader = BodyDeframer::new(true, &header_map![]).unwrap();
        test_endless(body_reader, vec![], b"blah", Err(Error::new(ErrorKind::Other, "read limit reached").into()))
    }

    #[test]
    fn no_content_length_should_not_read_until_eof() {
        let body_reader = BodyDeframer::new(false, &header_map![]).unwrap();
        test_endless(body_reader, vec![], b"blah", Ok(vec![]))
    }

    #[test]
    fn chunks_partial() {
        test_chunked(vec![
            (vec![b"5\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"hello"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"1\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"5\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"world"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"0\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Ok(b"hello world".to_vec())),
        ]);
    }

    #[test]
    fn chunks_partial_no_data_sometimes() {
        test_chunked(vec![
            (vec![b"5\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"hello"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"1\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b" "], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"5\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"world"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"0\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r\n"], Ok(b"hello world".to_vec())),
        ]);
    }

    #[test]
    fn chunks_all_at_once() {
        test_chunked(vec![
            (vec![b"5\r\nhello\r\n1\r\n \r\n5\r\nworld\r\n0\r\n\r\n"], Ok(b"hello world".to_vec())),
        ]);
    }

    #[test]
    fn chunks_all_at_once_fragmented() {
        test_chunked(vec![
            (vec![b"5\r", b"\nhel", b"lo\r", b"\n1\r\n", b" \r\n5", b"\r\nwor", b"ld\r\n", b"0\r\n", b"\r", b"\n"], Ok(b"hello world".to_vec())),
        ]);
    }

    #[test]
    fn one_empty_chunk() {
        test_chunked(vec![
            (vec![b"0\r\n", b"\r\n"], Ok(vec![]))
        ]);
    }

    #[test]
    fn chunk_size_in_hex() {
        test_chunked(vec![
            (vec![b"f\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"fifteen letters\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"0\r\n\r\n"], Ok(b"fifteen letters".to_vec()))
        ]);
    }

    #[test]
    fn multiple_chunked_bodies() {
        test_chunked(vec![
            (vec![b"5\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"hello\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"0\r\n\r\n"], Ok(b"hello".to_vec())),
            (vec![b"7\r\n"], Ok(vec![])),
            (vec![b"goodbye\r\n"], Ok(vec![])),
            (vec![b"0\r\n\r\n"], Ok(vec![])),
        ]);
    }

    #[test]
    fn chunk_one_byte_at_a_time() {
        test_chunked(vec![
            (vec![b"a"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"0"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"1"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"2"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"3"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"4"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"5"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"6"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"7"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"8"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"9"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"0"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\r"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"\n"], Ok(b"0123456789".to_vec())),
        ]);
    }

    #[test]
    fn chunk_size_too_large() {
        test_chunked(vec![
            (vec![b"fffffff\r\n"], Err(ContentLengthTooLarge))
        ]);
    }

    #[test]
    fn endless_chunk_content() {
        let body_reader = BodyDeframer::new(false, &header_map![("transfer-encoding", "chunked")]).unwrap();
        test_endless(body_reader, vec![b"ff\r\n"], b"a", Err(Error::new(ErrorKind::Other, "read limit reached").into()));
    }

    #[test]
    fn endless_chunks() {
        let body_reader = BodyDeframer::new(false, &header_map![("transfer-encoding", "chunked")]).unwrap();
        test_endless(body_reader, vec![], b"1\r\na\r\n", Err(Error::new(ErrorKind::Other, "read limit reached").into()));
    }

    #[test]
    fn chunk_body_too_large() {
        test_chunked(vec![
            (vec![b"5\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"helloo\r\n"], Err(BadSyntax)),
        ]);
    }

    #[test]
    fn chunk_body_too_short() {
        test_chunked(vec![
            (vec![b"5\r\n"], Err(Error::from(ErrorKind::WouldBlock).into())),
            (vec![b"hell\r\n"], Err(BadSyntax)),
        ]);
    }
}