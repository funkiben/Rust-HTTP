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