use crate::common::header::{CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps, TRANSFER_ENCODING};
use crate::common::HTTP_VERSION;
use crate::common::method::Method;
use crate::common::request::Request;
use crate::deframe::error::{ParsingError, RequestParsingError};

struct RequestDeframer {
    buf: Vec<u8>,
    used: usize,
    state: State,
    request: Option<Request>,
}

#[derive(Copy, Clone)]
enum State {
    Method,
    URI,
    Version,
    Header,
    Body(BodyState),
    Done,
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

    fn deframe(self, request: &mut Request, buf: Vec<u8>, new: usize) -> Result<(State, Vec<u8>), RequestParsingError> {
        match self {
            State::Method => self.deframe_method(request, buf, new),
            State::URI => self.deframe_uri(request, buf, new),
            State::Version => self.deframe_version(buf, new),
            State::Header => self.deframe_header(request, buf, new),
            State::Body(state) => self.deframe_body(request, state, buf, new),
            State::Done => Ok((self, buf))
        }
    }

    fn deframe_method(self, request: &mut Request, buf: Vec<u8>, new: usize) -> Result<(State, Vec<u8>), RequestParsingError> {
        self.try_deframe(
            request, buf,
            |buf| split_off_at_first_element(buf, new, b' '),
            |req, buf| {
                request.method = parse_method(&buf)?;
                Ok(State::URI)
            })
    }

    fn deframe_uri(self, request: &mut Request, buf: Vec<u8>, new: usize) -> Result<(State, Vec<u8>), RequestParsingError> {
        self.try_deframe(
            request, buf,
            |buf| split_off_at_first_element(buf, new, b' '),
            |req, buf| {
                request.uri = read_utf8(buf)?;
                Ok(State::Version)
            })
    }

    fn deframe_version(self, buf: Vec<u8>, new: usize) -> Result<(State, Vec<u8>), RequestParsingError> {
        self.try_deframe(
            request, buf,
            |buf| split_off_at_crlf(buf, new),
            |req, buf| {
                if buf.eq(&HTTP_VERSION.as_bytes()) {
                    Ok(State::Header)
                } else {
                    Err(ParsingError::WrongHttpVersion.into())
                }
            })
    }

    fn deframe_header(self, request: &mut Request, mut buf: Vec<u8>, new: usize) -> Result<(State, Vec<u8>), RequestParsingError> {
        if let Some(rest) = split_off_at_crlf(&mut buf, new) {
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

    fn deframe_body(self, request: &mut Request, body_state: BodyState, mut buf: Vec<u8>, new: usize) -> Result<(State, Vec<u8>), RequestParsingError> {
        let (body_state, buf) = body_state.deframe(&mut request.body, buf, new)?;
        if let BodyState::Done = body_state {
            Ok((State::Done, buf))
        } else {
            Ok((self, buf))
        }
    }

    fn try_deframe<F, J>(self, request: &mut Request, mut buf: Vec<u8>, try_split: F, parse: J) -> Result<(State, Vec<u8>), RequestParsingError>
        where
            F: Fn(&mut Vec<u8>, usize) -> Option<Vec<u8>>,
            J: Fn(&mut Request, Vec<u8>) -> Result<State, RequestParsingError> {
        if let Some(rest) = try_split(&mut buf) {
            let next = parse(request, buf)?;
            Ok((next, rest))
        } else {
            Ok((self, buf))
        }
    }
}

#[derive(Copy, Clone)]
enum BodyState {
    Sized(usize),
    ChunkSize,
    ChunkBody(usize),
    Done,
}

impl BodyState {
    fn new(headers: &HeaderMap) -> Result<BodyState, RequestParsingError> {
        if let Some(size) = get_content_length(headers) {
            Ok(BodyState::Sized(size?))
        } else if is_chunked_transfer_encoding(headers) {
            Ok(BodyState::ChunkSize)
        } else {
            Ok(BodyState::Done)
        }
    }

    fn deframe(self, body: &mut Vec<u8>, mut buf: Vec<u8>, new: usize) -> Result<(BodyState, Vec<u8>), RequestParsingError> {
        match self {
            BodyState::Sized(size) => self.deframe_sized(body, size, buf),
            BodyState::ChunkSize => self.deframe_chunk_size(buf, new),
            BodyState::ChunkBody(size) => self.deframe_chunk_body(body, size, buf, new),
            BodyState::Done => Ok((BodyState::Done, buf))
        }
    }

    fn deframe_sized(self, body: &mut Vec<u8>, size: usize, mut buf: Vec<u8>) -> Result<(BodyState, Vec<u8>), RequestParsingError> {
        if buf.len() >= size {
            let rest = buf.split_off(size);
            *body = buf;
            Ok((BodyState::Done, rest))
        } else {
            Ok((self, buf))
        }
    }

    fn deframe_chunk_size(self, mut buf: Vec<u8>, new: usize) -> Result<(BodyState, Vec<u8>), RequestParsingError> {
        if let Some(rest) = split_off_at_crlf(&mut buf, new) {
            let size = usize::from_str_radix(&read_utf8(buf)?, 16).map_err(|_| ParsingError::InvalidChunkSize)?;
            Ok((BodyState::ChunkBody(size), rest))
        } else {
            Ok((self, buf))
        }
    }

    fn deframe_chunk_body(self, body: &mut Vec<u8>, size: usize, mut buf: Vec<u8>, new: usize) -> Result<(BodyState, Vec<u8>), RequestParsingError> {
        if let Some(rest) = split_off_at_crlf(&mut buf, new) {
            if buf.len() != size + 2 {
                Err(ParsingError::BadSyntax.into())
            } else if size == 0 {
                Ok((BodyState::Done, rest))
            } else {
                body.append(&mut buf);
                Ok((BodyState::ChunkSize, rest))
            }
        } else {
            Ok((self, buf))
        }
    }
}

fn split_off_at_crlf(buf: &mut Vec<u8>, new: usize) -> Option<Vec<u8>> {
    if let Some(rest) = split_off_at_first_element(buf, new, b'\n') {
        buf.pop();
        Some(rest)
    } else {
        None
    }
}

fn split_off_at_colon_space(buf: &mut Vec<u8>, new: usize) -> Option<Vec<u8>> {
    if let Some(rest) = split_off_at_first_element(buf, new, b' ') {
        buf.pop();
        Some(rest)
    } else {
        None
    }
}

fn split_off_at_first_element(buf: &mut Vec<u8>, new: usize, byte: u8) -> Option<Vec<u8>> {
    for i in (buf.len() - new)..buf.len() {
        if buf[i] == byte {
            let rest = buf.split_off(i);
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
    let mut split = raw.splitn(2, |b| b.eq(&b' '));

    let mut header_raw = split.next().ok_or(ParsingError::BadSyntax)?.to_vec();
    let value = split.next().ok_or(ParsingError::BadSyntax)?.to_vec();

    match header_raw.pop() {
        Some(b':') => {}
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

trait Deframer {
    fn try_split(&self, buf: &mut Vec<u8>, new: usize) -> Option<Vec<u8>>;

    fn parse(&self, request: &mut Request, buf: Vec<u8>) -> Result<State, RequestParsingError>;
}

struct MethodDeframer;

impl Deframer for MethodDeframer {
    fn try_split(&self, buf: &mut Vec<u8>, new: usize) -> Option<Vec<u8>> {
        split_off_at_first_element(buf, new, b' ')
    }

    fn parse(&self, request: &mut Request, buf: Vec<u8>) -> Result<State, RequestParsingError> {
        request.method = parse_method(&buf)?;
        Ok(State::URI)
    }
}