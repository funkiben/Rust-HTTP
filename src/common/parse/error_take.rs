use std::task::Context;

use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, Error, ErrorKind, Result, Take};
use tokio::macros::support::{Pin, Poll};

/// Provides a method like take, but instead throws an error when the limit is reached.
pub trait ErrorTake {
    /// Like take, but will return an error as soon as the read limit if reached.
    fn error_take(self, limit: u64) -> CustomTake<Self> where Self: Sized;
}

impl<T: AsyncReadExt> ErrorTake for T {
    fn error_take(self, limit: u64) -> CustomTake<T> {
        CustomTake::new(self.take(limit))
    }
}

/// Like Take, but will return an error when the limit is reached.
/// The standard Take returns Ok(0) when the limit is reached.
pub struct CustomTake<T>(Take<T>);

impl<'a, T: AsyncRead> CustomTake<T> {
    /// Creates a new custom take using an inner take.
    fn new(inner: Take<T>) -> CustomTake<T> {
        CustomTake(inner)
    }

    /// Checks if the take limit has been reached. If so, returns an error.
    fn check_limit(&self) -> std::io::Result<()> {
        match self.0.limit() {
            0 => Err(Error::new(ErrorKind::Other, "read limit reached")),
            _ => Ok(())
        }
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for CustomTake<T> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>> {
        if let Err(err) = self.check_limit() {
            Poll::Ready(Err(err))
        } else {
            Pin::new(&mut self.0).poll_read(cx, buf)
        }
    }
}

impl<T: AsyncBufRead + Unpin> AsyncBufRead for CustomTake<T> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        if let Err(err) = self.check_limit() {
            Poll::Ready(Err(err))
        } else {
            let this = self.get_mut();
            Pin::new(&mut this.0).poll_fill_buf(cx)
        }
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut self.0).consume(amt)
    }
}
