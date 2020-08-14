use std::io::{BufRead, Error, ErrorKind, Read};

use crate::common::header::{CONTENT_LENGTH, HeaderMap, HeaderMapOps, TRANSFER_ENCODING};
use crate::read::crlf_line_reader::CrlfLineReader;
use crate::read::error::ParsingError;
use crate::read::error_take::ErrorTake;

const MAX_BODY_SIZE: usize = 3 * 1024 * 1024; // 3 megabytes

pub struct BodyReader {
    kind: BodyReaderKind,
    body: Option<Vec<u8>>,
}

enum BodyReaderKind {
    Sized(usize),
    UntilEof,
    Chunked(ChunkState),
    Empty,
}

impl BodyReader {
    pub fn new(read_if_no_content_length: bool, headers: &HeaderMap) -> Result<BodyReader, ParsingError> {
        if let Some(size) = get_content_length(headers) {
            let size = size?;
            if size > MAX_BODY_SIZE {
                return Err(ParsingError::ContentLengthTooLarge);
            }
            return Ok(BodyReader { kind: BodyReaderKind::Sized(0), body: Some(vec![0; size]) });
        }

        let kind = if is_chunked_transfer_encoding(headers) {
            BodyReaderKind::Chunked(ChunkState::new())
        } else if read_if_no_content_length {
            BodyReaderKind::UntilEof
        } else {
            BodyReaderKind::Empty
        };

        Ok(BodyReader { kind, body: Some(vec![]) })
    }

