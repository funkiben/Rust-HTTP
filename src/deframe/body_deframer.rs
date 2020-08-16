use std::io::{BufRead, Error, ErrorKind, Read};

use crate::common::header::{CONTENT_LENGTH, HeaderMap, HeaderMapOps, TRANSFER_ENCODING};
use crate::deframe::crlf_line_deframer::CrlfLineDeframer;
use crate::deframe::deframe::Deframe;
use crate::deframe::error::DeframingError;
use crate::deframe::error_take::ReadExt;

const MAX_BODY_SIZE: usize = 3 * 1024 * 1024; // 3 megabytes

pub enum BodyDeframer {
    Sized(SizedDeframer),
    UntilEOF(UntilEOFDeframer),
    Chunked(ChunkDeframer),
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
            Ok(BodyDeframer::Sized(SizedDeframer::new(size)))
        } else if is_chunked_transfer_encoding(headers) {
            Ok(BodyDeframer::Chunked(ChunkDeframer::new()))
        } else if read_if_no_content_length {
            Ok(BodyDeframer::UntilEOF(UntilEOFDeframer::new()))
        } else {
            Ok(BodyDeframer::Empty)
        }
    }
}

impl Deframe<Vec<u8>> for BodyDeframer {
    fn read(self, reader: &mut impl BufRead) -> Result<Vec<u8>, (Self, DeframingError)> {
        match self {
            BodyDeframer::Sized(deframer) => deframer.read(reader).map_err(|(deframer, err)| (BodyDeframer::Sized(deframer), err)),
            BodyDeframer::UntilEOF(deframer) => deframer.read(reader).map_err(|(deframer, err)| (BodyDeframer::UntilEOF(deframer), err)),
            BodyDeframer::Chunked(deframer) => deframer.read(reader).map_err(|(deframer, err)| (BodyDeframer::Chunked(deframer), err)),
            BodyDeframer::Empty => Ok(vec![])
        }
    }
}

pub struct SizedDeframer {
    body: Vec<u8>,
    pos: usize,
}

impl SizedDeframer {
    fn new(size: usize) -> SizedDeframer {
        SizedDeframer { body: vec![0; size], pos: 0 }
    }
}

impl Deframe<Vec<u8>> for SizedDeframer {
    fn read(mut self, reader: &mut impl BufRead) -> Result<Vec<u8>, (Self, DeframingError)> {
        let mut reader = reader.error_take((MAX_BODY_SIZE - self.body.len()) as u64);

        loop {
            let len = self.body.len();
            let mut buf = &mut self.body[self.pos..len];

            match reader.read(&mut buf) {
                Ok(0) if buf.len() != 0 => return Err((self, Error::from(ErrorKind::UnexpectedEof).into())),
                Ok(amt) => {
                    self.pos += amt;
                    if self.pos == len {
                        return Ok(self.body);
                    }
                }
                Err(err) => return Err((self, err.into()))
            }
        }
    }
}

pub struct UntilEOFDeframer {
    body: Vec<u8>
}

impl UntilEOFDeframer {
    fn new() -> UntilEOFDeframer {
        UntilEOFDeframer { body: vec![] }
    }
}

impl Deframe<Vec<u8>> for UntilEOFDeframer {
    fn read(mut self, reader: &mut impl BufRead) -> Result<Vec<u8>, (Self, DeframingError)> {
        let mut reader = reader.error_take((MAX_BODY_SIZE - self.body.len()) as u64);

        match reader.read_to_end(&mut self.body) {
            Ok(_) => Ok(self.body),
            Err(err) => Err((self, err.into()))
        }
    }
}

pub struct ChunkDeframer {
    body: Vec<u8>,
    state: ChunkedBodyState,
}

enum ChunkedBodyState {
    Size(ChunkSizeDeframer),
    Data(SizedDeframer),
    TailingCrlf(TailingCrlfDeframer, bool),
}

impl ChunkDeframer {
    fn new() -> ChunkDeframer {
        ChunkDeframer { body: vec![], state: ChunkedBodyState::Size(ChunkSizeDeframer::new()) }
    }

    fn size_state(deframer: ChunkSizeDeframer, body: Vec<u8>) -> ChunkDeframer {
        ChunkDeframer { state: ChunkedBodyState::Size(deframer), body }
    }

    fn data_state(deframer: SizedDeframer, body: Vec<u8>) -> ChunkDeframer {
        ChunkDeframer { state: ChunkedBodyState::Data(deframer), body }
    }

    fn tailing_crlf_state(deframer: TailingCrlfDeframer, is_last: bool, body: Vec<u8>) -> ChunkDeframer {
        ChunkDeframer { state: ChunkedBodyState::TailingCrlf(deframer, is_last), body }
    }
}

struct ChunkSizeDeframer {
    inner: CrlfLineDeframer
}

impl ChunkSizeDeframer {
    fn new() -> ChunkSizeDeframer {
        ChunkSizeDeframer { inner: CrlfLineDeframer::new() }
    }
}

