use std::io::Error;

/// Error for when an HTTP message can't be parsed.
#[derive(Debug)]
pub enum ParsingError {
    /// Invalid syntax in the message.
    BadSyntax,
    /// Message has wrong HTTP version.
    WrongHttpVersion,
    /// Header has invalid value.
    InvalidHeaderValue,
    /// EOF found before any request or response can be read.
    EOF,
    /// Size of chunk in chunked transfer encoding can not be parsed as a number.
    InvalidChunkSize,
    /// Error reading from the reader.
    Reading(Error),
}

/// Possible errors that can be encountered when trying to parse a request.
#[derive(Debug)]
pub enum RequestParsingError {
    /// Method is unrecognized.
    UnrecognizedMethod,
    /// Base error.
    Base(ParsingError),
}

/// Possible errors that can be encountered when trying to parse a response.
#[derive(Debug)]
pub enum ResponseParsingError {
    /// Response had an unknown status code.
    InvalidStatusCode,
    /// Base error.
    Base(ParsingError),
}

impl From<ParsingError> for ResponseParsingError {
    fn from(err: ParsingError) -> Self {
        ResponseParsingError::Base(err)
    }
}

impl From<ParsingError> for RequestParsingError {
    fn from(err: ParsingError) -> Self {
        RequestParsingError::Base(err)
    }
}

impl From<Error> for ParsingError {
    fn from(err: Error) -> Self {
        ParsingError::Reading(err)
    }
}