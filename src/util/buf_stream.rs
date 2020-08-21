use std::io::{BufRead, BufReader, BufWriter, Read, Result, Write};

struct WriteableBufReader<T>(BufReader<T>);

impl<T: Write> Write for WriteableBufReader<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.get_mut().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.get_mut().flush()
    }
}

pub struct BufStream<T: Write>(BufWriter<WriteableBufReader<T>>);

impl<T: Read + Write> BufStream<T> {
    pub fn with_capacities(inner: T, read_buffer_capacity: usize, write_buffer_capacity: usize) -> BufStream<T> {
        BufStream(BufWriter::with_capacity(write_buffer_capacity, WriteableBufReader(BufReader::with_capacity(read_buffer_capacity, inner))))
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