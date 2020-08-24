/// Parsing errors.
pub mod error;
/// Parse trait and other basic parsing types.
pub mod parse;
/// Request parsing components.
pub mod request;
/// Response parsing components.
pub mod response;

/// Parser for CRLF lines.
mod crlf_line;
/// Parser for headers.
mod headers;
/// Parser for message bodies.
mod body;
/// Deframing (or, in other words, stateful IO reading)
mod deframe;
/// error_take method utility.
mod error_take;
/// Generic parser for HTTP messages. (Request and response parsers compose over this)
mod message;

/// Utility for testing parsers.
#[cfg(test)]
mod test_util;