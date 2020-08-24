use std::io::{BufRead, ErrorKind, Write};
use std::net::SocketAddr;

use crate::common::request::Request;
use crate::parse::error::ParsingError;
use crate::parse::parse::{Parse, ParseStatus};
use crate::parse::request::RequestParser;
use crate::server::connection::ReadRequestError::{IoErr, ParseErr};
use crate::server::connection::ReadRequestResult::{Closed, Error, NotReady, Ready};

/// The result of attempting to read a request.
pub enum ReadRequestResult {
    /// There is not enough data yet for a request to be fully parsed.
    NotReady,
    /// A new request has been parsed.
    Ready(Request),
    /// An error occurred while trying to read a request.
    Error(ReadRequestError),
    /// The connection was closed.
    Closed,
}

/// An error that may result from trying to read a request.
#[derive(Debug)]
pub enum ReadRequestError {
    /// An error in parsing the request.
    ParseErr(ParsingError),
    /// An unhandled IO error.
    IoErr(std::io::Error),
}

/// A connection to a client. The main purpose of this is to store the state of asynchronous IO.
pub struct Connection<S: BufRead + Write> {
    /// The address of the client.
    pub addr: SocketAddr,
    stream: S,
    parser: Option<RequestParser>,
}

impl<S: BufRead + Write> Connection<S> {
    /// Creates a new connection out of the given address and stream.
    pub fn new(addr: SocketAddr, stream: S) -> Connection<S> {
        Connection {
            addr,
            stream,
            parser: Some(RequestParser::new()),
        }
    }

    /// Attempts to read a request and parse it from the underlying stream.
    pub fn read_request(&mut self) -> ReadRequestResult {
        let parser = self.parser.take().unwrap_or_else(|| RequestParser::new());

        match parser.parse(&mut self.stream) {
            Ok(ParseStatus::Done(request)) => Ready(request),
            Ok(ParseStatus::IoErr(parser, err)) if err.kind() == ErrorKind::WouldBlock => {
                self.parser = Some(parser);
                NotReady
            }
            Ok(ParseStatus::IoErr(parser, err)) if is_closed(&parser, &err) => Closed,
            Ok(ParseStatus::IoErr(_, err)) => Error(IoErr(err)),
            Err(res) => Error(ParseErr(res))
        }
    }

    /// Gets a reference to the underlying stream.
    pub fn stream_ref(&self) -> &S {
        &self.stream
    }
}

impl<S: BufRead + Write> Write for Connection<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

/// Checks if the given IO error and parser states indicates the connection has closed.
fn is_closed(parser: &RequestParser, error: &std::io::Error) -> bool {
    // If an unexpected EOF was encountered by no data was read then the connection is assumed to be closed.
    (error.kind() == ErrorKind::UnexpectedEof && !parser.has_data())
        // ConnectionAborted is caused from https streams that have closed
        || error.kind() == ErrorKind::ConnectionAborted
}