use std::io::{Error, ErrorKind};

/// Error for when an HTTP message can't be parsed.
#[derive(Debug)]
pub enum ParsingError {
    /// Invalid syntax in the message.
    BadSyntax,
    /// Message has wrong HTTP version.
    WrongHttpVersion,
    /// Header has invalid value.
    InvalidHeaderValue,
    /// EOF found before any part of a request or response can be deframed.
    EOF,
    /// Size of chunk in chunked transfer encoding can not be parsed as a number.
    InvalidChunkSize,
    /// Content length exceeds maximum size.
    ContentLengthTooLarge,
    /// Method is unrecognized.
    UnrecognizedMethod,
    /// Invalid status code.
    InvalidStatusCode,
    /// IO error from reader.
    Reading(std::io::Error),
}

impl From<std::io::Error> for ParsingError {
    fn from(err: Error) -> Self {
        ParsingError::Reading(err)
    }
}

impl From<std::io::ErrorKind> for ParsingError {
    fn from(kind: ErrorKind) -> Self {
        ParsingError::Reading(Error::from(kind))
    }
}