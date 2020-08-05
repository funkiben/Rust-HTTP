use crate::common::header::{CONTENT_LENGTH, HeaderMap};
use crate::common::status;
use crate::common::status::Status;
use crate::header_map;

/// An HTTP response.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Response {
    /// The status.
    pub status: Status,
    /// The headers.
    pub headers: HeaderMap,
    /// The body.
    pub body: Vec<u8>,
}

impl From<Status> for Response {
    /// Creates an empty response with the given status.
    fn from(status: Status) -> Self {
        Response {
            status,
            headers: header_map![(CONTENT_LENGTH, "0")],
            body: vec![],
        }
    }
}

impl From<String> for Response {
    /// Creates a response with the given string as its body.
    fn from(body: String) -> Self {
        body.into_bytes().into()
    }
}

impl From<&str> for Response {
    /// Creates a response with the given string as its body.
    fn from(body: &str) -> Self {
        body.to_string().into()
    }
}

impl From<Vec<u8>> for Response {
    /// Creates a response with the given bytes as its body.
    fn from(body: Vec<u8>) -> Self {
        Response {
            status: status::OK,
            headers: header_map![(CONTENT_LENGTH, body.len().to_string())],
            body,
        }
    }
}