use std::io::{Error, ErrorKind, Read, Write};
use std::net::SocketAddr;

use crate::common::request::Request;
use crate::parse::error::ParsingError;
use crate::parse::parse::{Parse, ParseStatus};
use crate::parse::request::RequestParser;
use crate::server::connection::ReadRequestResult::{Closed, NotReady, Ready};
use crate::util::buf_stream::BufStream;

const WRITE_BUF_SIZE: usize = 1024;
const READ_BUF_SIZE: usize = 4096;

pub enum ReadRequestResult {
    NotReady,
    Ready(Request),
    Closed,
}

pub struct Connection<S: Read + Write> {
    pub addr: SocketAddr,
    stream: BufStream<S>,
    parser: Option<RequestParser>,
}

impl<S: Read + Write> Connection<S> {
    pub fn new(addr: SocketAddr, stream: S) -> Connection<S> {
        Connection {
            addr,
            stream: BufStream::with_capacities(stream, READ_BUF_SIZE, WRITE_BUF_SIZE),
            parser: Some(RequestParser::new()),
        }
    }

    pub fn read_request(&mut self) -> Result<ReadRequestResult, ParsingError> {
        let parser = self.parser.take().unwrap_or_else(|| RequestParser::new());

        match parser.parse(&mut self.stream) {
            Ok(ParseStatus::Done(request)) => Ok(Ready(request)),
            Ok(ParseStatus::Blocked(parser)) => {
                self.parser = Some(parser);
                Ok(NotReady)
            }
            Err(ParsingError::Eof) => Ok(Closed),
            Err(ParsingError::Reading(ref error)) if is_io_error_ok(error) => Ok(Closed),
            Err(res) => Err(res)
        }
    }
}

impl<S: Read + Write> Write for Connection<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

/// Checks if the given IO error is OK.
fn is_io_error_ok(error: &Error) -> bool {
    // ConnectionAborted is caused from https streams that have closed
    error.kind() == ErrorKind::ConnectionAborted
}