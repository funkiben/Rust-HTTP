use std::fmt::Debug;
use std::io::{BufReader, ErrorKind, Read};

use crate::parse::error::ParsingError;
use crate::parse::parse::{Parse, ParseResult};
use crate::parse::parse::ParseStatus;
use crate::parse::test_util::TestParseResult::{IoErr, ParseErr, Value};
use crate::util::mock::{EndlessMockReader, MockReader};

#[derive(Debug)]
pub enum TestParseResult<T> {
    Value(T),
    IoErr(std::io::Error),
    ParseErr(ParsingError),
}

impl<T> From<ErrorKind> for TestParseResult<T> {
    fn from(kind: ErrorKind) -> Self {
        IoErr(std::io::Error::from(kind))
    }
}

impl<T> From<std::io::Error> for TestParseResult<T> {
    fn from(err: std::io::Error) -> Self {
        IoErr(err)
    }
}

impl<T> From<ParsingError> for TestParseResult<T> {
    fn from(err: ParsingError) -> Self {
        ParseErr(err)
    }
}

pub fn test_blocking<T: Debug + Eq>(parser: impl Parse<T>, tests: Vec<(Vec<&[u8]>, TestParseResult<T>)>) {
    let mut reader = MockReader::from_bytes(vec![]);
    reader.return_would_block_when_empty = true;
    let mut reader = BufReader::new(reader);

    let mut parser = Some(parser);

    for (new_data, expected) in tests {
        assert!(parser.is_some(), "deframer consumed before test ({:?}, {:?})", new_data, expected);

        reader.get_mut().data.extend(new_data.into_iter().map(|v| v.to_vec()));

        let actual = parser.take().unwrap().parse(&mut reader);

        let (actual, new_parser) = to_parse_test_result(actual);

        parser = new_parser;

        assert_results_equal(actual, expected);
    }
}

pub fn test_with_eof<T: Eq + Debug>(parser: impl Parse<T>, data: Vec<&str>, expected: TestParseResult<T>) {
    let reader = MockReader::from_strs(data);
    test_ignore_new_parser(parser, reader, expected);
}

pub fn test_endless_strs<T: Debug + Eq>(parser: impl Parse<T>, data: Vec<&str>, endless_data: &str, expected: TestParseResult<T>) {
    let reader = EndlessMockReader::from_strs(data, endless_data);
    test_ignore_new_parser(parser, reader, expected);
}

pub fn test_endless_bytes<T: Debug + Eq>(parser: impl Parse<T>, data: Vec<&[u8]>, endless_data: &[u8], expected: TestParseResult<T>) {
    let reader = EndlessMockReader::from_bytes(data, endless_data);
    test_ignore_new_parser(parser, reader, expected);
}

fn test_ignore_new_parser<T: Debug + Eq>(parser: impl Parse<T>, reader: impl Read, expected: TestParseResult<T>) {
    let mut reader = BufReader::new(reader);
    let actual = parser.parse(&mut reader);
    let (actual, _) = to_parse_test_result(actual);
    assert_results_equal(actual, expected);
}

fn to_parse_test_result<T, R>(result: ParseResult<T, R>) -> (TestParseResult<T>, Option<R>) {
    match result {
        Err(err) => (ParseErr(err), None),
        Ok(ParseStatus::Done(value)) => (Value(value), None),
        Ok(ParseStatus::IoErr(new_parser, err)) => (IoErr(err), Some(new_parser))
    }
}

fn assert_results_equal<T: Debug + Eq>(actual: TestParseResult<T>, expected: TestParseResult<T>) {
    match (expected, actual) {
        (Value(exp), Value(act)) => assert_eq!(exp, act),
        (exp, act) => assert_eq!(format!("{:?}", exp), format!("{:?}", act))
    }
}