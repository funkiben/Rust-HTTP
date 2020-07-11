use std::time::Duration;

use crate::common::response::Response;

/// The config for an HTTP server.
pub struct Config {
    /// The address to bind the server listener to.
    pub addr: &'static str,
    /// The number of threads to spawn for handling connections. Each thread is used for one
    /// connection at a time.
    pub connection_handler_threads: usize,
    /// The timeout for a single blocking read call.
    pub read_timeout: Duration,
    /// The response to send when there is no response produced by the router.
    pub no_route_response: Response,
}