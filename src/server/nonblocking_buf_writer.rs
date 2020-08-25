use std::io::{ErrorKind, Result, Write};

/// A buffered writer that handles WouldBlock errors.
/// WouldBlock errors simply stop execution of either a flush or a write, and remaining unwritten
/// data is stored in a buffer.
pub struct NonBlockingBufWriter<T> {
    buf: Vec<u8>,
    pos: usize,
    inner: T,
}

impl<T: Write> NonBlockingBufWriter<T> {
    /// Creates a new writer with a buffer that has the given capacity.
    pub fn with_capacity(capacity: usize, inner: T) -> NonBlockingBufWriter<T> {
        NonBlockingBufWriter { pos: 0, buf: Vec::with_capacity(capacity), inner }
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

        if self.pos >= self.buf.len() {
            self.pos = 0;
            self.buf.clear();
        }

        Ok(())
    }

    /// Flushes the underlying writer. If the underlying buffer blocks when flushed, then Ok is
    /// still returned.
    fn flush_inner(&mut self) -> Result<()> {
        match self.inner.flush() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(()),
            x => x
        }
    }
}

impl<T: Write> Write for NonBlockingBufWriter<T> {
    fn write(&mut self, mut buf: &[u8]) -> Result<usize> {
        let len = buf.len();
        // try to avoid allocating more memory for buffer by flushing and then writing large data directly into underlying writer.
        if self.buf.len() + buf.len() > self.buf.capacity() {
            self.flush_buf()?;
            if self.pos == 0 && buf.len() > self.buf.capacity() { // if we flushed the writer but theres still not enough room
                let amount = write_until_blocked(&mut self.inner, buf)?;
                buf = &buf[amount..];
            }
        }
        self.buf.write(buf)?;
        Ok(len)
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

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::ops::Deref;

    use crate::server::nonblocking_buf_writer::NonBlockingBufWriter;
    use crate::util::mock::MockWriter;

    #[test]
    fn writes_and_flushes_with_no_blocking() {
        let writer = MockWriter::new();
        let written = writer.written.clone();
        let flushed = writer.flushed.clone();

        let mut writer = NonBlockingBufWriter::with_capacity(16, writer);

        writer.write(b"hello 1").unwrap();
        writer.write(b"hello 2").unwrap();

        assert!(written.borrow().is_empty());

        writer.write(b"hello 3").unwrap();

        assert_eq!(written.borrow().deref(), &vec![b"hello 1hello 2".to_vec()]);

        writer.write(b"this is big and wont fit in the buffer").unwrap();

        assert_eq!(written.borrow().deref(), &vec![b"hello 1hello 2".to_vec(), b"hello 3".to_vec(), b"this is big and wont fit in the buffer".to_vec()]);

        writer.flush().unwrap();

        assert_eq!(flushed.borrow().deref(), &vec![b"hello 1hello 2".to_vec(), b"hello 3".to_vec(), b"this is big and wont fit in the buffer".to_vec()]);
    }
}