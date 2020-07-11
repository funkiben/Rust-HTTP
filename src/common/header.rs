use std::collections::HashMap;

use crate::common::header::Header::{Custom, Normal};

/// Connection header.
pub const CONNECTION: Header = Normal("Connection");
/// Content-Length header.
pub const CONTENT_LENGTH: Header = Normal("Content-Length");
/// Content-Type header.
pub const CONTENT_TYPE: Header = Normal("Content-Type");

/// A header. Is either a predefined "Normal" header with a static string, or a "Custom" header with a uniquely allocated String.
/// The "Normal" variant is to reuse memory for frequently seen headers.
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum Header {
    Normal(&'static str),
    Custom(String),
}

impl Header {
    /// Gets the given header as a string slice.
    pub fn as_str(&self) -> &str {
        match self {
            Normal(s) => s,
            Custom(s) => &s
        }
    }
}

/// Operations for a header map.
pub trait HeaderMapOps {
    /// Gets a header map from the given vector of header value and key pairs.
    fn from(header_values: Vec<(Header, String)>) -> Self;
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
    fn from(header_values: Vec<(Header, String)>) -> HeaderMap {
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

    use crate::common::header::{CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, HeaderMapOps, HeaderMap};

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
    fn header_map_from() {
        let headers: HeaderMap = HeaderMapOps::from(vec![
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
}