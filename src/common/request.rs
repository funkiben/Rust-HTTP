use crate::common::method::Method;
use crate::common::header::HeaderMap;

#[derive(Debug, Eq, PartialEq)]
pub struct Request {
    pub uri: String,
    pub method: Method,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}