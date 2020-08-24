/// Basic thread pool utility.
pub mod thread_pool;

/// Utility for creating mock trait implementations.
#[cfg(test)]
pub mod mock;

/// Stream that automatically handles TLS.
pub mod tls_stream;