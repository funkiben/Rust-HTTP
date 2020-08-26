use std::io::BufRead;

use crate::common::header::{HeaderMap, HeaderMapOps};
use crate::common::header;
use crate::parse::body::BodyParser::{Chunked, Empty, UntilEof, WithSize};
use crate::parse::body::chunked::ChunksParser;
use crate::parse::deframe::bytes::{BytesDeframer, BytesUntilEofDeframer};
use crate::parse::deframe::deframe::Deframe;
use crate::parse::error::ParsingError;
use crate::parse::error_take::ReadExt;
use crate::parse::parse::{Parse, ParseResult};
use crate::parse::parse::ParseStatus::Done;

/// The maximum size of a body.
const MAX_BODY_SIZE: usize = 3 * 1024 * 1024; // 3 megabytes

/// Parser for a message body.
pub enum BodyParser {
    WithSize(BytesDeframer),
    UntilEof(BytesUntilEofDeframer),
    Chunked(ChunksParser),
    Empty,
}

impl BodyParser {
    /// Creates a new body parser.
    /// If read_if_no_content_length is true and no content length is present, then a "UntilEof" BodyParser is returned.
    pub fn new(headers: &HeaderMap, read_if_no_content_length: bool) -> Result<BodyParser, ParsingError> {
        if let Some(size) = get_content_length(headers) {
            let size = size?;
            if size > MAX_BODY_SIZE {
                return Err(ParsingError::ContentLengthTooLarge);
            }
            Ok(WithSize(BytesDeframer::new(size)))
        } else if is_chunked_transfer_encoding(headers) {
            Ok(Chunked(ChunksParser::new()))
        } else if read_if_no_content_length {
            Ok(UntilEof(BytesUntilEofDeframer::new()))
        } else {
            Ok(Empty)
        }
    }

    /// Gets the size of the body read so far.
    fn read_so_far(&self) -> usize {
        match self {
            WithSize(parser) => parser.read_so_far(),
            UntilEof(parser) => parser.read_so_far(),
            Chunked(parser) => parser.data_so_far(),
            Empty => 0
        }
    }
}

/// Gets the value of a content-length header from the given header map. May return None if there's
/// no content-length header, or an error if the content-length value can not be parsed.
fn get_content_length(headers: &HeaderMap) -> Option<Result<usize, ParsingError>> {
    headers.get_first_header_value(&header::CONTENT_LENGTH)
        .map(|value| value.parse().map_err(|_| ParsingError::InvalidHeaderValue))
}

/// Checks if the header map has chunked transfer encoding header value.
fn is_chunked_transfer_encoding(headers: &HeaderMap) -> bool {
    headers.get_first_header_value(&header::TRANSFER_ENCODING)
        .map(|v| v.eq("chunked")).unwrap_or(false)
}

impl Parse<Vec<u8>> for BodyParser {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<Vec<u8>, Self> {
        let mut reader = reader.error_take((MAX_BODY_SIZE - self.read_so_far()) as u64);

        Ok(match self {
            WithSize(parser) => parser.parse(&mut reader)?.map_blocked(|parser| WithSize(parser)),
            UntilEof(parser) => parser.parse(&mut reader)?.map_blocked(|parser| UntilEof(parser)),
            Chunked(parser) => parser.parse(&mut reader)?.map_blocked(|parser| Chunked(parser)),
            Empty => Done(vec![])
        })
    }
}

/// Chunked transfer-encoding body parser.
/// A chunked body might look like:
/// A\r\n
/// 0123456789\r\n
/// 0\r\n
/// \r\n
mod chunked {
    use std::io::BufRead;

    use crate::parse::body::chunked::State::{Data, Finished, Size, TailingCrlf};
    use crate::parse::body::MAX_BODY_SIZE;
    use crate::parse::crlf_line::CrlfLineParser;
    use crate::parse::deframe::bytes::BytesDeframer;
    use crate::parse::error::ParsingError;
    use crate::parse::parse::{Parse, ParseResult};
    use crate::parse::parse::ParseStatus::{Done, IoErr};

    /// A parser for chunked transfer-encoding body.
    pub struct ChunksParser {
        body: Vec<u8>,
        state: State,
    }

