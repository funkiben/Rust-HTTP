#[derive(Debug, Eq, PartialEq)]
pub struct Status {
    pub code: u16,
    pub reason: &'static str,
}

pub const OK_200: Status = Status { code: 200, reason: "OK" };
pub const BAD_REQUEST_400: Status = Status { code: 400, reason: "BAD REQUEST" };
pub const NOT_FOUND_404: Status = Status { code: 404, reason: "NOT FOUND" };