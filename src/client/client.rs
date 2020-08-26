use std::io::{BufReader, BufWriter, Error, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::Duration;

use crate::client::config::Config;
use crate::client::RequestError::{Connecting, Reading, Writing};
use crate::common::request::Request;
use crate::common::response::Response;
use crate::common::version::HTTP_VERSION_1_1;
use crate::parse::error::ParsingError;
use crate::parse::parse::Parse;
use crate::parse::parse::ParseStatus::{Done, IoErr};
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
    /// Error connecting to the server.
    Connecting(Error),
    /// Error reading responses from server.
    Reading(Error),
    /// Error sending the request to the server.
    Writing(Error),
}

impl From<ParsingError> for RequestError {
    fn from(err: ParsingError) -> Self {
        RequestError::ResponseParsing(err)
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
    /// If a request was written but a response can not be read, then a new connection is made and the request is retried once.
    fn send(&mut self, request: &Request) -> Result<Response, RequestError> {
        self.ensure_connected()?;

        if self.write_request(request).is_err() {
            self.connect()?;
            self.write_request(request)?
        }

        match self.read_response() {
            Ok(response) => Ok(response),
            Err(_) => {
                self.connect()?;
                self.write_request(request)?;
                self.read_response()
            }
        }
    }

    /// Attempts to read a response from the stream.
    fn read_response(&mut self) -> Result<Response, RequestError> {
        let response_parser = ResponseParser::new();
        match response_parser.parse(self.reader.as_mut().unwrap())? {
            Done(response) => Ok(response),
            IoErr(_, err) => Err(Reading(err))
        }
    }

    /// Attempts to write a request to the stream.
    fn write_request(&mut self, request: &Request) -> Result<(), RequestError> {
        write_request(self.writer.as_mut().unwrap(), request).map_err(|err| Writing(err))
    }

    /// Connects to the server if not already connected.
    fn ensure_connected(&mut self) -> Result<(), RequestError> {
        if self.reader.is_none() || self.writer.is_none() {
            self.connect()?;
        }
        Ok(())
    }

    /// Opens a new connection to the server.
    fn connect(&mut self) -> Result<(), RequestError> {
        let (reader, writer) =
            connect(self.addr, self.read_timeout).map_err(|err| Connecting(err))?;
        self.reader = Some(reader);
        self.writer = Some(writer);
        Ok(())
    }
}

/// Opens a new connection to the specified address and returns a reader and writer for communication.
fn connect<A: ToSocketAddrs>(addr: A, read_timeout: Duration) -> Result<(BufReader<TcpStream>, BufWriter<TcpStream>), Error> {
    let stream = TcpStream::connect(addr)?;
    let stream_clone = stream.try_clone()?;
    stream.set_read_timeout(Some(read_timeout)).unwrap();

    Ok((BufReader::new(stream), BufWriter::new(stream_clone)))
}

/// Writes the given request to the given writer.
pub fn write_request(writer: &mut impl Write, request: &Request) -> std::io::Result<()> {
    write!(writer, "{} {} {}\r\n", request.method, request.uri, HTTP_VERSION_1_1)?;
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
    use std::sync::Arc;
    use std::thread::spawn;
    use std::time::Duration;

    use crate::client::{Client, Config, write_request};
    use crate::common::header::CONTENT_TYPE;
    use crate::common::method::Method;
    use crate::common::request::Request;
    use crate::header_map;
    use crate::util::mock::MockWriter;

    #[test]
    #[should_panic]
    fn zero_connections() {
        Client::new(Config {
            addr: "localhost:7878",
            read_timeout: Duration::from_millis(10),
            num_connections: 0,
        });
    }

    #[test]
    fn write_request_with_headers_and_body() {
        let request = Request {
            uri: "/hello/blah".to_string(),
            method: Method::POST,
            headers: header_map![
                (CONTENT_TYPE, String::from("hello")),
                ("HeLlO", "blah")
            ],
            body: b"the body".to_vec(),
        };

        let mut writer = MockWriter::new();

        write_request(&mut writer, &request).unwrap();

        let bytes = writer.flushed.borrow().concat();
        let request_bytes_as_string = String::from_utf8_lossy(&bytes);

        assert!(
            request_bytes_as_string.eq("POST /hello/blah HTTP/1.1\r\ncontent-type: hello\r\nhello: blah\r\n\r\nthe body")
                || request_bytes_as_string.eq("POST /hello/blah HTTP/1.1\r\nhello: blah\r\ncontent-type: hello\r\n\r\nthe body")
        )
    }

    #[test]
    fn write_empty_request() {
        let request = Request {
            uri: "/".to_string(),
            method: Method::GET,
            headers: header_map![],
            body: vec![],
        };
        let mut buf: Vec<u8> = vec![];
        write_request(&mut buf, &request).unwrap();
        assert_eq!(String::from_utf8_lossy(&buf), "GET / HTTP/1.1\r\n\r\n")
    }

    #[test]
    fn write_response_one_header_no_body_to_bytes() {
        let request = Request {
            uri: "/".to_string(),
            method: Method::GET,
            headers: header_map![
                ("custom header", "header value")
            ],
            body: vec![],
        };
        let mut buf: Vec<u8> = vec![];
        write_request(&mut buf, &request).unwrap();
        assert_eq!(String::from_utf8_lossy(&buf), "GET / HTTP/1.1\r\ncustom header: header value\r\n\r\n")
    }

    #[test]
    fn can_send_requests_from_multiple_threads() {
        let client = Client::new(Config {
            addr: "0.0.0.0:9000",
            read_timeout: Duration::from_secs(1),
            num_connections: 5,
        });

        let client = Arc::new(client);

        let mut handlers = vec![];

        for _ in 0..5 {
            let client = client.clone();
            handlers.push(spawn(move ||
                client.send(&Request {
                    uri: "/".to_string(),
                    method: Method::GET,
                    headers: header_map![],
                    body: vec![],
                }).map(|_| ()).unwrap_or_default()
            ));
        }

        for handler in handlers {
            handler.join().unwrap();
        }
    }
}