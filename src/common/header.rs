use std::collections::HashMap;

use crate::common::header::Header::{Custom, Normal};

pub const CONNECTION: Header = Normal("Connection");
pub const CONTENT_LENGTH: Header = Normal("Content-Length");
pub const CONTENT_TYPE: Header = Normal("Content-Type");

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum Header {
    Normal(&'static str),
    Custom(String),
}

impl Header {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Normal(s) => s,
            Custom(s) => &s
        }
    }
}

pub trait HeaderMapOps {
    fn from(header_values: Vec<(Header, String)>) -> Self;
    fn add_header(&mut self, k: Header, v: String);
    fn contains_header_value(&self, k: &Header, v: &str) -> bool;
    fn get_first_header_value(&self, k: &Header) -> Option<&String>;
}

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

    // fn eq(&self, other: &Self) -> bool {
    //
    // }
}
