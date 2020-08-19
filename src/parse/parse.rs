use std::io::{BufRead, Error, ErrorKind};

use crate::parse::deframe::deframe::Deframe;
use crate::parse::error::ParsingError;
use crate::parse::parse::ParseStatus::{Blocked, Done};

/// The result of a parse call. Contains either an error, the new parser state, or the fully parsed value.
pub type ParseResult<T, R> = Result<ParseStatus<T, R>, ParsingError>;

/// Trait for parsing statefully. Reads as much data from the given reader as possible, and either
/// returns an error or the status of the parser. The status of the parser will either be the updated
/// parser state if a value can't be construct yet, or the fully parsed value.
pub trait Parse<T>: Sized {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<T, Self>;
}

/// The status of a parser.
pub enum ParseStatus<T, R> {
    /// The parser has fully constructed a value.
    Done(T),
    /// The new state of the parser.
    Blocked(R),
}

impl<T, R> ParseStatus<T, R> {
    pub fn map_blocked<V>(self, mapper: impl Fn(R) -> V) -> ParseStatus<T, V> {
        match self {
            Done(val) => Done(val),
            Blocked(new) => Blocked(mapper(new))
        }
    }
}

impl<D: Deframe<T>, T> Parse<T> for D {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<T, Self> {
        match self.read(reader) {
            Err((reader, err)) if can_continue_after_error(&err) => Ok(Blocked(reader)),
            Err((_, err)) => Err(err.into()),
            Ok(value) => Ok(Done(value))
        }
    }
}

fn can_continue_after_error(err: &Error) -> bool {
    err.kind() == ErrorKind::WouldBlock
}