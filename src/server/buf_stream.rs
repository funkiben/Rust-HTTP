use std::io::{BufRead, BufReader, Read, Result, Write};

use crate::server::nonblocking_buf_writer::NonBlockingBufWriter;

/// A buffered stream using a BufReader and a NonBlockingBufWriter.
pub struct BufStream<T>(BufReader<NonBlockingBufWriter<T>>);

impl<T: Read + Write> BufStream<T> {
    /// Creates a new buffered stream with the given capacity for its buffer.
    pub fn with_capacities(inner: T, read_buffer_capacity: usize, write_buffer_capacity: usize) -> BufStream<T> {
        BufStream(BufReader::with_capacity(read_buffer_capacity, NonBlockingBufWriter::with_capacity(write_buffer_capacity, inner)))
    }

    /// Gets a reference to the inner stream.
    pub fn inner_ref(&self) -> &T {
        self.0.get_ref().inner_ref()
    }
}

impl<T: Read + Write> Read for BufStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

impl<T: Read + Write> Write for BufStream<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.get_mut().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.get_mut().flush()
    }
}

impl<T: Read + Write> BufRead for BufStream<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        self.0.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.consume(amt)
    }
}

impl<T: Read + Write> Read for NonBlockingBufWriter<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner_mut().read(buf)
    }
}

#[cfg(test)]
mod test {
    use std::io::{BufRead, Write};

    use crate::server::buf_stream::BufStream;
    use crate::util::mock::{MockReader, MockStream, MockWriter};

    #[test]
    fn test_buf_read_and_write() -> std::io::Result<()> {
        let reader = MockReader::from_strs(vec!["hello", "\nworld", "!"]);
        let writer = MockWriter::new();
        let mut stream = BufStream::with_capacities(MockStream::new(reader, writer), 1024, 1024);

        let mut buf = String::new();
        stream.read_line(&mut buf)?;
        assert_eq!("hello\n", buf);

        buf.clear();
        stream.read_line(&mut buf)?;
        assert_eq!("world!", buf);

        buf.clear();
        stream.read_line(&mut buf)?;
        assert_eq!("", buf);

        stream.write(b"hello")?;
        stream.write(b" ")?;
        stream.write(b"goodbye")?;

        assert!(stream.inner_ref().writer.written.is_empty());

        stream.flush()?;

        assert_eq!(stream.inner_ref().writer.flushed, vec![b"hello goodbye".to_vec()]);

        Ok(())
    }
}