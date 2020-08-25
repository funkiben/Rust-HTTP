use std::io::BufRead;

use crate::parse::deframe::deframe::Deframe;
use crate::parse::error::ParsingError;
use crate::parse::parse::ParseStatus::{Done, IoErr};

/// The result of a parse call. Contains either an error, the new parser state, or the fully parsed value.
pub type ParseResult<T, R> = Result<ParseStatus<T, R>, ParsingError>;

/// Trait for parsing statefully.
pub trait Parse<T>: Sized {
    /// Reads data from the reader until either a value can be parsed, an IO error is encountered, or a parsing error is encountered.
    /// The parser is consumed if either a parsing error occurs or a value is successfully parsed.
    /// Otherwise the parser is returned back along with the IO error that stopped it from parsing.
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<T, Self>;
}

/// The status of a parser.
pub enum ParseStatus<T, R> {
    /// The parser has fully constructed a value.
    Done(T),
    /// The new state of the parser and the IO error that was encountered.
    IoErr(R, std::io::Error),
}

impl<T, R> ParseStatus<T, R> {
    pub fn map_blocked<V>(self, mapper: impl Fn(R) -> V) -> ParseStatus<T, V> {
        match self {
            Done(val) => Done(val),
            IoErr(new, err) => IoErr(mapper(new), err)
        }
    }
}

impl<D: Deframe<T>, T> Parse<T> for D {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<T, Self> {
        match self.read(reader) {
            Err((reader, err)) => Ok(IoErr(reader, err)),
            Ok(value) => Ok(Done(value))
        }
    }
}