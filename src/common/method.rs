use std::fmt::{Display, Formatter};

/// An HTTP method.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Method {
    /// GET method.
    GET,
    /// POST method.
    POST,
    /// DELETE method.
    DELETE,
    /// PUT method
    PUT,
}

impl Display for Method {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}