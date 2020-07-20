use std::fmt::{Display, Formatter};

/// An HTTP method.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Method {
    /// GET method.
    Get,
    /// POST method.
    Post,
    /// DELETE method.
    Delete,
    /// PUT method
    Put,
}

impl Display for Method {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}