/// Deframing errors.
pub mod error;
/// Deframer for requests.
pub mod request_deframer;
/// Deframer for responses.
pub mod response_deframer;
/// Deframer for headers and body.
mod headers_and_body_deframer;
/// error_take trait and impl for readers.
mod error_take;
/// Deframer for message bodies.
mod body_deframer;
/// Deframer for headers.
mod headers_deframer;
/// Deframer for CRLF terminated lines.
mod crlf_line_deframer;
/// Abstract deframer for generic HTTP messages.
mod message_deframer;
