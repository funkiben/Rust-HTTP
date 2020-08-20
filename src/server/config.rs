use rustls::ServerConfig;

/// The config for an HTTP server.
pub struct Config {
    /// The address to bind the server listener to.
    pub addr: &'static str,
    /// The number of threads to spawn for handling connections. Each thread is used for one
    /// connection at a time.
    pub connection_handler_threads: usize,
    /// Config for TLS encryption to enable HTTPS. If this is not set then normal HTTP will be used.
    pub tls_config: Option<ServerConfig>,
}