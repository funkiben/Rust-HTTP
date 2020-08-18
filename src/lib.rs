/// HTTP data types.
pub mod common;
/// Components for running an HTTP server and handling requests.
pub mod server;
/// Components for communicating with an HTTP server.
pub mod client;

/// Utility components.
pub(crate) mod util;

/// Components for parsing HTTP requests and responses.
pub(crate) mod parse;