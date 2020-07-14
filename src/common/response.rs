use crate::common::header::{CONTENT_LENGTH, HeaderMap, HeaderMapOps};
use crate::common::status::Status;

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

impl Response {
    /// Creates a response with the given status. The response will have no body.
    pub fn from(status: Status) -> Response {
        Response {
            status,
            headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, String::from("0"))]),
            body: vec![],
        }
    }
}