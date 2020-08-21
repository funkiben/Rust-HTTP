use std::io::{Error, ErrorKind, Read, Write};
use std::net::SocketAddr;

use crate::common::request::Request;
use crate::parse::error::ParsingError;
use crate::parse::parse::{Parse, ParseStatus};
use crate::parse::request::RequestParser;
use crate::server::connection::ReadRequestResult::{BadData, Closed, NotReady, Ready};
use crate::util::buf_stream::BufStream;

const WRITE_BUF_SIZE: usize = 1024;
const READ_BUF_SIZE: usize = 4096;

/// The result of attempting to read a request.
pub enum ReadRequestResult {
    /// There is not enough data yet for a request to be fully parsed.
    NotReady,
    /// A new request has been parsed.
    Ready(Request),
    /// An error occurred while trying to read a request.
    BadData(ParsingError),
    /// The connection was closed.
    Closed,
}

/// A connection to a client. The main purpose of this is to store the state of request parsing for asynchronous IO.
pub struct Connection<S: Read + Write> {
    /// The address of the client.
    pub addr: SocketAddr,
    stream: BufStream<S>,
    parser: Option<RequestParser>,
}

impl<S: Read + Write> Connection<S> {
    /// Creates a new connection out of the given address and stream.
    pub fn new(addr: SocketAddr, stream: S) -> Connection<S> {
        Connection {
            addr,
            stream: BufStream::with_capacities(stream, READ_BUF_SIZE, WRITE_BUF_SIZE),
            parser: Some(RequestParser::new()),
        }
    }

    /// Attempts to read a request and parse it from the underlying stream.
    pub fn read_request(&mut self) -> ReadRequestResult {
        let parser = self.parser.take().unwrap_or_else(|| RequestParser::new());

        match parser.parse(&mut self.stream) {
            Ok(ParseStatus::Done(request)) => Ready(request),
            Ok(ParseStatus::Blocked(parser)) => {
                self.parser = Some(parser);
                NotReady
            }
            Err(ParsingError::Eof) => Closed,
            Err(ParsingError::Reading(ref error)) if is_closed(error) => Closed,
            Err(res) => BadData(res)
        }
    }

    /// Gets a reference to the underlying stream.
    pub fn stream_ref(&self) -> &S {
        &self.stream.inner_ref()
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

/// Checks if the given IO error indicates whether the connection has closed.
fn is_closed(error: &Error) -> bool {
    // ConnectionAborted is caused from https streams that have closed
    error.kind() == ErrorKind::ConnectionAborted
}