    pub fn read(&mut self, reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, ParsingError> {
        let body = self.body.as_mut().unwrap();
        let mut reader = reader.error_take((MAX_BODY_SIZE - body.len()) as u64);

        let done = match self.kind {
            BodyReaderKind::Sized(ref mut pos) => read_sized(&mut reader, body, pos),
            BodyReaderKind::Chunked(ref mut state) => read_chunked(&mut reader, body, state),
            BodyReaderKind::UntilEof => read_until_end(&mut reader, body),
            BodyReaderKind::Empty => Ok(true)
        }?;

        if done {
            Ok(self.body.replace(vec![]))
        } else {
            Ok(None)
        }
    }
}

enum ChunkState {
    Size(CrlfLineReader),
    Body(usize, Option<Vec<u8>>),
    TailingCrlf(CrlfLineReader, bool),
}

impl ChunkState {
    fn new() -> ChunkState {
        ChunkState::Size(CrlfLineReader::new())
    }
}

/// Gets the value of a content-length header from the given header map. May return None if there's
/// no content-length header, or an error if the content-length value can not be parsed.
fn get_content_length(headers: &HeaderMap) -> Option<Result<usize, ParsingError>> {
    headers.get_first_header_value(&CONTENT_LENGTH)
        .map(|value| value.parse().map_err(|_| ParsingError::InvalidHeaderValue))
}

/// Checks if the header map has chunked transfer encoding header value.
fn is_chunked_transfer_encoding(headers: &HeaderMap) -> bool {
    headers.get_first_header_value(&TRANSFER_ENCODING).map(|v| v.eq("chunked")).unwrap_or(false)
}

fn read_sized(reader: &mut impl Read, body: &mut Vec<u8>, pos: &mut usize) -> Result<bool, ParsingError> {
    loop {
        let size = body.len();
        let mut buf = &mut body[*pos..size];

        match reader.read(&mut buf) {
            Ok(0) if buf.len() != 0 => return Err(Error::from(ErrorKind::UnexpectedEof).into()),
            Ok(amt) => {
                *pos += amt;
                if *pos == size {
                    return Ok(true);
                }
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => return Ok(false),
            Err(err) => return Err(err.into())
        }
    }
}

fn read_until_end(reader: &mut impl Read, body: &mut Vec<u8>) -> Result<bool, ParsingError> {
    return match reader.read_to_end(body) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(false),
        Err(err) => Err(err.into())
    };
}

fn read_chunked(reader: &mut impl BufRead, body: &mut Vec<u8>, state: &mut ChunkState) -> Result<bool, ParsingError> {
    match state {
        ChunkState::Size(inner) => {
            if let Some(line) = inner.read(reader)? {
                let size = usize::from_str_radix(&line, 16).map_err(|_| ParsingError::InvalidChunkSize)?;
                *state = ChunkState::Body(0, Some(vec![0; size]));
                return read_chunked(reader, body, state);
            }
        }
        ChunkState::Body(pos, chunk) => {
            let chunk_mut = chunk.as_mut().unwrap();
            if read_sized(reader, chunk_mut, pos)? {
                let is_last = chunk_mut.is_empty();
                body.append(chunk_mut);
                *state = ChunkState::TailingCrlf(CrlfLineReader::new(), is_last);
                return read_chunked(reader, body, state);
            }
        }
        ChunkState::TailingCrlf(inner, is_last) => {
            if let Some(line) = inner.read(reader)? {
                if !line.is_empty() {
                    return Err(ParsingError::BadSyntax);
                } else if *is_last {
                    return Ok(true);
                } else {
                    *state = ChunkState::Size(CrlfLineReader::new());
                    return read_chunked(reader, body, state);
                }
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Error, ErrorKind};

    use crate::header_map;
    use crate::read::body_reader::BodyReader;
    use crate::read::error::ParsingError;
    use crate::util::mock::{EndlessMockReader, MockReader};

    fn test_sized(size: usize, tests: Vec<(Vec<&[u8]>, Result<Option<Vec<u8>>, ParsingError>)>) {
        let body_reader = BodyReader::new(false, &header_map![("content-length", size.to_string())]).unwrap();
        test(body_reader, tests);
    }

    fn test_until_eof(tests: Vec<(Vec<&[u8]>, Result<Option<Vec<u8>>, ParsingError>)>) {
        let body_reader = BodyReader::new(true, &header_map![]).unwrap();
        test(body_reader, tests);
    }

    fn test(mut body_reader: BodyReader, tests: Vec<(Vec<&[u8]>, Result<Option<Vec<u8>>, ParsingError>)>) {
        let mut reader = MockReader::from_bytes(vec![]);
        reader.return_would_block_when_empty = true;
        let mut reader = BufReader::new(reader);
        for (new_data, expected_result) in tests {
            reader.get_mut().data.extend(new_data.into_iter().map(|v| v.to_vec()));
            let actual_result = body_reader.read(&mut reader);
            assert_eq!(format!("{:?}", actual_result), format!("{:?}", expected_result));
        }
    }

    fn test_endless(mut body_reader: BodyReader, start: Vec<&[u8]>, sequence: &[u8], expected: Result<Option<Vec<u8>>, ParsingError>) {
        let reader = EndlessMockReader::from_bytes(start, sequence);
        let mut reader = BufReader::new(reader);
        let actual = body_reader.read(&mut reader);
        assert_eq!(format!("{:?}", actual), format!("{:?}", expected));
    }

    #[test]
    fn sized_body_all_at_once() {
        test_sized(11, vec![
            (vec![b"hello world"], Ok(Some(b"hello world".to_vec())))
        ])
    }

    #[test]
    fn sized_body_all_at_once_fragmented() {
        test_sized(11, vec![
            (vec![b"h", b"el", b"lo", b" w", b"or", b"ld"], Ok(Some(b"hello world".to_vec())))
        ])
    }

    #[test]
    fn sized_body_partial() {
        test_sized(11, vec![
            (vec![], Ok(None)),
            (vec![b"h", b"ell"], Ok(None)),
            (vec![b"o"], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![b" w", b"o", b"rl"], Ok(None)),
            (vec![], Ok(None)),
            (vec![b"d"], Ok(Some(b"hello world".to_vec()))),
        ])
    }

    #[test]
    fn sized_body_eof() {
        test_sized(11, vec![
            (vec![b"h", b"ell"], Ok(None)),
            (vec![b"o"], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![b" w", b"o", b"rl"], Ok(None)),
            (vec![b""], Err(Error::from(ErrorKind::UnexpectedEof).into())),
        ])
    }

    #[test]
    fn sized_body_more_data_than_size() {
        test_sized(11, vec![
            (vec![b"h", b"ell"], Ok(None)),
            (vec![b"o"], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![b" w", b"o", b"rl"], Ok(None)),
            (vec![b"dblahblahblah"], Ok(Some(b"hello world".to_vec()))),
        ])
    }

    #[test]
    fn sized_body_too_big() {
        let res = BodyReader::new(false, &header_map![("content-length", usize::max_value().to_string())]);
        assert_eq!(format!("{:?}", res.err().unwrap()), format!("{:?}", ParsingError::ContentLengthTooLarge))
    }

    #[test]
    fn until_eof_all_at_once_with_eof() {
        test_until_eof(vec![
            (vec![b"hello ", b"blah ", b"blah", b" blah", b""], Ok(Some(b"hello blah blah blah".to_vec())))
        ])
    }

    #[test]
    fn until_eof_partial() {
        test_until_eof(vec![
            (vec![b"hello "], Ok(None)),
            (vec![b"he", b"l", b"lo"], Ok(None)),
            (vec![], Ok(None)),
            (vec![b" "], Ok(None)),
            (vec![b"hello"], Ok(None)),
            (vec![], Ok(None)),
            (vec![], Ok(None)),
            (vec![b""], Ok(Some(b"hello hello hello".to_vec())))
        ])
    }

    #[test]
    fn until_eof_endless() {
        let body_reader = BodyReader::new(true, &header_map![]).unwrap();
        test_endless(body_reader, vec![], b"blah", Err(Error::new(ErrorKind::Other, "read limit reached").into()))
    }

    #[test]
    fn no_content_length_should_not_read_until_eof() {
        let body_reader = BodyReader::new(false, &header_map![]).unwrap();
        test_endless(body_reader, vec![], b"blah", Ok(Some(vec![])))
    }
}