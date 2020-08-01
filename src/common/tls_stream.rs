use std::cell::RefCell;
use std::io::{Read, Result, Write};

use rustls::{Session, StreamOwned};

/// A TlsStream with interior mutability to allow reading and writing on non-mutable references.
/// Currently uses Rustls's TLS stream implementation under the hood.
pub struct TlsStream<S: Session, T: Read + Write>(RefCell<StreamOwned<S, T>>);

impl<S: Session, T: Read + Write> TlsStream<S, T> {
    /// Creates a new TLS stream with the given inner stream and session.
    /// The inner stream is used for reading TLS data in and writing TLS data out.
    /// When the TLS stream goes out of scope, it will send a close_notify message.
    pub fn new(inner: T, session: S) -> TlsStream<S, T> {
        TlsStream(RefCell::new(StreamOwned::new(session, inner)))
    }
}

impl<S: Session, T: Read + Write> Read for TlsStream<S, T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.borrow_mut().read(buf)
    }
}

impl<S: Session, T: Read + Write> Write for TlsStream<S, T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.borrow_mut().flush()
    }
}

impl<S: Session, T: Read + Write> Read for &TlsStream<S, T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.borrow_mut().read(buf)
    }
}

impl<S: Session, T: Read + Write> Write for &TlsStream<S, T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.borrow_mut().flush()
    }
}

impl<S: Session, T: Read + Write> Drop for TlsStream<S, T> {
    fn drop(&mut self) {
        let mut stream = self.0.borrow_mut();
        stream.sess.send_close_notify();
        stream.flush().unwrap_or_default();
    }
}