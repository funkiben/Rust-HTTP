/// Error for when an HTTP message can't be parsed.
#[derive(Debug)]
pub enum ParsingError {
    /// Invalid syntax in the message.
    BadSyntax,
    /// Message has wrong HTTP version.
    WrongHttpVersion,
    /// Header has invalid value.
    InvalidHeaderValue,
    /// Size of chunk in chunked transfer encoding can not be parsed as a number.
    InvalidChunkSize,
    /// Content length exceeds maximum size.
    ContentLengthTooLarge,
    /// Method is unrecognized.
    UnrecognizedMethod,
    /// Invalid status code.
    InvalidStatusCode,
    /// Data is not valid UTF8.
    InvalidUtf8,
}