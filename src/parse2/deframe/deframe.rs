use std::io::{BufRead, Error};

pub type DeframerResult<T, R> = Result<T, (R, Error)>;

pub trait Deframe<T>: Sized {
    /// Reads data from the reader until a value can be constructed.
    /// If an IO error if encountered while reading, then the state of the deframer as well as the error are returned.
    fn read(self, reader: &mut impl BufRead) -> DeframerResult<T, Self>;

    fn data_so_far(&self) -> &T;
}