    /// The state of the chunk parser.
    enum State {
        /// The size of the chunk is being parsed.
        Size(CrlfLineParser),
        /// The content of the chunk is being parsed.
        Data(BytesDeframer),
        /// The tailing CRLF after the data is being parsed.
        TailingCrlf(CrlfLineParser, bool),
        /// A 0 length chunk has been parsed last and there are no more chunks to parse.
        Finished,
    }

    impl ChunksParser {
        /// Creates a new chunk parser.
        pub fn new() -> ChunksParser {
            ChunksParser { body: vec![], state: Size(CrlfLineParser::new()) }
        }

        /// The size of the body collected by the chunk parser so far.
        pub fn data_so_far(&self) -> usize {
            self.body.len()
        }
    }

    impl Parse<Vec<u8>> for ChunksParser {
        fn parse(self, reader: &mut impl BufRead) -> ParseResult<Vec<u8>, Self> {
            let ChunksParser { mut state, mut body } = self;

            loop {
                let result = match state {
                    Size(parser) => size_state(reader, parser)?,
                    Data(parser) => data_state(reader, parser, &mut body)?,
                    TailingCrlf(parser, is_last) => tailing_crlf_state(reader, parser, is_last)?,
                    Finished => return Ok(Done(body))
                };

                state = match result {
                    Done(state) => state,
                    IoErr(state, err) => return Ok(IoErr(Self { state, body }, err))
                }
            }
        }
    }

    /// Parses the size of a chunk and returns either a Data state or the current Size state if blocked.
    fn size_state(reader: &mut impl BufRead, parser: CrlfLineParser) -> ParseResult<State, State> {
        Ok(match parser.parse(reader)? {
            Done(raw) => Done(Data(BytesDeframer::new(parse_chunk_size(raw)?))),
            IoErr(parser, err) => IoErr(Size(parser), err)
        })
    }

    /// Parses the content of a chunk and returns either a TailingCrlf state or the current Data state if blocked.
    fn data_state(reader: &mut impl BufRead, parser: BytesDeframer, body: &mut Vec<u8>) -> ParseResult<State, State> {
        Ok(match parser.parse(reader)? {
            Done(ref mut data) => {
                let is_last = data.is_empty();
                body.append(data);
                Done(TailingCrlf(CrlfLineParser::new(), is_last))
            }
            IoErr(parser, err) => IoErr(Data(parser), err)
        })
    }

    /// Parses the tailing CRLF after a chunks content and returns either a Finished state, a Size state, or the current Data state if blocked.
    /// Returns a parsing error if the CRLF contains any extra data before it.
    fn tailing_crlf_state(reader: &mut impl BufRead, parser: CrlfLineParser, is_last: bool) -> ParseResult<State, State> {
        Ok(match parser.parse(reader)? {
            Done(line) if !line.is_empty() => Err(ParsingError::BadSyntax)?,
            Done(_) if is_last => Done(Finished),
            Done(_) => Done(Size(CrlfLineParser::new())),
            IoErr(parser, err) => IoErr(TailingCrlf(parser, is_last), err)
        })
    }

    /// Parses the chunk size from the given string.
    fn parse_chunk_size(raw: String) -> Result<usize, ParsingError> {
        let size = usize::from_str_radix(&raw, 16).map_err(|_| ParsingError::InvalidChunkSize)?;
        if size > MAX_BODY_SIZE {
            return Err(ParsingError::InvalidChunkSize);
        }
        Ok(size)
    }
}


