use std::io::{BufReader, BufWriter, Error, Write};
use std::net::TcpStream;
use std::sync::Mutex;

use rustls::ClientConfig;

use crate::client::client::RequestError::{Reading, Writing};
use crate::client::config::Config;
use crate::client::RequestError::Connecting;
use crate::client::stream_factory::{ClientTlsStream, StreamFactory, TcpStreamFactory, TlsStreamFactory};
use crate::common::request::Request;
use crate::common::response::Response;
use crate::common::version::HTTP_VERSION_1_1;
use crate::parse::error::ParsingError;
use crate::parse::parse::Parse;
use crate::parse::parse::ParseStatus::{Done, IoErr};
use crate::parse::response::ResponseParser;
use crate::util::stream;
use crate::util::stream::{BufStream, StdBufStream, Stream};

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

/// Client for making HTTP requests.
pub struct Client<S: Stream, F> {
    /// The config the client uses.
    pub config: Config,
    /// The connections to the server.
    connections: Vec<Mutex<Connection<S>>>,
    /// Factory for spawning new streams to the server.
    stream_factory: F,
}

impl Client<TcpStream, TcpStreamFactory> {
    /// Creates a new HTTP client.
    pub fn new_http(config: Config) -> Client<TcpStream, TcpStreamFactory> {
        let factory = TcpStreamFactory::new(&config);
        Self::new(config, factory)
    }
}

impl Client<ClientTlsStream, TlsStreamFactory> {
    /// Creates a new HTTP client.
    pub fn new_https(config: Config, tls_config: ClientConfig) -> Client<ClientTlsStream, TlsStreamFactory> {
        let factory = TlsStreamFactory::new(&config, tls_config);
        Self::new(config, factory)
    }
}

impl<S: Stream + 'static, F: StreamFactory<S>> Client<S, F> {
    /// Creates a new HTTP client with the given config. Will not actually connect to the server until a request is sent.
    fn new(config: Config, stream_factory: F) -> Client<S, F> {
        assert!(config.num_connections > 0, "Number of connections must be positive");

        let mut connections = Vec::with_capacity(config.num_connections);
        for _ in 0..config.num_connections {
            connections.push(Mutex::new(Connection::new()))
        }

        Client { connections, config, stream_factory }
    }
}

impl<S: Stream + 'static, F: StreamFactory<S>> Client<S, F> {
    /// Finds an unused connection to the server and makes a request. The connection will be locked until this method returns.
    /// If all connections are in use then this method will block until a connection is free.
    /// Returns the returned response from the server or an error.
    pub fn send(&self, request: &Request) -> Result<Response, RequestError> {
        loop {
            let mut free = self.connections.iter().filter_map(|conn| conn.try_lock().ok());
            if let Some(mut conn) = free.next() {
                return conn.send(&self.stream_factory, request);
            }
        }
    }
}


/// A single connection to the server. Holds the state of the stream to a server.
struct Connection<S: Stream> {
    stream: Option<StdBufStream<S>>,
}

impl<S: Stream + 'static> Connection<S> {
    /// Creates a new empty connection. No stream to the server will be created until the first request is sent.
    fn new() -> Connection<S> {
        Connection { stream: None }
    }

    /// Sends a request to the server and returns the response.
    /// If the connection is not yet open, then a new connection will be opened.
    /// If the request fails at any step (writing or reading), then a new connection is created and the entire request is retried.
    /// New connections are spawned using the given stream_factory argument.
    fn send<F: StreamFactory<S>>(&mut self, stream_factory: &F, request: &Request) -> Result<Response, RequestError> {
        if self.stream.is_none() {
            self.connect(stream_factory)?;
        }

        match send_request(self.stream.as_mut().unwrap(), request) {
            Err(_) => {
                self.connect(stream_factory)?;
                send_request(self.stream.as_mut().unwrap(), request)
            }
            x => x
        }
    }

    /// Opens a new connection to the server.
    fn connect<F: StreamFactory<S>>(&mut self, stream_factory: &F) -> Result<(), RequestError> {
        let new_stream = stream_factory.create().map_err(|err| Connecting(err))?;
        self.stream = Some(stream::with_buf_reader_and_writer(new_stream, BufReader::new, BufWriter::new));
        Ok(())
    }
}

/// Sends a request to the server and returns the response.
fn send_request<T: BufStream>(stream: &mut T, request: &Request) -> Result<Response, RequestError> {
    write_request(stream, request).map_err(|err| Writing(err))?;

    let response_parser = ResponseParser::new();
    match response_parser.parse(stream)? {
        Done(response) => Ok(response),
        IoErr(_, err) => Err(Reading(err))
    }
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
    use std::io::{Error, ErrorKind};
    use std::net::TcpStream;
    use std::sync::Arc;
    use std::thread::spawn;
    use std::time::Duration;

    use crate::client::{Client, Config, write_request};
    use crate::client::stream_factory::StreamFactory;
    use crate::common::header::CONTENT_TYPE;
    use crate::common::method::Method;
    use crate::common::request::Request;
    use crate::header_map;
    use crate::util::mock::MockWriter;

    struct MockFactory;

    impl StreamFactory<TcpStream> for MockFactory {
        fn create(&self) -> std::io::Result<TcpStream> {
            Err(Error::from(ErrorKind::Other))
        }
    }

    #[test]
    #[should_panic]
    fn zero_connections() {
        Client::new(Config {
            addr: "localhost:7878",
            read_timeout: Duration::from_millis(10),
            num_connections: 0,
        }, MockFactory);
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
    fn can_call_send_request_from_multiple_threads() {
        let client = Client::new(Config {
            addr: "0.0.0.0:9000",
            read_timeout: Duration::from_secs(1),
            num_connections: 5,
        }, MockFactory);

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