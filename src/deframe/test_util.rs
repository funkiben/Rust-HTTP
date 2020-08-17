use std::fmt::Debug;
use std::io::{BufReader, Read};

use crate::deframe::deframe::Deframe;
use crate::deframe::error::DeframingError;
use crate::util::mock::{EndlessMockReader, MockReader};

pub fn test_blocking<T: Debug + Eq>(deframer: impl Deframe<T>, tests: Vec<(Vec<&[u8]>, Result<T, DeframingError>)>) {
    let mut reader = MockReader::from_bytes(vec![]);
    reader.return_would_block_when_empty = true;
    let mut reader = BufReader::new(reader);

    let mut deframer = Some(deframer);

    for (new_data, expected) in tests {
        assert!(deframer.is_some(), "deframer consumed before test ({:?}, {:?})", new_data, expected);

        reader.get_mut().data.extend(new_data.into_iter().map(|v| v.to_vec()));

        let actual_result = deframer.take().unwrap().read(&mut reader);

        let (new_deframer, actual) = match actual_result {
            Err((new, err)) => (Some(new), Err(err)),
            Ok(v) => (None, Ok(v))
        };

        deframer = new_deframer;

        assert_results_equal(expected, actual);
    }
}

pub fn test_with_eof<T: Eq + Debug>(deframer: impl Deframe<T>, data: Vec<&str>, expected: Result<T, DeframingError>) {
    let reader = MockReader::from_strs(data);
    test_ignore_new_deframer(deframer, reader, expected);
}

pub fn test_endless_strs<T: Debug + Eq>(deframer: impl Deframe<T>, data: Vec<&str>, endless_data: &str, expected: Result<T, DeframingError>) {
    let reader = EndlessMockReader::from_strs(data, endless_data);
    test_ignore_new_deframer(deframer, reader, expected);
}

pub fn test_endless_bytes<T: Debug + Eq>(deframer: impl Deframe<T>, data: Vec<&[u8]>, endless_data: &[u8], expected: Result<T, DeframingError>) {
    let reader = EndlessMockReader::from_bytes(data, endless_data);
    test_ignore_new_deframer(deframer, reader, expected);
}

fn test_ignore_new_deframer<T: Debug + Eq>(deframer: impl Deframe<T>, reader: impl Read, expected: Result<T, DeframingError>) {
    let mut reader = BufReader::new(reader);
    let actual = deframer.read(&mut reader);
    let actual = actual.map_err(|(_, err)| err);
    assert_results_equal(expected, actual);
}

fn assert_results_equal<T: Debug + Eq>(actual: Result<T, DeframingError>, expected: Result<T, DeframingError>) {
    match (expected, actual) {
        (Ok(exp), Ok(act)) => assert_eq!(exp, act),
        (exp, act) => assert_eq!(format!("{:?}", exp), format!("{:?}", act))
    }
}