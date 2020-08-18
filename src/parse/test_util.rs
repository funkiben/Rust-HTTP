use std::fmt::Debug;
use std::io::{BufReader, Read};

use crate::parse::error::ParsingError;
use crate::parse::parse::{Parse, ParseResult};
use crate::parse::parse::ParseStatus::{Blocked, Done};
use crate::util::mock::{EndlessMockReader, MockReader};

pub fn test_blocking<T: Debug + Eq>(parser: impl Parse<T>, tests: Vec<(Vec<&[u8]>, Result<Option<T>, ParsingError>)>) {
    let mut reader = MockReader::from_bytes(vec![]);
    reader.return_would_block_when_empty = true;
    let mut reader = BufReader::new(reader);

    let mut parser = Some(parser);

    for (new_data, expected) in tests {
        assert!(parser.is_some(), "deframer consumed before test ({:?}, {:?})", new_data, expected);

        reader.get_mut().data.extend(new_data.into_iter().map(|v| v.to_vec()));

        let actual = parser.take().unwrap().parse(&mut reader);

        let (actual, new_parser) = map_result(actual);

        parser = new_parser;

        assert_results_equal(actual, expected);
    }
}

pub fn test_with_eof<T: Eq + Debug>(parser: impl Parse<T>, data: Vec<&str>, expected: Result<T, ParsingError>) {
    let reader = MockReader::from_strs(data);
    test_ignore_new_parser(parser, reader, expected);
}

pub fn test_endless_strs<T: Debug + Eq>(parser: impl Parse<T>, data: Vec<&str>, endless_data: &str, expected: Result<T, ParsingError>) {
    let reader = EndlessMockReader::from_strs(data, endless_data);
    test_ignore_new_parser(parser, reader, expected);
}

pub fn test_endless_bytes<T: Debug + Eq>(parser: impl Parse<T>, data: Vec<&[u8]>, endless_data: &[u8], expected: Result<T, ParsingError>) {
    let reader = EndlessMockReader::from_bytes(data, endless_data);
    test_ignore_new_parser(parser, reader, expected);
}

fn test_ignore_new_parser<T: Debug + Eq>(parser: impl Parse<T>, reader: impl Read, expected: Result<T, ParsingError>) {
    let expected = expected.map(|e| Some(e));
    let mut reader = BufReader::new(reader);
    let actual = parser.parse(&mut reader);
    let (actual, _) = map_result(actual);
    assert_results_equal(actual, expected);
}

fn map_result<T, R>(result: ParseResult<T, R>) -> (Result<Option<T>, ParsingError>, Option<R>) {
    match result {
        Err(err) => (Err(err), None),
        Ok(Done(value)) => (Ok(Some(value)), None),
        Ok(Blocked(new_parser)) => (Ok(None), Some(new_parser))
    }
}

fn assert_results_equal<T: Debug + Eq>(actual: Result<Option<T>, ParsingError>, expected: Result<Option<T>, ParsingError>) {
    match (expected, actual) {
        (Ok(Some(exp)), Ok(Some(act))) => assert_eq!(exp, act),
        (exp, act) => assert_eq!(format!("{:?}", exp), format!("{:?}", act))
    }
}