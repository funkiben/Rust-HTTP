use std::borrow::Cow;
use std::io::Read;

use crate::common::header::{Header, HeaderMap, HeaderMapOps, CONTENT_LENGTH, TRANSFER_ENCODING};
use crate::common::HTTP_VERSION;
use crate::common::method::Method;
use crate::common::request::Request;
use crate::deframe::error::{ParsingError, RequestParsingError};

struct RequestDeframer {
    buf: Vec<u8>,
    used: usize,
    state: State,
    requests: Option<Request>,
}

enum State {
    Method,
    URI,
    Version,
    Header,
    Body(BodyState),
    Done,
}

enum BodyState {
    Sized(usize),
    ChunkSize,
    ChunkBody(usize),
    Done(Vec<u8>)
}

impl State {
    fn new() -> State {
        State::Method
    }

    fn get_min_buffer_size(&self) -> usize {
        match self {
            State::Method => 16,
            State::URI => 512,
            State::Version => 16,
            State::Header => 1024,
            State::Body(..) => 8192,
            State::Done => 0
        }
    }

    fn deframe(self, request: &mut Request, mut buf: Vec<u8>, new: usize) -> Result<(State, Vec<u8>), RequestParsingError> {
        let new_bytes = &buf[(buf.len() - new)..];
        match self {
            State::Method => self.deframe_method(request, buf, new_bytes),
            State::URI => self.deframe_uri(request, buf, new_bytes),
            State::Version => self.deframe_version(buf, new_bytes),
            State::Header => self.deframe_header(request, buf, new_bytes),
            State::Body(state) => self.deframe_body(request, state, buf, new_bytes),
            State::Done => Ok((self, buf))
        }
    }

    fn deframe_method(self, request: &mut Request, mut buf: Vec<u8>, new_bytes: &[u8]) -> Result<(State, Vec<u8>), RequestParsingError> {
        if let Some(rest) = split_off_at_first_element(&mut buf, new_bytes, b' ') {
            reqiest.method = parse_method(&buf)?;
            Ok((State::URI, rest))
        } else {
            Ok((self, buf))
        }
    }

    fn deframe_uri(self, request: &mut Request, mut buf: Vec<u8>, new_bytes: &[u8]) -> Result<(State, Vec<u8>), RequestParsingError> {
        if let Some(rest) = split_off_at_first_element(&mut buf, new_bytes, b' ') {
            request.uri = read_utf8(buf)?;
            Ok((State::Version, rest))
        } else {
            Ok((self, buf))
        }
    }

    fn deframe_version(self, mut buf: Vec<u8>, new_bytes: &[u8]) -> Result<(State, Vec<u8>), RequestParsingError> {
        if let Some(rest) = split_off_at_crlf(&mut buf, new_bytes) {
            if buf.eq(HTTP_VERSION) {
                Ok((State::HeaderName, rest))
            } else {
                Err(ParsingError::WrongHttpVersion.into())
            }
        } else {
            Ok((self, buf))
        }
    }

    fn deframe_header(self, request: &mut Request, mut buf: Vec<u8>, new_bytes: &[u8]) -> Result<(State, Vec<u8>), RequestParsingError> {
        if let Some(rest) = split_off_at_crlf(&mut buf, new_bytes) {
            if buf.is_empty() {
                Ok((State::Body(BodyState::new(&request.headers)?), rest))
            } else {
                let (name, value) = parse_header(&buf)?;
                request.headers.add_header(name, value);
                Ok((State::Header, rest))
            }
        } else {
            Ok((self, buf))
        }
    }

    fn deframe_body(self, request: &mut Request, body_state: BodyState, mut buf: Vec<u8>, new_bytes: &[u8]) -> Result<(State, Vec<u8>), RequestParsingError> {
        let (body_state, buf) = body_state.deframe(buf, new_bytes.len())?;
        if let BodyState::Done(body) = body_state {
            request.body = body;
            Ok((State::Done, buf))
        } else {
            Ok((self, buf))
        }
    }
}

impl BodyState {
    fn new(headers: &HeaderMap) -> Result<BodyState, RequestParsingError> {
        if let Some(size) = get_content_length(headers) {
            Ok(BodyState::Sized(size?))
        } else if is_chunked_transfer_encoding(headers) {
            Ok(BodyState::ChunkSize)
        } else {
            Ok(BodyState::Done(vec![]))
        }
    }

    fn deframe(self, mut buf: Vec<u8>, new: usize) -> Result<(BodyState, Vec<u8>), RequestParsingError> {
        let new_bytes = &buf[(buf.len() - new)..];
        match self {
            BodyState::Sized => self.deframe()
        }
    }

    fn deframe_body(self, request: &mut Request, mut buf: Vec<u8>, new_bytes: &[u8]) -> Result<(State, Vec<u8>), RequestParsingError> {
        let (body_state, buf) = body_state.deframe(buf, new_bytes.len())?;
        if let BodyState::Done(body) = body_state {
            request.body = body;
            Ok((State::Done, buf))
        } else {
            Ok((self, buf))
        }
    }

}

fn split_off_at_crlf(buf: &mut Vec<u8>, new: &[u8]) -> Option<Vec<u8>> {
    if let Some(rest) = split_off_at_first_element(buf, new, b'\n') {
        buf.pop();
        Some(rest)
    }
    None
}

fn split_off_at_colon_space(buf: &mut Vec<u8>, new: &[u8]) -> Option<Vec<u8>> {
    if let Some(rest) = split_off_at_first_element(buf, new, b' ') {
        buf.pop();
        Some(rest)
    }
    None
}

fn split_off_at_first_element(buf: &mut Vec<u8>, new: &[u8], byte: u8) -> Option<Vec<u8>> {
    for i in 0..new.len() {
        if new[i] == byte {
            let rest = buf.split_off(buf.len() - new.len() + i + 1);
            buf.pop();
            return Some(rest);
        }
    }
    None
}

/// Parses the given string into a method. If the method is not recognized, will return an error.
fn parse_method(raw: &[u8]) -> Result<Method, RequestParsingError> {
    match raw {
        b"GET" => Ok(Method::GET),
        b"PUT" => Ok(Method::PUT),
        b"DELETE" => Ok(Method::DELETE),
        b"POST" => Ok(Method::POST),
        _ => Err(RequestParsingError::UnrecognizedMethod)
    }
}

/// Parses the given line as a header. Splits the line at the first ": " pattern.
fn parse_header(raw: &[u8]) -> Result<(Header, String), ParsingError> {
    let mut split = raw.splitn(2, |b| b == b' ');

    let mut header_raw = split.next().ok_or(ParsingError::BadSyntax)?.to_vec();
    let value = split.next().ok_or(ParsingError::BadSyntax)?.to_vec();

    match header_raw.pop() {
        Some(b':') => {},
        _ => return Err(ParsingError::BadSyntax)
    }

    Ok((Header::from(read_utf8(header_raw)?), read_utf8(value.to_vec())?))
}


fn read_utf8(bytes: Vec<u8>) -> Result<String, ParsingError> {
    String::from_utf8(bytes).map_err(|_| ParsingError::BadSyntax)
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