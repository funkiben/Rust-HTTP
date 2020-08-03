use std::cmp::min;
use std::io::{Read, Write};

pub struct MockReader {
    data: Vec<Vec<u8>>
}

impl MockReader {
    pub fn new(data: Vec<&str>) -> MockReader {
        MockReader { data: data.into_iter().map(|s| s.as_bytes().to_vec()).collect() }
    }
}

impl Read for MockReader {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        if self.data.is_empty() {
            return Ok(0);
        }

        let next = self.data.first_mut().unwrap();

        let amount = min(buf.len(), next.len());
        let to_read: Vec<u8> = next.drain(0..amount).collect();
        buf.write(&to_read).unwrap();

        if next.is_empty() {
            self.data.remove(0);
        }

        Ok(amount)
    }
}

pub struct EndlessMockReader {
    finite_reader: MockReader,
    sequence: Vec<u8>,
    current: usize,
}

impl EndlessMockReader {
    pub fn new(finite_data: Vec<&str>, sequence: &str) -> EndlessMockReader {
        EndlessMockReader { finite_reader: MockReader::new(finite_data), sequence: sequence.as_bytes().to_vec(), current: 0 }
    }
}

impl Read for EndlessMockReader {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        let size = self.finite_reader.read(buf)?;
        if size > 0 {
            Ok(size)
        } else {
            let start = self.current;
            let range = min(self.sequence.len() - start, buf.len());
            buf.write_all(&self.sequence[start..(start + range)])?;
            self.current = (start + range) % self.sequence.len();
            Ok(range)
        }
    }
}

pub struct MockWriter {
    pub written: Vec<Vec<u8>>,
    pub flushed: Vec<Vec<u8>>,
}

impl MockWriter {
    pub fn new() -> MockWriter {
        MockWriter { written: vec![], flushed: vec![] }
    }
}

impl Write for MockWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.written.push(Vec::from(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushed.append(&mut self.written);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use crate::util::mock::EndlessMockReader;

    fn test_read(reader: &mut impl Read, expected: &str, buf_size: usize) {
        let mut buf = vec![0u8; buf_size];
        let len = reader.read(&mut buf).unwrap();
        assert_eq!(expected, String::from_utf8_lossy(&buf[..len]));
    }

    #[test]
    fn endless_mock_reader() -> std::io::Result<()> {
        let mut reader = EndlessMockReader::new(vec!["hello", "world", "ok bye"], "blah");

        test_read(&mut reader, "hello", 5);
        test_read(&mut reader, "w", 1);
        test_read(&mut reader, "o", 1);
        test_read(&mut reader, "r", 1);
        test_read(&mut reader, "l", 1);
        test_read(&mut reader, "d", 3);
        test_read(&mut reader, "ok b", 4);
        test_read(&mut reader, "ye", 10);
        test_read(&mut reader, "blah", 10);
        test_read(&mut reader, "bla", 3);
        test_read(&mut reader, "h", 3);
        test_read(&mut reader, "blah", 4);
        test_read(&mut reader, "blah", 4);
        test_read(&mut reader, "b", 1);
        test_read(&mut reader, "l", 1);
        test_read(&mut reader, "a", 1);
        test_read(&mut reader, "h", 1);
        test_read(&mut reader, "b", 1);
        test_read(&mut reader, "lah", 3);
        test_read(&mut reader, "bl", 2);
        test_read(&mut reader, "ah", 10);

        for _ in 0..100 {
            test_read(&mut reader, "blah", 10);
        }

        Ok(())
    }
}