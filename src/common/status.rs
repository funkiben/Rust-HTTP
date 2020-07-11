/// An HTTP status.
#[derive(Debug, Eq, PartialEq)]
pub struct Status {
    /// The status code.
    pub code: u16,
    /// The reason for the status.
    pub reason: &'static str,
}

/// 200 OK status.
pub const OK_200: Status = Status { code: 200, reason: "OK" };
/// 400 Bad Request status.
pub const BAD_REQUEST_400: Status = Status { code: 400, reason: "BAD REQUEST" };
/// 404 Not Found status.
pub const NOT_FOUND_404: Status = Status { code: 404, reason: "NOT FOUND" };