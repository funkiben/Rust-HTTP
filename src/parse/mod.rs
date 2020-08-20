/// Parsing errors.
pub mod error;
/// Parse trait and other basic parsing types.
pub mod parse;
/// Request parsing components.
pub mod request;
/// Response parsing components.
pub mod response;

mod crlf_line;
mod headers;
mod body;
mod deframe;
mod error_take;
mod message;

/// Utility for testing parsers.
#[cfg(test)]
mod test_util;