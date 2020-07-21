use std::time::Duration;

/// Config for an HTTP client.
pub struct Config {
    /// The address to connect to.
    pub addr: &'static str,
    /// The timeout for reading a response.
    pub read_timeout: Duration,
    /// The number of connections to open to the server.
    pub num_connections: usize,
}