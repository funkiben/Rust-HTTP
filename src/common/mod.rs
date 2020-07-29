pub const HTTP_VERSION: &str = "HTTP/1.1";

/// HTTP header data types and functions.
pub mod header;
/// HTTP method data type and functions.
pub mod method;
/// HTTP request data type and functions.
pub mod request;
/// HTTP response data type and functions
pub mod response;
/// HTTP status data type and functions.
pub mod status;
/// Basic HTTP message parsing functions for requests and responses.
pub(crate) mod parse;
