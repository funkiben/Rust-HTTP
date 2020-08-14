use std::io::BufRead;

use crate::common::header::HeaderMap;
use crate::read::body_reader::BodyReader;
use crate::read::error::ParsingError;
use crate::read::headers_reader::HeadersReader;

pub struct HeadersAndBodyReader {
    read_body_if_no_content_length: bool,
    state: State,
}

enum State {
    Headers(HeadersReader),
    Body(Option<HeaderMap>, BodyReader),
}

impl State {
    fn new() -> State {
        State::Headers(HeadersReader::new())
    }
}

impl HeadersAndBodyReader {
    pub fn new(read_body_if_no_content_length: bool) -> HeadersAndBodyReader {
        HeadersAndBodyReader { read_body_if_no_content_length, state: State::new() }
    }

    pub fn read(&mut self, reader: &mut impl BufRead) -> Result<Option<(HeaderMap, Vec<u8>)>, ParsingError> {
        match &mut self.state {
            State::Headers(headers_reader) => {
                if let Some(headers) = headers_reader.read(reader)? {
                    let body_reader = BodyReader::new(self.read_body_if_no_content_length, &headers)?;
                    self.state = State::Body(Some(headers), body_reader);
                    return self.read(reader);
                }
            }
            State::Body(headers, body_reader) => {
                if let Some(body) = body_reader.read(reader)? {
                    let ret = Ok(Some((headers.take().unwrap(), body)));
                    self.state = State::new();
                    return ret;
                }
            }
        }
        Ok(None)
    }
}