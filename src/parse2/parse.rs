use std::io::{BufRead, Error, ErrorKind};

use crate::parse2::deframe::deframe::Deframe;
use crate::parse2::error::ParsingError;
use crate::parse2::parse::ParseStatus::{Blocked, Done};

pub type ParseResult<T, R> = Result<ParseStatus<T, R>, ParsingError>;

pub trait Parse<T>: Sized {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<T, Self>;
}

pub enum ParseStatus<T, R> {
    Done(T),
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