use std::io::{BufRead, ErrorKind, Read};

/// Provides a method like take, but instead throws an error when the limit is reached.
pub trait ReadExt<T> {
    /// Like take, but will return an error as soon as the read limit if reached.
    fn error_take(self, limit: u64) -> ErrorTake<T>;
}

impl<T: Read> ReadExt<T> for T {
    fn error_take(self, limit: u64) -> ErrorTake<T> {
        ErrorTake::new(self.take(limit))
    }
}

/// Like Take, but will return an error when the limit is reached.
/// The standard Take returns Ok(0) when the limit is reached.
pub struct ErrorTake<T>(std::io::Take<T>);

impl<T> ErrorTake<T> {
    /// Creates a new custom take using an inner take.
    fn new(inner: std::io::Take<T>) -> ErrorTake<T> {
        ErrorTake(inner)
    }

    /// Checks if the take limit has been reached. If so, returns an error.
    fn check_limit(&self) -> std::io::Result<()> {
        match self.0.limit() {
            0 => Err(std::io::Error::new(ErrorKind::Other, "read limit reached")),
            _ => Ok(())
        }
    }
}

impl<T: Read> Read for ErrorTake<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.check_limit()?;
        self.0.read(buf)
    }
}

impl<T: BufRead> BufRead for ErrorTake<T> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.check_limit()?;
        self.0.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.consume(amt);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind, Read};

    use crate::parse::error_take::ReadExt;
    use crate::util::mock::EndlessMockReader;

    #[test]
    fn infinite_reader() {
        let reader = EndlessMockReader::from_strs(vec![], "blahblahblah");
        let mut reader = reader.error_take(100);

        let mut buf = vec![];

        let res = reader.read_to_end(&mut buf);

        assert!(res.is_err());
        assert_eq!(format!("{:?}", res.err().unwrap()), format!("{:?}", Error::new(ErrorKind::Other, "read limit reached")));

        assert!(buf.len() <= 100);
    }
}