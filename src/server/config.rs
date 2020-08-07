use std::net::SocketAddr;

use tokio_rustls::rustls::ServerConfig;

/// The config for an HTTP server.
pub struct Config {
    /// The address to bind the server listener to.
    pub addr: SocketAddr,
    /// Config for TLS encryption to enable HTTPS. If this is not set then normal HTTP will be used.
    pub tls_config: Option<ServerConfig>,
}