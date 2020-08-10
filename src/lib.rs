/// HTTP data types.
pub mod common;
/// Components for running an HTTP server and handling requests.
pub mod server;
/// Components for communicating with an HTTP server.
pub mod client;
/// Components for parsing HTTP requests and responses.
pub(crate) mod parse;

/// Utility components.
pub(crate) mod util;

pub(crate) mod deframing;