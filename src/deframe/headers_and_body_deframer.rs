use std::io::BufRead;

use crate::common::header::HeaderMap;
use crate::deframe::body_deframer::BodyDeframer;
use crate::deframe::error::DeframingError;
use crate::deframe::headers_deframer::HeadersDeframer;

/// Deframer for the headers and body of an HTTP request or response.
pub struct HeadersAndBodyDeframer {
    read_body_if_no_content_length: bool,
    state: State,
}

/// The state of the headers and body deframer.
enum State {
    Headers(HeadersDeframer),
    Body(Option<HeaderMap>, BodyDeframer),
}

impl HeadersAndBodyDeframer {
    /// Creates a new headers and body deframer.
    /// If "read_body_if_no_content_length" is true and no content-length is provided, then the body will be read until EOF.
    pub fn new(read_body_if_no_content_length: bool) -> HeadersAndBodyDeframer {
        HeadersAndBodyDeframer { read_body_if_no_content_length, state: State::Headers(HeadersDeframer::new()) }
    }

    /// Reads data from the reader and tries to deframe headers and a body.
    pub fn read(&mut self, reader: &mut impl BufRead) -> Result<(HeaderMap, Vec<u8>), DeframingError> {
        loop {
            match &mut self.state {
                State::Headers(headers_reader) => {
                    let headers = headers_reader.read(reader)?;
                    let body_reader = BodyDeframer::new(self.read_body_if_no_content_length, &headers)?;
                    self.state = State::Body(Some(headers), body_reader);
                    continue;
                }
                State::Body(headers, body_reader) => {
                    let body = body_reader.read(reader)?;
                    let ret = Ok((headers.take().unwrap(), body));
                    self.state = State::Headers(HeadersDeframer::new());
                    return ret;
                }
            }
        }
    }
}