use std::io::{BufRead, BufReader, BufWriter, Read, Result, Write};

pub trait Stream: Read + Write {}

pub trait BufStream: BufRead + Write {}

impl<T: Read + Write> Stream for T {}

impl<T: BufRead + Write> BufStream for T {}

/// Creates a new stream wrapped with the given reader and writer.
// pub fn with_reader_and_writer<W: Write + InnerMut<Inner=WriteableReader<R>>, R: Read + InnerMut<Inner=T>, T: Read + Write>(inner: T, make_reader: fn(T) -> R, make_writer: fn(WriteableReader<R>) -> W) -> impl Stream {
//     ReadableWriter(make_writer(WriteableReader(make_reader(inner))))
// }

/// Creates a new buffered stream wrapped with the given reader and writer.
pub fn with_buf_reader_and_writer<W: Write + InnerMut<Inner=WriteableReader<R>>, R: BufRead + InnerMut<Inner=T> + 'static, T: Stream>(inner: T, make_reader: fn(T) -> R, make_writer: fn(WriteableReader<R>) -> W) -> impl BufStream {
    ReadableWriter(make_writer(WriteableReader(make_reader(inner))))
}

pub struct ReadableWriter<T>(T);

pub struct WriteableReader<T>(T);

impl<R: Read, T: InnerMut<Inner=R>> Read for ReadableWriter<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.inner_mut().read(buf)
    }
}

impl<T: Write> Write for ReadableWriter<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}

impl<R: BufRead + 'static, T: InnerMut<Inner=R>> BufRead for ReadableWriter<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        self.0.inner_mut().fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.inner_mut().consume(amt)
    }
}

impl<T: Read> Read for WriteableReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

impl<W: Write, T: InnerMut<Inner=W>> Write for WriteableReader<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.inner_mut().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.0.inner_mut().flush()
    }
}

impl<T: BufRead> BufRead for WriteableReader<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        self.0.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.consume(amt)
    }
}

pub trait InnerMut {
    type Inner;

    fn inner_mut(&mut self) -> &mut Self::Inner;
}

impl<R> InnerMut for BufReader<R> {
    type Inner = R;

    fn inner_mut(&mut self) -> &mut Self::Inner {
        self.get_mut()
    }
}

impl<W: Write> InnerMut for BufWriter<W> {
    type Inner = W;

    fn inner_mut(&mut self) -> &mut Self::Inner {
        self.get_mut()
    }
}