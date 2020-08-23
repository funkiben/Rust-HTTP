use std::io::{Read, Result, Write};
use std::net::SocketAddr;

use crate::server::connection::Connection;
use crate::server::slots::{ToEmptySlot, ToFilledSlot};
use crate::util::buf_stream::BufStream;

const WRITE_BUF_SIZE: usize = 1024;
const READ_BUF_SIZE: usize = 4096;

type UnusedConnection<T> = BufStream<MaybeStream<T>>;
pub type UsedConnection<T> = Connection<BufStream<MaybeStream<T>>>;

pub enum MaybeStream<T> {
    Stream(T),
    Empty,
}

impl<T> MaybeStream<T> {
    pub fn unwrap_ref(&self) -> &T {
        match self {
            MaybeStream::Stream(stream) => stream,
            _ => panic!("tried to unwrap empty stream")
        }
    }

    pub fn unwrap_mut(&mut self) -> &mut T {
        match self {
            MaybeStream::Stream(stream) => stream,
            _ => panic!("tried to unwrap empty stream")
        }
    }

    pub fn unwrap(self) -> T {
        match self {
            MaybeStream::Stream(stream) => stream,
            _ => panic!("tried to unwrap empty stream")
        }
    }
}

impl<T: Write> Write for MaybeStream<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.unwrap_mut().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.unwrap_mut().flush()
    }
}

impl<T: Read> Read for MaybeStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.unwrap_mut().read(buf)
    }
}

impl<T: Read + Write> Default for BufStream<MaybeStream<T>> {
    fn default() -> Self {
        BufStream::with_capacities(MaybeStream::Empty, READ_BUF_SIZE, WRITE_BUF_SIZE)
    }
}

impl<T: Write + Read> ToFilledSlot<(SocketAddr, T), UsedConnection<T>> for UnusedConnection<T> {
    fn to_filled_slot(mut self, (addr, inner): (SocketAddr, T)) -> UsedConnection<T> {
        self.replace_inner(MaybeStream::Stream(inner));
        Connection::new(addr, self)
    }
}

impl<T: Write + Read> ToEmptySlot<(SocketAddr, T), UnusedConnection<T>> for UsedConnection<T> {
    fn to_empty_slot(self) -> ((SocketAddr, T), UnusedConnection<T>) {
        let addr = self.addr;
        let mut stream = self.into_inner();
        let old = stream.replace_inner(MaybeStream::Empty);
        ((addr, old.unwrap()), stream)
    }
}