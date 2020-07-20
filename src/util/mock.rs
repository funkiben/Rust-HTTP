use std::io::{Write, Read};
use std::cmp::min;

pub struct MockReader {
    pub data: Vec<Vec<u8>>
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

pub struct MockWriter {
    pub data: Vec<Vec<u8>>,
    pub flushed: Vec<Vec<u8>>,
}

impl Write for MockWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.data.push(Vec::from(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushed.append(&mut self.data);
        Ok(())
    }
}