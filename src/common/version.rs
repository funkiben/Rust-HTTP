/// HTTP version "HTTP/1.0"
pub const HTTP_VERSION_1_0: &str = "HTTP/1.0";
/// HTTP version "HTTP/1.1"
pub const HTTP_VERSION_1_1: &str = "HTTP/1.1";

/// Checks if the given raw version string is supported.
pub fn is_supported(raw: &str) -> bool {
    HTTP_VERSION_1_1.eq(raw) || HTTP_VERSION_1_0.eq(raw)
}