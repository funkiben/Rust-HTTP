use std::io::{BufRead, BufReader, BufWriter, Read, Result, Write};

/// A BufStream using the standard libraries BufWriter and BufReader implementations.
pub type StdBufStream<T> = ReadableWriter<BufWriter<WriteableReader<BufReader<T>>>>;

/// A stream that can be read and written.
pub trait Stream: Read + Write {}

impl<T: Read + Write> Stream for T {}

/// A stream that supports buffered reading operations.
pub trait BufStream: BufRead + Write {}

impl<T: BufRead + Write> BufStream for T {}

/// Creates a new buffered stream by wrapping the given stream with a reader and writer.
pub fn with_buf_reader_and_writer<W, R, T>(
    inner: T,
    make_reader: fn(T) -> R,
    make_writer: fn(WriteableReader<R>) -> W,
) -> ReadableWriter<W>
    where W: Write + InnerMut<Inner=WriteableReader<R>>,
          R: BufRead + InnerMut<Inner=T> + 'static,
          T: Stream
{
    ReadableWriter(make_writer(WriteableReader(make_reader(inner))))
}


/// Wrapper with some inner value that can be mutably referenced.
pub trait InnerMut {
    type Inner;

    /// Gets a mutable reference to the inner value.
    fn inner_mut(&mut self) -> &mut Self::Inner;
}

/// A writer that contains a readable inner.
pub struct ReadableWriter<T>(T);

/// A reader that contains a writable inner.
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