impl Deframe<usize> for ChunkSizeDeframer {
    fn read(self, reader: &mut impl BufRead) -> Result<usize, (Self, DeframingError)> {
        let line = self.inner.read(reader)
            .map_err(|(deframer, err)| (ChunkSizeDeframer { inner: deframer }, err))?;

        usize::from_str_radix(&line, 16)
            .map_err(|_| DeframingError::InvalidChunkSize)
            .and_then(|size|
                if size > MAX_BODY_SIZE {
                    Err(DeframingError::ContentLengthTooLarge)
                } else {
                    Ok(size)
                })
            .map_err(|err| (ChunkSizeDeframer::new(), err))
    }
}

struct TailingCrlfDeframer {
    inner: CrlfLineDeframer
}

impl TailingCrlfDeframer {
    fn new() -> TailingCrlfDeframer {
        TailingCrlfDeframer { inner: CrlfLineDeframer::new() }
    }
}

impl Deframe<()> for TailingCrlfDeframer {
    fn read(self, reader: &mut impl BufRead) -> Result<(), (Self, DeframingError)> {
        let line = self.inner.read(reader)
            .map_err(|(deframer, err)| (TailingCrlfDeframer { inner: deframer }, err))?;

        if line.is_empty() {
            Ok(())
        } else {
            Err((TailingCrlfDeframer::new(), DeframingError::BadSyntax))
        }
    }
}


impl Deframe<Vec<u8>> for ChunkDeframer {
    fn read(self, reader: &mut impl BufRead) -> Result<Vec<u8>, (Self, DeframingError)> {
        let mut reader = reader.error_take((MAX_BODY_SIZE - self.body.len()) as u64);

        let ChunkDeframer { mut state, mut body } = self;
        loop {
            state = match state {
                ChunkedBodyState::Size(deframer) => {
                    match deframer.read(&mut reader) {
                        Ok(size) => ChunkedBodyState::Data(SizedDeframer::new(size)),
                        Err((deframer, err)) => return Err((ChunkDeframer::size_state(deframer, body), err))
                    }
                }
                ChunkedBodyState::Data(deframer) => {
                    match deframer.read(&mut reader) {
                        Ok(ref mut chunk) => {
                            let is_last = chunk.is_empty();
                            body.append(chunk);
                            ChunkedBodyState::TailingCrlf(TailingCrlfDeframer::new(), is_last)
                        }
                        Err((deframer, err)) => return Err((ChunkDeframer::data_state(deframer, body), err))
                    }
                }
                ChunkedBodyState::TailingCrlf(deframer, is_last) => {
                    match deframer.read(&mut reader) {
                        Ok(()) if is_last => return Ok(body),
                        Ok(()) => ChunkedBodyState::Size(ChunkSizeDeframer::new()),
                        Err((deframer, err)) => return Err((ChunkDeframer::tailing_crlf_state(deframer, is_last, body), err))
                    }
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
    use crate::deframe::deframe::Deframe;
    use crate::deframe::error::DeframingError;
    use crate::deframe::error::DeframingError::{BadSyntax, ContentLengthTooLarge};
    use crate::deframe::test_util;
    use crate::header_map;
    use crate::util::mock::EndlessMockReader;

    fn test_sized(size: usize, tests: Vec<(Vec<&[u8]>, Result<Vec<u8>, DeframingError>)>) {
        let deframer = BodyDeframer::new(false, &header_map![("content-length", size.to_string())]).unwrap();
        test_util::test_blocking(deframer, tests);
    }

    fn test_until_eof(tests: Vec<(Vec<&[u8]>, Result<Vec<u8>, DeframingError>)>) {
        let deframer = BodyDeframer::new(true, &header_map![]).unwrap();
        test_util::test_blocking(deframer, tests);
    }

    fn test_chunked(tests: Vec<(Vec<&[u8]>, Result<Vec<u8>, DeframingError>)>) {
        let deframer = BodyDeframer::new(false, &header_map![("transfer-encoding", "chunked")]).unwrap();
        test_util::test_blocking(deframer, tests);
    }

    fn test_endless(deframer: BodyDeframer, start: Vec<&[u8]>, sequence: &[u8], expected: Result<Vec<u8>, DeframingError>) {
        test_util::test_endless_bytes(deframer, start, sequence, expected);
    }

    #[test]
    fn sized_body_all_at_once() {
        test_sized(11, vec![
            (vec![b"hello world"], Ok(b"hello world".to_vec()))
        ])
    }

    #[test]
    fn stops_reading_once_size_is_reached() {
        test_sized(11, vec![
            (vec![b"hello worldhello world"], Ok(b"hello world".to_vec())),
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
    fn stops_reading_at_empty_chunk() {
        test_chunked(vec![
            (vec![b"5\r\n", b"hello\r\n", b"0\r\n\r\n", b"7\r\n", b"goodbye\r\n", b"0\r\n\r\n"], Ok(b"hello".to_vec())),
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