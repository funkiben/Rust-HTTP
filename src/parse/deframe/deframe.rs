use std::io::{BufRead, Error};

/// Result of a deframer.
pub type DeframerResult<T, R> = Result<T, (R, Error)>;

/// Trait for stateful IO reading.
pub trait Deframe<T>: Sized {
    /// Reads data from the reader until a value can be constructed.
    /// If an IO error if encountered while reading, then the state of the deframer as well as the error are returned.
    fn read(self, reader: &mut impl BufRead) -> DeframerResult<T, Self>;

    /// Returns how many bytes have been read so far by this deframer.
    fn read_so_far(&self) -> usize;
}