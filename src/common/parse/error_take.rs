use std::io::{Read, ErrorKind, BufRead};

/// Provides a method like take, but instead throws an error when the limit is reached.
pub trait ErrorTake<T> {
    /// Like take, but will return an error as soon as the read limit if reached.
    fn error_take(self, limit: u64) -> CustomTake<T>;
}

impl<T: Read> ErrorTake<T> for T {
    fn error_take(self, limit: u64) -> CustomTake<T> {
        CustomTake::new(self.take(limit))
    }
}

/// Like Take, but will return an error when the limit is reached.
/// The standard Take returns Ok(0) when the limit is reached.
pub struct CustomTake<T>(std::io::Take<T>);

impl<T> CustomTake<T> {
    /// Creates a new custom take using an inner take.
    fn new(inner: std::io::Take<T>) -> CustomTake<T> {
        CustomTake(inner)
    }

    /// Checks if the take limit has been reached. If so, returns an error.
    fn check_limit(&self) -> std::io::Result<()> {
        match self.0.limit() {
            0 => Err(std::io::Error::new(ErrorKind::Other, "read limit reached")),
            _ => Ok(())
        }
    }
}

impl<T: Read> Read for CustomTake<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.check_limit()?;
        self.0.read(buf)
    }
}

impl<T: BufRead> BufRead for CustomTake<T> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.check_limit()?;
        self.0.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.consume(amt);
    }
}
