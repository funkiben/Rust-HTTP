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
    /// Unexpected EOF will be thrown when EOF is found in the middle of reading a request or response.
    UnexpectedEOF,
    /// EOF found before any request or response can be read.
    EOF,
    /// Size of chunk in chunked transfer encoding can not be parsed as a number.
    InvalidChunkSize,
    /// Error reading from the reader.
    Reading(Error),
}

/// The possible errors that can be encountered when trying to parse a request.
#[derive(Debug)]
pub enum RequestParsingError {
    /// Method is unrecognized.
    UnrecognizedMethod(String),
    /// Base parsing error.
    Base(ParsingError),
}

/// Error when parsing an HTTP response from a server.
#[derive(Debug)]
pub enum ResponseParsingError {
    /// Response had an unknown status code.
    InvalidStatusCode,
    /// Base parsing error.
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