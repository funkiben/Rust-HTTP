use std::io::Read;

use crate::common::header::HeaderMap;
use crate::common::method::Method;
use crate::common::request::Request;
use std::borrow::Cow;
use crate::deframing::error::{ParsingError, RequestParsingError};

struct Deframer {
    buf: Vec<u8>,
    used: usize,
    state: State,
    requests: Option<Request>,
}

enum State<'a> {
    Method,
    URI(Method),
    Version(Method, String),
    Headers(Method, String, String, HeaderMap),
    Body(String, HeaderMap, Vec<u8>),
}

impl State {
    fn new() -> State {
        State::Method
    }

    fn read(self, mut bytes: Vec<u8>, new: usize) -> Result<(State, Vec<u8>), ParsingError> {
        let new_bytes = &bytes[..bytes.len() - new];
        match self {
            State::Method => {
                if let Some(pos) = find_first_character(new_bytes, b' ') {
                    let rest = bytes.split_off(pos);
                    Ok((State::URI(parse_method(&read_utf8(bytes)?)?), rest))
                } else {
                    self
                }
            }
            State::URI(_) => {
                if let Some(pos) = find_first_character(new_bytes, b' ') {
                    State::Headers(String::from_utf8_lossy(bytes[0..pos]))
                } else {
                    self
                }
            }
        }
    }
}

/// Parses the given string into a method. If the method is not recognized, will return an error.
fn parse_method(raw: &str) -> Result<Method, RequestParsingError> {
    Method::try_from_str(raw).ok_or_else(|| RequestParsingError::UnrecognizedMethod(String::from(raw)))
}



fn read_utf8(bytes: Vec<u8>) -> Result<String, ParsingError> {
    String::from_utf8(bytes).map_err(|_| ParsingError::BadSyntax)
}

fn find_first_character(bytes: &[u8], byte: u8) -> Option<usize> {
    for i in 0..bytes.len() {
        if bytes[i] == byte {
            return Some(start + 2);
        }
    }
    None
}
