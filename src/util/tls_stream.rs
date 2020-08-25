use std::io::{Read, Result, Write};

use rustls::{Session, StreamOwned};

/// Wrapper for Rustls StreamOwner that implements Drop.
/// Will call send_close_notify when dropped to make sure the TLS connection ends properly.
pub struct TlsStream<S: Session, T: Read + Write>(StreamOwned<S, T>);

impl<S: Session, T: Read + Write> TlsStream<S, T> {
    /// Creates a new TLS stream with the given inner stream and session.
    /// The inner stream is used for reading TLS data in and writing TLS data out.
    /// When the TLS stream goes out of scope, it will send a close_notify message.
    pub fn new(session: S, inner: T) -> TlsStream<S, T> {
        TlsStream(StreamOwned::new(session, inner))
    }
}

impl<S: Session, T: Read + Write> Read for TlsStream<S, T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

impl<S: Session, T: Read + Write> Write for TlsStream<S, T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}

impl<S: Session, T: Read + Write> Drop for TlsStream<S, T> {
    fn drop(&mut self) {
        self.0.sess.send_close_notify();
        self.0.flush().unwrap_or_default();
    }
}