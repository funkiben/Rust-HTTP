use std::net::SocketAddr;

/// Config for an HTTP client.
pub struct Config {
    /// The address to connect to.
    pub addr: SocketAddr,
    /// The number of connections to open to the server.
    pub num_connections: usize,
}