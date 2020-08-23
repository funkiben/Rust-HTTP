use std::io::{BufRead, BufReader, BufWriter, Read, Result, Write};

/// A buffered stream. Stores buffers for writing and reading.
pub struct BufStream<T: Write>(BufWriter<WriteableBufReader<T>>);

impl<T: Read + Write> BufStream<T> {
    /// Creates a new buffered stream with the given capacities for its buffers.
    pub fn with_capacities(inner: T, read_buffer_capacity: usize, write_buffer_capacity: usize) -> BufStream<T> {
        BufStream(BufWriter::with_capacity(write_buffer_capacity, WriteableBufReader(BufReader::with_capacity(read_buffer_capacity, inner))))
    }

    /// Creates a new buffered stream with default capacities.
    pub fn new(inner: T) -> BufStream<T> {
        BufStream(BufWriter::new(WriteableBufReader(BufReader::new(inner))))
    }

    /// Replaces the inner stream in the buffered reader. Returns the old inner stream.
    pub fn replace_inner(&mut self, new: T) -> T {
        // Flush the writer buffer
        self.0.flush().unwrap_or_default();
        // Consume all the contents of the read buffer
        self.consume(self.0.get_ref().0.buffer().len());

        std::mem::replace(self.0.get_mut().0.get_mut(), new)
    }

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