#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use crate::header_map;
    use crate::parse::body::BodyParser;
    use crate::parse::error::ParsingError::{BadSyntax, ContentLengthTooLarge, InvalidChunkSize};
    use crate::parse::test_util;
    use crate::parse::test_util::TestParseResult;
    use crate::parse::test_util::TestParseResult::{IoErr, ParseErr, Value};

    fn test_sized(size: usize, tests: Vec<(Vec<&[u8]>, TestParseResult<Vec<u8>>)>) {
        let parser = BodyParser::new(&header_map![("content-length", size.to_string())], false).unwrap();
        test_util::test_blocking(parser, tests);
    }

    fn test_until_eof(tests: Vec<(Vec<&[u8]>, TestParseResult<Vec<u8>>)>) {
        let parser = BodyParser::new(&header_map![], true).unwrap();
        test_util::test_blocking(parser, tests);
    }

    fn test_chunked(tests: Vec<(Vec<&[u8]>, TestParseResult<Vec<u8>>)>) {
        let parser = BodyParser::new(&header_map![("transfer-encoding", "chunked")], false).unwrap();
        test_util::test_blocking(parser, tests);
    }

    fn test_endless(parser: BodyParser, start: Vec<&[u8]>, sequence: &[u8], expected: TestParseResult<Vec<u8>>) {
        test_util::test_endless_bytes(parser, start, sequence, expected);
    }

    #[test]
    fn sized_body_all_at_once() {
        test_sized(11, vec![
            (vec![b"hello world"], Value(b"hello world".to_vec()))
        ])
    }

    #[test]
    fn stops_reading_once_size_is_reached() {
        test_sized(11, vec![
            (vec![b"hello worldhello world"], Value(b"hello world".to_vec())),
        ])
    }

    #[test]
    fn sized_body_all_at_once_fragmented() {
        test_sized(11, vec![
            (vec![b"h", b"el", b"lo", b" w", b"or", b"ld"], Value(b"hello world".to_vec()))
        ])
    }

    #[test]
    fn sized_body_partial() {
        test_sized(11, vec![
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"h", b"ell"], ErrorKind::WouldBlock.into()),
            (vec![b"o"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b" w", b"o", b"rl"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"d"], Value(b"hello world".to_vec())),
        ])
    }

    #[test]
    fn sized_body_eof_before_size_reached() {
        test_sized(11, vec![
            (vec![b"h", b"ell"], ErrorKind::WouldBlock.into()),
            (vec![b"o"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b" w", b"o", b"rl"], ErrorKind::WouldBlock.into()),
            (vec![b""], IoErr(Error::from(ErrorKind::UnexpectedEof))),
        ])
    }

    #[test]
    fn sized_body_more_data_than_size() {
        test_sized(11, vec![
            (vec![b"h", b"ell"], ErrorKind::WouldBlock.into()),
            (vec![b"o"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b" w", b"o", b"rl"], ErrorKind::WouldBlock.into()),
            (vec![b"dblahblahblah"], Value(b"hello world".to_vec())),
        ])
    }

    #[test]
    fn sized_body_eof_before_any_data() {
        test_sized(11, vec![
            (vec![b""], IoErr(Error::from(ErrorKind::UnexpectedEof))),
        ])
    }

    #[test]
    fn sized_body_too_big() {
        let res = BodyParser::new(&header_map![("content-length", usize::max_value().to_string())], false);
        assert_eq!(format!("{:?}", res.err().unwrap()), format!("{:?}", ContentLengthTooLarge))
    }

    #[test]
    fn until_eof_all_at_once_with_eof() {
        test_until_eof(vec![
            (vec![b"hello ", b"blah ", b"blah", b" blah", b""], Value(b"hello blah blah blah".to_vec()))
        ])
    }

    #[test]
    fn until_eof_partial() {
        test_until_eof(vec![
            (vec![b"hello "], ErrorKind::WouldBlock.into()),
            (vec![b"he", b"l", b"lo"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![b"hello"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b""], Value(b"hello hello hello".to_vec()))
        ])
    }

    #[test]
    fn until_eof_endless() {
        let body_reader = BodyParser::new(&header_map![], true).unwrap();
        test_endless(body_reader, vec![], b"blah", IoErr(Error::new(ErrorKind::Other, "read limit reached")))
    }

    #[test]
    fn no_content_length_should_not_read_until_eof() {
        let body_reader = BodyParser::new(&header_map![], false).unwrap();
        test_endless(body_reader, vec![], b"blah", Value(vec![]))
    }

    #[test]
    fn chunks_partial() {
        test_chunked(vec![
            (vec![b"5\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"hello"], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"1\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"5\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"world"], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"0\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], Value(b"hello world".to_vec())),
        ]);
    }

    #[test]
    fn chunks_partial_no_data_sometimes() {
        test_chunked(vec![
            (vec![b"5\r\n"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"hello"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"1\r\n"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b" "], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"5\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"world"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"0\r\n"], ErrorKind::WouldBlock.into()),
            (vec![], ErrorKind::WouldBlock.into()),
            (vec![b"\r\n"], Value(b"hello world".to_vec())),
        ]);
    }

    #[test]
    fn chunks_all_at_once() {
        test_chunked(vec![
            (vec![b"5\r\nhello\r\n1\r\n \r\n5\r\nworld\r\n0\r\n\r\n"], Value(b"hello world".to_vec())),
        ]);
    }

    #[test]
    fn chunks_all_at_once_fragmented() {
        test_chunked(vec![
            (vec![b"5\r", b"\nhel", b"lo\r", b"\n1\r\n", b" \r\n5", b"\r\nwor", b"ld\r\n", b"0\r\n", b"\r", b"\n"], Value(b"hello world".to_vec())),
        ]);
    }

    #[test]
    fn one_empty_chunk() {
        test_chunked(vec![
            (vec![b"0\r\n", b"\r\n"], Value(vec![]))
        ]);
    }

    #[test]
    fn chunk_size_in_hex() {
        test_chunked(vec![
            (vec![b"f\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"fifteen letters\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"0\r\n\r\n"], Value(b"fifteen letters".to_vec()))
        ]);
    }

    #[test]
    fn stops_reading_at_empty_chunk() {
        test_chunked(vec![
            (vec![b"5\r\n", b"hello\r\n", b"0\r\n\r\n", b"7\r\n", b"goodbye\r\n", b"0\r\n\r\n"], Value(b"hello".to_vec())),
        ]);
    }

    #[test]
    fn chunk_one_byte_at_a_time() {
        test_chunked(vec![
            (vec![b"a"], ErrorKind::WouldBlock.into()),
            (vec![b"\r"], ErrorKind::WouldBlock.into()),
            (vec![b"\n"], ErrorKind::WouldBlock.into()),
            (vec![b"0"], ErrorKind::WouldBlock.into()),
            (vec![b"1"], ErrorKind::WouldBlock.into()),
            (vec![b"2"], ErrorKind::WouldBlock.into()),
            (vec![b"3"], ErrorKind::WouldBlock.into()),
            (vec![b"4"], ErrorKind::WouldBlock.into()),
            (vec![b"5"], ErrorKind::WouldBlock.into()),
            (vec![b"6"], ErrorKind::WouldBlock.into()),
            (vec![b"7"], ErrorKind::WouldBlock.into()),
            (vec![b"8"], ErrorKind::WouldBlock.into()),
            (vec![b"9"], ErrorKind::WouldBlock.into()),
            (vec![b"\r"], ErrorKind::WouldBlock.into()),
            (vec![b"\n"], ErrorKind::WouldBlock.into()),
            (vec![b"0"], ErrorKind::WouldBlock.into()),
            (vec![b"\r"], ErrorKind::WouldBlock.into()),
            (vec![b"\n"], ErrorKind::WouldBlock.into()),
            (vec![b"\r"], ErrorKind::WouldBlock.into()),
            (vec![b"\n"], Value(b"0123456789".to_vec())),
        ]);
    }

    #[test]
    fn chunk_size_too_large() {
        test_chunked(vec![
            (vec![b"fffffff\r\n"], ParseErr(InvalidChunkSize))
        ]);
    }

    #[test]
    fn endless_chunk_content() {
        let body_reader = BodyParser::new(&header_map![("transfer-encoding", "chunked")], false).unwrap();
        test_endless(body_reader, vec![b"ff\r\n"], b"a", IoErr(Error::new(ErrorKind::Other, "read limit reached")));
    }

    #[test]
    fn endless_chunks() {
        let body_reader = BodyParser::new(&header_map![("transfer-encoding", "chunked")], false).unwrap();
        test_endless(body_reader, vec![], b"1\r\na\r\n", IoErr(Error::new(ErrorKind::Other, "read limit reached")));
    }

    #[test]
    fn chunk_body_too_large() {
        test_chunked(vec![
            (vec![b"5\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"helloo\r\n"], ParseErr(BadSyntax)),
        ]);
    }

    #[test]
    fn chunk_body_too_short() {
        test_chunked(vec![
            (vec![b"5\r\n"], ErrorKind::WouldBlock.into()),
            (vec![b"hell\r\n"], ParseErr(BadSyntax)),
        ]);
    }

    #[test]
    fn zero_content_length_no_data() {
        test_sized(0, vec![
            (vec![], Value(vec![]))
        ])
    }

    #[test]
    fn zero_content_length_eof() {
        test_sized(0, vec![
            (vec![b""], Value(vec![]))
        ])
    }

    #[test]
    fn zero_content_length_with_data() {
        test_sized(0, vec![
            (vec![b"h", b"ell"], Value(vec![]))
        ])
    }
}