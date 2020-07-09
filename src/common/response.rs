use crate::common::header::{CONTENT_LENGTH, HeaderMap, HeaderMapOps};
use crate::common::status::Status;

#[derive(Debug, Clone)]
pub struct Response {
    pub status: &'static Status,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Response {
    pub fn from(status: &'static Status) -> Response {
        Response {
            status,
            headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, String::from("0"))]),
            body: vec![],
        }
    }
}