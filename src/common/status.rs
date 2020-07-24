/// An HTTP status.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Status {
    /// The status code.
    pub code: u16,
    /// The reason for the status.
    pub reason: &'static str,
}

macro_rules! status_codes {
    (
        $(
            $(#[$docs:meta])*
            ($name:ident, $num:expr, $phrase:expr);
        )+
    ) => {
        $(
            $(#[$docs])*
            pub const $name: Status = Status { code: $num, reason: $phrase };
        )+

        /// Gets the status from the given status code.
        impl Status {
            pub fn from_code(code: u16) -> Option<Status> {
                match code {
                    $(
                    $num => Some($name),
                    )+
                    _ => None
                }
            }
        }
    }
}

status_codes! {
    /// 200 OK status.
    (OK_200, 200, "OK");
    /// 400 Bad Request status.
    (BAD_REQUEST_400, 400, "BAD REQUEST");
    /// 404 Not Found status.
    (NOT_FOUND_404,  404, "NOT FOUND");
}

#[cfg(test)]
mod tests {
    use crate::common::status::{OK_200, Status};

    #[test]
    fn from_code_valid() {
        assert_eq!(Some(OK_200), Status::from_code(200))
    }

    #[test]
    fn from_code_invalid() {
        assert_eq!(None, Status::from_code(2))
    }
}