use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::common::header::Header::{Custom, Standard};

/// A header. Is either a "Standard" header with a static string, or a "Custom" header with a uniquely allocated String.
/// The "Standard" variant is to reuse memory for frequently seen headers.
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum Header {
    Standard(&'static str),
    Custom(String),
}

impl Display for Header {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Standard(s) => f.write_str(s),
            Custom(s) => f.write_str(s)
        }
    }
}

macro_rules! standard_headers {
    (
        $(
            $(#[$docs:meta])*
            ($name:ident, $value:expr);
        )+
    ) => {
        $(
            $(#[$docs])*
            pub const $name: Header = Header::Standard($value);
        )+

        impl From<&str> for Header {
            /// Gets a header from the given string representing the header name.
            fn from(value: &str) -> Header {
                let value = value.to_lowercase();
                match value.as_str() {
                    $(
                    $value => $name,
                    )+
                    _ => Header::Custom(value)
                }
            }
        }

        impl From<String> for Header {
            /// Gets a header from the given string representing the header name.
            fn from(mut value: String) -> Header {
                value.to_lowercase();
                match value.as_str() {
                    $(
                    $value => $name,
                    )+
                    _ => Header::Custom(value)
                }
            }
        }
    }
}

standard_headers! {
    (ACCEPT, "accept");
    (ACCEPT_CHARSET, "accept-charset");
    (ACCEPT_ENCODING, "accept-encoding");
    (ACCEPT_LANGUAGE, "accept-language");
    (ACCEPT_RANGES, "accept-ranges");
    (ACCESS_CONTROL_ALLOW_CREDENTIALS, "access-control-allow-credentials");
    (ACCESS_CONTROL_ALLOW_HEADERS, "access-control-allow-headers");
    (ACCESS_CONTROL_ALLOW_METHODS, "access-control-allow-methods");
    (ACCESS_CONTROL_ALLOW_ORIGIN, "access-control-allow-origin");
    (ACCESS_CONTROL_EXPOSE_HEADERS, "access-control-expose-headers");
    (ACCESS_CONTROL_MAX_AGE, "access-control-max-age");
    (ACCESS_CONTROL_REQUEST_HEADERS, "access-control-request-headers");
    (ACCESS_CONTROL_REQUEST_METHOD, "access-control-request-method");
    (AGE, "age");
    (ALLOW, "allow");
    (ALT_SVC, "alt-svc");
    (AUTHORIZATION, "authorization");
    (CACHE_CONTROL, "cache-control");
    (CONNECTION, "connection");
    (CONTENT_DISPOSITION, "content-disposition");
    (CONTENT_ENCODING, "content-encoding");
    (CONTENT_LANGUAGE, "content-language");
    (CONTENT_LENGTH, "content-length");
    (CONTENT_LOCATION, "content-location");
    (CONTENT_RANGE, "content-range");
    (CONTENT_SECURITY_POLICY, "content-security-policy");
    (CONTENT_SECURITY_POLICY_REPORT_ONLY, "content-security-policy-report-only");
    (CONTENT_TYPE, "content-type");
    (COOKIE, "cookie");
    (DNT, "dnt");
    (DATE, "date");
    (ETAG, "etag");
    (EXPECT, "expect");
    (EXPIRES, "expires");
    (FORWARDED, "forwarded");
    (FROM, "from");
    (HOST, "host");
    (IF_MATCH, "if-match");
    (IF_MODIFIED_SINCE, "if-modified-since");
    (IF_NONE_MATCH, "if-none-match");
    (IF_RANGE, "if-range");
    (IF_UNMODIFIED_SINCE, "if-unmodified-since");
    (LAST_MODIFIED, "last-modified");
    (LINK, "link");
    (LOCATION, "location");
    (MAX_FORWARDS, "max-forwards");
    (ORIGIN, "origin");
    (PRAGMA, "pragma");
    (PROXY_AUTHENTICATE, "proxy-authenticate");
    (PROXY_AUTHORIZATION, "proxy-authorization");
    (PUBLIC_KEY_PINS, "public-key-pins");
    (PUBLIC_KEY_PINS_REPORT_ONLY, "public-key-pins-report-only");
    (RANGE, "range");
    (REFERER, "referer");
    (REFERRER_POLICY, "referrer-policy");
    (REFRESH, "refresh");
    (RETRY_AFTER, "retry-after");
    (SEC_WEBSOCKET_ACCEPT, "sec-websocket-accept");
    (SEC_WEBSOCKET_EXTENSIONS, "sec-websocket-extensions");
    (SEC_WEBSOCKET_KEY, "sec-websocket-key");
    (SEC_WEBSOCKET_PROTOCOL, "sec-websocket-protocol");
    (SEC_WEBSOCKET_VERSION, "sec-websocket-version");
    (SERVER, "server");
    (SET_COOKIE, "set-cookie");
    (STRICT_TRANSPORT_SECURITY, "strict-transport-security");
    (TE, "te");
    (TRAILER, "trailer");
    (TRANSFER_ENCODING, "transfer-encoding");
    (USER_AGENT, "user-agent");
    (UPGRADE, "upgrade");
    (UPGRADE_INSECURE_REQUESTS, "upgrade-insecure-requests");
    (VARY, "vary");
    (VIA, "via");
    (WARNING, "warning");
    (WWW_AUTHENTICATE, "www-authenticate");
    (X_CONTENT_TYPE_OPTIONS, "x-content-type-options");
    (X_DNS_PREFETCH_CONTROL, "x-dns-prefetch-control");
    (X_FRAME_OPTIONS, "x-frame-options");
    (X_XSS_PROTECTION, "x-xss-protection");
}

/// Creates a map of headers.
/// ```
/// use my_http::common::header::{CONNECTION, CONTENT_TYPE, CONTENT_LENGTH, Header, TRANSFER_ENCODING, HeaderMapOps};
/// use my_http::header_map;
///
/// let headers = header_map![
///    (CONNECTION, "keep-alive"),
///    (CONTENT_LENGTH, "5"),
///    ("custom-header", "hello"),
///    ("coNtEnt-TyPE", "something"),
///    ("Transfer-encoding", "chunked")
/// ];
///
/// assert!(headers.contains_header_value(&CONNECTION, "keep-alive"));
/// assert!(headers.contains_header_value(&CONTENT_LENGTH, "5"));
/// assert!(headers.contains_header_value(&CONTENT_TYPE, "something"));
/// assert!(headers.contains_header_value(&Header::Custom("custom-header".into()), "hello"));
/// assert!(headers.contains_header_value(&TRANSFER_ENCODING, "chunked"));
/// ```
#[macro_export]
macro_rules! header_map {
    () => { $crate::common::header::HeaderMap::new() };
    ($(($header:expr, $value:expr)),+ $(,)?) => {
        <$crate::common::header::HeaderMap as $crate::common::header::HeaderMapOps>::from_pairs(vec![
            $(($header.into(), $value.into()),)+
        ])
    }
}

/// Operations for a header map.
pub trait HeaderMapOps {
    /// Gets a header map from the given vector of header value and key pairs.
    fn from_pairs(header_values: Vec<(Header, String)>) -> Self;
    /// Adds a header to the map.
    fn add_header(&mut self, k: Header, v: String);
    /// Checks if the map contains the given header and corresponding header value.
    fn contains_header_value(&self, k: &Header, v: &str) -> bool;
    /// Gets the first value for the given header.
    fn get_first_header_value(&self, k: &Header) -> Option<&String>;
}

/// A multimap of headers to values.
pub type HeaderMap = HashMap<Header, Vec<String>>;

impl HeaderMapOps for HeaderMap {
    fn from_pairs(header_values: Vec<(Header, String)>) -> HeaderMap {
        header_values.into_iter().fold(HashMap::new(), |mut m, (header, value)| {
            m.add_header(header, value);
            m
        })
    }

    fn add_header(&mut self, k: Header, v: String) {
        self.entry(k).or_insert(Vec::new()).push(v)
    }

    fn contains_header_value(&self, k: &Header, v: &str) -> bool {
        if let Some(values) = self.get(k) {
            return values.contains(&String::from(v));
        }
        false
    }

    fn get_first_header_value(&self, k: &Header) -> Option<&String> {
        self.get(k)?.get(0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::header::{CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, Header, HeaderMap, HeaderMapOps, TRANSFER_ENCODING};

    #[test]
    fn header_map() {
        let mut headers = HashMap::new();
        headers.add_header(CONNECTION, String::from("value 1"));
        headers.add_header(CONNECTION, String::from("value 2"));
        headers.add_header(CONNECTION, String::from("value 3"));
        headers.add_header(CONTENT_LENGTH, String::from("5"));
        headers.add_header(CONTENT_TYPE, String::from("something"));

        assert!(headers.contains_header_value(&CONNECTION, "value 1"));
        assert!(headers.contains_header_value(&CONNECTION, "value 2"));
        assert!(headers.contains_header_value(&CONNECTION, "value 3"));
        assert!(headers.contains_header_value(&CONTENT_LENGTH, "5"));
        assert!(headers.contains_header_value(&CONTENT_TYPE, "something"));

        assert_eq!(headers.get_first_header_value(&CONNECTION).unwrap(), "value 1");
        assert_eq!(headers.get_first_header_value(&CONTENT_LENGTH).unwrap(), "5");
        assert_eq!(headers.get_first_header_value(&CONTENT_TYPE).unwrap(), "something");
    }

    #[test]
    fn header_map_from_pairs() {
        let headers: HeaderMap = HeaderMap::from_pairs(vec![
            (CONNECTION, String::from("value 1")),
            (CONTENT_LENGTH, String::from("5")),
            (CONNECTION, String::from("value 2")),
            (CONTENT_TYPE, String::from("something")),
            (CONNECTION, String::from("value 3")),
        ]);

        assert!(headers.contains_header_value(&CONNECTION, "value 1"));
        assert!(headers.contains_header_value(&CONNECTION, "value 2"));
        assert!(headers.contains_header_value(&CONNECTION, "value 3"));
        assert!(headers.contains_header_value(&CONTENT_LENGTH, "5"));
        assert!(headers.contains_header_value(&CONTENT_TYPE, "something"));

        assert_eq!(headers.get_first_header_value(&CONNECTION).unwrap(), "value 1");
        assert_eq!(headers.get_first_header_value(&CONTENT_LENGTH).unwrap(), "5");
        assert_eq!(headers.get_first_header_value(&CONTENT_TYPE).unwrap(), "something");
    }

    #[test]
    fn header_map_macro_empty_header_map() {
        assert!(header_map![].is_empty());
    }

    #[test]
    fn header_map_macro_predefined_header_from_str() {
        assert_eq!(CONNECTION, Header::from("ConnEctiOn"));
    }

    #[test]
    fn header_map_macro_custom_header_from_str() {
        assert_eq!(Header::Custom("custom-header".to_string()), Header::from("Custom-Header"));
    }

    #[test]
    fn header_map_macro() {
        let headers = header_map![
            (CONNECTION, "value 1"),
            (CONTENT_LENGTH, "5"),
            (CONNECTION, "value 2"),
            (CONTENT_TYPE, "something"),
            (CONNECTION, "value 3"),
            ("custom-header", "hello"),
            ("coNneCtion", "value 4"),
            ("transfer-encoding", "chunked")
        ];

        assert!(headers.contains_header_value(&CONNECTION, "value 1"));
        assert!(headers.contains_header_value(&CONNECTION, "value 2"));
        assert!(headers.contains_header_value(&CONNECTION, "value 3"));
        assert!(headers.contains_header_value(&CONNECTION, "value 4"));
        assert!(headers.contains_header_value(&CONTENT_LENGTH, "5"));
        assert!(headers.contains_header_value(&CONTENT_TYPE, "something"));
        assert!(headers.contains_header_value(&Header::Custom("custom-header".into()), "hello"));
        assert!(headers.contains_header_value(&"transfer-encoding".into(), "chunked"));

        assert_eq!(headers.get_first_header_value(&CONNECTION).unwrap(), "value 1");
        assert_eq!(headers.get_first_header_value(&CONTENT_LENGTH).unwrap(), "5");
        assert_eq!(headers.get_first_header_value(&CONTENT_TYPE).unwrap(), "something");
        assert_eq!(headers.get_first_header_value(&TRANSFER_ENCODING).unwrap(), "chunked");
    }
}