use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::common::header::Header::{Custom, Predefined};

/// A header. Is either a predefined "Normal" header with a static string, or a "Custom" header with a uniquely allocated String.
/// The "Normal" variant is to reuse memory for frequently seen headers.
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum Header {
    Predefined(&'static str),
    Custom(String),
}

impl Display for Header {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Predefined(s) => f.write_str(s),
            Custom(s) => f.write_str(s)
        }
    }
}

macro_rules! headers {
    (
        $(
            $(#[$docs:meta])*
            ($name:ident, $value:expr);
        )+
    ) => {
        $(
            $(#[$docs])*
            pub const $name: Header = Header::Predefined($value);
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
    }
}

headers! {
    /// Connection header.
    (CONNECTION, "connection");
    /// Content-Length header.
    (CONTENT_LENGTH, "content-length");
    /// Content-Type header.
    (CONTENT_TYPE, "content-type");
    /// Transfer-Encoding header.
    (TRANSFER_ENCODING, "transfer-encoding");
    /// Host header.
    (HOST, "host");
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
    fn predefined_header_from_str() {
        assert_eq!(CONNECTION, Header::from("ConnEctiOn"));
    }

    #[test]
    fn custom_header_from_str() {
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