use std::io::BufRead;

use crate::common::header::HeaderMap;
use crate::deframe::body_deframer::BodyDeframer;
use crate::deframe::deframe::Deframe;
use crate::deframe::error::DeframingError;
use crate::deframe::headers_and_body_deframer::HeadersAndBodyDeframer::{Body, Headers};
use crate::deframe::headers_deframer::HeadersDeframer;

/// The state of the headers and body deframer.
pub enum HeadersAndBodyDeframer {
    Headers(HeadersDeframer, bool),
    Body(BodyDeframer, HeaderMap),
}


impl HeadersAndBodyDeframer {
    /// Creates a new headers and body deframer.
    /// If "read_body_if_no_content_length" is true and no content-length is provided, then the body will be read until EOF.
    pub fn new(read_body_if_no_content_length: bool) -> HeadersAndBodyDeframer {
        Headers(HeadersDeframer::new(), read_body_if_no_content_length)
    }
}

impl Deframe<(HeaderMap, Vec<u8>)> for HeadersAndBodyDeframer {
    fn read(self, reader: &mut impl BufRead) -> Result<(HeaderMap, Vec<u8>), (Self, DeframingError)> {
        match self {
            Headers(deframer, read_body_if_no_content_length) => {
                match deframer.read(reader) {
                    Ok(headers) => {
                        let body_deframer = BodyDeframer::new(read_body_if_no_content_length, &headers)
                            .map_err(|err| (Self::new(read_body_if_no_content_length), err))?;
                        Body(body_deframer, headers).read(reader)
                    }
                    Err((deframer, err)) => Err((Headers(deframer, read_body_if_no_content_length), err))
                }
            }
            Body(deframer, headers) =>
                match deframer.read(reader) {
                    Ok(body) => Ok((headers, body)),
                    Err((deframer, err)) => Err((Body(deframer, headers), err))
                }
        }
    }
}