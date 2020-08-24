use std::io::{ErrorKind, Result, Write};

/// The default capacity of the buffer.
const DEFAULT_CAPACITY: usize = 8 * 1024;

/// A buffered writer that handles WouldBlock errors.
/// WouldBlock errors simply stop execution of either a flush or a write, and remaining unwritten
/// data is stored in a buffer.
pub struct NonBlockingBufWriter<T> {
    buf: Vec<u8>,
    pos: usize,
    inner: T,
    inner_needs_flush: bool,
}

impl<T: Write> NonBlockingBufWriter<T> {
    /// Creates a new writer with a buffer that has the given capacity.
    pub fn with_capacity(capacity: usize, inner: T) -> NonBlockingBufWriter<T> {
        NonBlockingBufWriter { pos: 0, buf: Vec::with_capacity(capacity), inner, inner_needs_flush: false }
    }

    /// Creates a new writer with a buffer of default capacity.
    pub fn new(inner: T) -> NonBlockingBufWriter<T> {
        Self::with_capacity(DEFAULT_CAPACITY, inner)
    }

    /// Checks if the given writer has unflushed data. flush() should be called when the underlying
    /// writer is writeable until this returns false.
    pub fn needs_flush(&self) -> bool {
        self.pos > 0 || self.inner_needs_flush
    }

    /// Gets a reference to the underlying writer.
    pub fn inner_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying writer.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Writes the contents of the buffer to the underlying writer.
    /// May only partially flush if the underlying writer blocks.
    fn flush_buf(&mut self) -> Result<()> {
        let amount = write_until_blocked(&mut self.inner, &self.buf[self.pos..])?;

        self.pos += amount;

        if self.pos == self.buf.len() {
            self.pos = 0;
            self.buf.clear();
        }

        Ok(())
    }

    /// Flushes the underlying writer. If the underlying buffer blocks when flushed, then
    /// inner_needs_flush is set to true.
    fn flush_inner(&mut self) -> Result<()> {
        match self.inner.flush() {
            Ok(()) => {
                self.inner_needs_flush = false;
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                self.inner_needs_flush = true;
                Ok(())
            }
            Err(error) => Err(error)
        }
    }
}

impl<T: Write> Write for NonBlockingBufWriter<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.pos + buf.len() > self.buf.capacity() {
            self.flush()?;
            if self.needs_flush() {
                self.buf.write(buf)
            } else {
                let amount = write_until_blocked(&mut self.inner, buf)?;
                self.buf.write(&buf[amount..])
            }
        } else {
            self.buf.write(buf)
        }
    }

    fn flush(&mut self) -> Result<()> {
        self.flush_buf()?;
        self.flush_inner()
    }
}

/// Writes the given data to the given writer until completion or until the writer blocks.
fn write_until_blocked<W: Write>(writer: &mut W, buf: &[u8]) -> Result<usize> {
    let mut pos = 0;
    while pos != buf.len() {
        match writer.write(&buf[pos..]) {
            Ok(amount) => pos += amount,
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(pos),
            Err(error) => return Err(error)
        }
    }
    Ok(pos)
}