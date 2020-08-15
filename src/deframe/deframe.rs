use std::io::BufRead;

use crate::deframe::error::DeframingError;

pub trait Deframe: Sized {
    type Output;

    fn read(self, reader: &mut impl BufRead) -> Result<Self::Output, (Self, DeframingError)>;
}