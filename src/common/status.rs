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
    (OK, 200, "OK");
    (CREATED, 201, "CREATED");
    (ACCEPTED, 202, "ACCEPTED");
    (NONAUTHORITATIVE_INFORMATION, 203, "NON-AUTHORITATIVE INFORMATION");
    (NO_CONTENT, 204, "NO CONTENT");
    (RESET_CONTENT, 205, "RESET CONTENT");
    (PARTIAL_CONTENT, 206, "PARTIAL CONTENT");
    (MULTISTATUS, 207, "MULTI-STATUS");
    (ALREADY_REPORTED, 208, "ALREADY REPORTED");
    (IM_USED, 226, "IM USED");
    (MULTIPLE_CHOICES, 300, "MULTIPLE CHOICES");
    (MOVED_PERMANENTLY, 301, "MOVED PERMANENTLY");
    (FOUND, 302, "FOUND");
    (SEE_OTHER, 303, "SEE OTHER");
    (NOT_MODIFIED, 304, "NOT MODIFIED");
    (USE_PROXY, 305, "USE PROXY");
    (TEMPORARY_REDIRECT, 307, "TEMPORARY REDIRECT");
    (PERMANENT_REDIRECT, 308, "PERMANENT REDIRECT");
    (BAD_REQUEST, 400, "BAD REQUEST");
    (UNAUTHORIZED, 401, "UNAUTHORIZED");
    (PAYMENT_REQUIRED, 402, "PAYMENT REQUIRED");
    (FORBIDDEN, 403, "FORBIDDEN");
    (NOT_FOUND, 404,"NOT FOUND");
    (METHOD_NOT_ALLOWED, 405, "METHOD NOT ALLOWED");
    (NOT_ACCEPTABLE, 406, "NOT ACCEPTABLE");
    (PROXY_AUTHENTICATION_REQUIRED, 407, "PROXY AUTHENTICATION REQUIRED");
    (REQUEST_TIMEOUT, 408, "REQUEST TIMEOUT");
    (CONFLICT, 409, "CONFLICT");
    (GONE, 410, "GONE");
    (LENGTH_REQUIRED, 411, "LENGTH REQUIRED");
    (PRECONDITION_FAILED, 412, "PRECONDITION FAILED");
    (PAYLOAD_TOO_LARGE, 413, "PAYLOAD TOO LARGE");
    (URI_TOO_LONG, 414, "URI TOO LONG");
    (UNSUPPORTED_MEDIA_TYPE, 415, "UNSUPPORTED MEDIA TYPE");
    (REQUESTED_RANGE_NOT_SATISFIABLE, 416, "REQUESTED RANGE NOT SATISFIABLE");
    (EXPECTATION_FAILED, 417, "EXPECTATION FAILED");
    (IM_A_TEAPOT, 418, "I'M A TEAPOT");
    (MISDIRECTED_REQUEST, 421, "MISDIRECTED REQUEST");
    (UNPROCESSABLE_ENTITY, 422, "UNPROCESSABLE ENTITY");
    (LOCKED, 423, "LOCKED");
    (FAILED_DEPENDENCY, 424, "FAILED DEPENDENCY");
    (UPGRADE_REQUIRED, 426, "UPGRADE REQUIRED");
    (PRECONDITION_REQUIRED, 428, "PRECONDITION REQUIRED");
    (TOO_MANY_REQUESTS, 429, "TOO MANY REQUESTS");
    (REQUEST_HEADER_FIELDS_TOO_LARGE, 431, "REQUEST HEADER FIELDS TOO LARGE");
    (CONNECTION_CLOSED_WITHOUT_RESPONSE, 444, "CONNECTION CLOSED WITHOUT RESPONSE");
    (UNAVAILABLE_FOR_LEGAL_REASONS, 451, "UNAVAILABLE FOR LEGAL REASONS");
    (CLIENT_CLOSED_REQUEST, 499, "CLIENT CLOSED REQUEST");
    (INTERNAL_SERVER_ERROR, 500, "INTERNAL SERVER ERROR");
    (NOT_IMPLEMENTED, 501, "NOT IMPLEMENTED");
    (BAD_GATEWAY, 502, "BAD GATEWAY");
    (SERVICE_UNAVAILABLE, 503, "SERVICE UNAVAILABLE");
    (GATEWAY_TIMEOUT, 504, "GATEWAY TIMEOUT");
    (HTTP_VERSION_NOT_SUPPORTED, 505, "HTTP VERSION NOT SUPPORTED");
    (VARIANT_ALSO_NEGOTIATES, 506, "VARIANT ALSO NEGOTIATES");
    (INSUFFICIENT_STORAGE, 507, "INSUFFICIENT STORAGE");
    (LOOP_DETECTED, 508, "LOOP DETECTED");
    (NOT_EXTENDED, 510, "NOT EXTENDED");
    (NETWORK_AUTHENTICATION_REQUIRED, 511, "NETWORK AUTHENTICATION REQUIRED");
    (NETWORK_CONNECT_TIMEOUT_ERROR, 599, "NETWORK CONNECT TIMEOUT ERROR");
}

#[cfg(test)]
mod tests {
    use crate::common::status::{OK, Status};

    #[test]
    fn from_code_valid() {
        assert_eq!(Some(OK), Status::from_code(200))
    }

    #[test]
    fn from_code_invalid() {
        assert_eq!(None, Status::from_code(2))
    }
}