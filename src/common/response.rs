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
    pub fn from_status(status: Status) -> Self {
        Response {
            status,
            headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, String::from("0"))]),
            body: vec![],
        }
    }
}