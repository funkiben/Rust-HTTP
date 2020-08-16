use std::io::BufRead;

use crate::deframe::error::DeframingError;

pub trait Deframe<T: Sized>: Sized {
    fn read(self, reader: &mut impl BufRead) -> Result<T, (Self, DeframingError)>;
}