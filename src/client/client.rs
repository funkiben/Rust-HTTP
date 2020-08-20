use std::io::{BufReader, BufWriter, Error, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

use crate::client::config::Config;
use crate::common::HTTP_VERSION;
use crate::common::request::Request;
use crate::common::response::Response;
use crate::parse::error::ParsingError;
use crate::parse::parse::Parse;
use crate::parse::parse::ParseStatus::{Blocked, Done};
use crate::parse::response::ResponseParser;

/// Client for making HTTP requests.
pub struct Client {
    /// The config the client uses.
    pub config: Config,
    /// The connections to the server.
    connections: Vec<Mutex<Connection>>,
}

/// Error when making an HTTP request.
#[derive(Debug)]
pub enum RequestError {
    /// Error with parsing the response received from the server.
    ResponseParsing(ParsingError),
    /// Error sending the request to the server.
    Sending(Error),
}

impl From<ParsingError> for RequestError {
    fn from(err: ParsingError) -> Self {
        RequestError::ResponseParsing(err)
    }
}

impl From<Error> for RequestError {
    fn from(err: Error) -> Self {
        RequestError::Sending(err)
    }
}

impl Client {
    /// Creates a new client with the given config. Will not actually connect to the server until a request is sent.
    pub fn new(config: Config) -> Client {
        assert!(config.num_connections > 0, "Number of connections must be positive");

        let mut connections = Vec::with_capacity(config.num_connections);
        for _ in 0..config.num_connections {
            connections.push(Mutex::new(Connection::new(config.addr, config.read_timeout)))
        }

        Client { connections, config }
    }

    /// Finds an unused connection to the server and makes a request. The connection will be locked until this method returns.
    /// If all connections are in use then this method will block until a connection is free.
    /// Returns the returned response from the server or an error.
    pub fn send(&self, request: &Request) -> Result<Response, RequestError> {
        loop {
            let mut free = self.connections.iter().filter_map(|conn| conn.try_lock().ok());
            if let Some(mut conn) = free.next() {
                return conn.send(request);
            }
        }
    }
}

/// Connection to a server.
struct Connection {
    /// Address of the server.
    addr: &'static str,
    /// Read timeout for the connection.
    read_timeout: Duration,
    /// Reader for reading from the TCP stream.
    reader: Option<BufReader<TcpStream>>,
    /// Writer for writing to the TCP stream.
    writer: Option<BufWriter<TcpStream>>,
}

impl Connection {
    /// Creates a new connection. Does not actually open a connection to the server until the "send" method is called.
    fn new(addr: &'static str, timeout: Duration) -> Connection {
        Connection { addr, read_timeout: timeout, reader: None, writer: None }
    }

    /// Sends a request to the server and returns the response.
    /// If the connection is not yet open, then a new connection will be opened.
    /// If the request cannot be written, then a new connection is opened and the request is retried once more.
    fn send(&mut self, request: &Request) -> Result<Response, RequestError> {
        self.try_write(request)?;

        let response_parser = ResponseParser::new();
        match response_parser.parse(self.reader.as_mut().unwrap())? {
            Done(response) => Ok(response),
            Blocked(_) => panic!("this will never be reached because the reader is blocking")
        }
    }

    /// Tries to write the request to the server.
    /// If an existing connection is open, then that connection will be written to, otherwise a new connection is opened.
    /// If the existing connection cannot be written to, then a new connection is opened.
    fn try_write(&mut self, request: &Request) -> Result<(), RequestError> {
        self.ensure_connected()?;
        if let Ok(_) = write_request(self.writer.as_mut().unwrap(), request) {
            Ok(())
        } else {
            self.connect()?;
            write_request(self.writer.as_mut().unwrap(), request).map_err(Error::into)
        }
    }

    /// Connects to the server if not already connected.
    fn ensure_connected(&mut self) -> Result<(), RequestError> {
        if let None = self.reader {
            self.connect()?
        }
        Ok(())
    }

    /// Opens a new connection to the server.
    fn connect(&mut self) -> Result<(), RequestError> {
        let stream = TcpStream::connect(self.addr)?;
        let stream_clone = stream.try_clone()?;
        stream.set_read_timeout(Some(self.read_timeout)).unwrap();

        self.reader = Some(BufReader::new(stream));
        self.writer = Some(BufWriter::new(stream_clone));
        Ok(())
    }
}

/// Writes the given request to the given writer.
pub fn write_request(writer: &mut impl Write, request: &Request) -> std::io::Result<()> {
    write!(writer, "{} {} {}\r\n", request.method, request.uri, HTTP_VERSION)?;
    for (header, values) in request.headers.iter() {
        for value in values {
            write!(writer, "{}: {}\r\n", header, value)?;
        }
    }
    writer.write_all(b"\r\n")?;
    writer.write_all(&request.body)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::client::{Client, Config};

    #[test]
    #[should_panic]
    fn zero_connections() {
        Client::new(Config {
            addr: "localhost:7878",
            read_timeout: Duration::from_millis(10),
            num_connections: 0,
        });
    }
}