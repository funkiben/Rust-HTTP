/// Error for when an HTTP message can't be parsed.
#[derive(Debug)]
pub enum DeframingError {
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
    /// Error reading from the reader.
    Reading(std::io::Error),
    /// Method is unrecognized.
    UnrecognizedMethod,
    /// Invalid status code.
    InvalidStatusCode
}

impl From<std::io::Error> for DeframingError {
    fn from(err: std::io::Error) -> Self {
        DeframingError::Reading(err)
    }
}