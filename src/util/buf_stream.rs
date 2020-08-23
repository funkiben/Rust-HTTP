use std::io::{BufRead, BufReader, BufWriter, Read, Result, Write};

/// A buffered stream. Stores buffers for writing and reading.
/// Uses BufWriter and BufReader for buffer implementations.
pub struct BufStream<T: Write>(BufWriter<WriteableBufReader<T>>);

impl<T: Read + Write> BufStream<T> {
    /// Creates a new buffered stream with the given capacities for its buffers.
    pub fn with_capacities(inner: T, read_buffer_capacity: usize, write_buffer_capacity: usize) -> BufStream<T> {
        BufStream(BufWriter::with_capacity(write_buffer_capacity, WriteableBufReader(BufReader::with_capacity(read_buffer_capacity, inner))))
    }

    /// Creates a new buffered stream with the default buffer sizes.
    pub fn new(inner: T) -> BufStream<T> {
        BufStream(BufWriter::new(WriteableBufReader(BufReader::new(inner))))
    }

    /// Gets a reference to the inner stream.
    pub fn inner_ref(&self) -> &T {
        self.0.get_ref().0.get_ref()
    }
}

struct WriteableBufReader<T>(BufReader<T>);

impl<T: Write> Write for WriteableBufReader<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.get_mut().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.get_mut().flush()
    }
}

impl<T: Read + Write> Read for BufStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.get_mut().0.read(buf)
    }
}

impl<T: Read + Write> Write for BufStream<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}

impl<T: Read + Write> BufRead for BufStream<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        self.0.get_mut().0.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.get_mut().0.consume(amt)
    }
}

#[cfg(test)]
mod test {
    use std::io::{Write, BufRead};

    use crate::util::buf_stream::BufStream;
    use crate::util::mock::{MockReader, MockStream, MockWriter};

    #[test]
    fn test_buf_read_and_write() -> std::io::Result<()> {
        let reader = MockReader::from_strs(vec!["hello", "\nworld", "!"]);
        let writer = MockWriter::new();
        let mut stream = BufStream::new(MockStream::new(reader, writer));

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