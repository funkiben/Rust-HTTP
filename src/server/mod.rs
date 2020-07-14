/// Config for HTTP servers.
pub use config::Config;
/// HTTP server.
pub use server::Server;

mod server;
mod config;

/// Router for handling requests sent to an HTTP server.
pub mod router;
mod oos;
