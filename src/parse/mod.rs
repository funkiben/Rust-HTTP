/// Parsing errors.
pub mod error;
/// Parse trait.
pub mod parse;

mod crlf_line;
mod headers;
mod body;
mod deframe;
mod error_take;
mod message;

/// Request parsing components.
pub mod request;
/// Response parsing components.
pub mod response;

/// Utility for testing parsers.
#[cfg(test)]
mod test_util;