use crate::common::header::HeaderMap;
use crate::common::method::Method;

/// An HTTP request.
#[derive(Debug, Eq, PartialEq)]
pub struct Request {
    /// The URI.
    pub uri: String,
    /// The method.
    pub method: Method,
    /// The headers.
    pub headers: HeaderMap,
    /// The body.
    pub body: Vec<u8>,
}