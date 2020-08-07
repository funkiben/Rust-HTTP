use std::net::SocketAddr;

use tokio::io::{AsyncWriteExt, BufReader, BufWriter, Error};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::client::config::Config;
use crate::common::HTTP_VERSION;
use crate::common::parse::error::ResponseParsingError;
use crate::common::parse::read_response;
use crate::common::request::Request;
use crate::common::response::Response;

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
    ResponseParsing(ResponseParsingError),
    /// Error sending the request to the server.
    Sending(Error),
}

impl From<ResponseParsingError> for RequestError {
    fn from(err: ResponseParsingError) -> Self {
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
            connections.push(Mutex::new(Connection::new(config.addr)))
        }

        Client { connections, config }
    }

    /// Finds an unused connection to the server and makes a request. The connection will be locked until this method returns.
    /// If all connections are in use then this method will block until a connection is free.
    /// Returns the returned response from the server or an error.
    pub async fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let mut free = self.connections.iter().filter_map(|conn| conn.try_lock().ok());
        let mut conn = if let Some(conn) = free.next() {
            conn
        } else {
            self.connections.get(0).unwrap().lock().await
        };

        conn.send(request).await
    }
}

/// Connection to a server.
struct Connection {
    /// Address of the server.
    addr: SocketAddr,
    /// Reader for reading from the TCP stream.
    reader: Option<BufReader<OwnedReadHalf>>,
    /// Writer for writing to the TCP stream.
    writer: Option<BufWriter<OwnedWriteHalf>>,
}

impl Connection {
    /// Creates a new connection. Does not actually open a connection to the server until the "send" method is called.
    fn new(addr: SocketAddr) -> Connection {
        Connection { addr, reader: None, writer: None }
    }

    /// Sends a request to the server and returns the response.
    /// If the connection is not yet open, then a new connection will be opened.
    /// If the request cannot be written, then a new connection is opened and the request is retried once more.
    async fn send(&mut self, request: &Request) -> Result<Response, RequestError> {
        self.try_write(request).await?;
        read_response(self.reader.as_mut().unwrap()).await.map_err(ResponseParsingError::into)
    }

    /// Tries to write the request to the server.
    /// If an existing connection is open, then that connection will be written to, otherwise a new connection is opened.
    /// If the existing connection cannot be written to, then a new connection is opened.
    async fn try_write(&mut self, request: &Request) -> Result<(), RequestError> {
        self.ensure_connected().await?;
        if let Ok(_) = write_request(self.writer.as_mut().unwrap(), request).await {
            Ok(())
        } else {
            self.connect().await?;
            write_request(self.writer.as_mut().unwrap(), request).await.map_err(Error::into)
        }
    }

    /// Connects to the server if not already connected.
    async fn ensure_connected(&mut self) -> Result<(), RequestError> {
        if let None = self.reader {
            self.connect().await?
        }
        Ok(())
    }

    /// Opens a new connection to the server.
    async fn connect(&mut self) -> Result<(), RequestError> {
        let stream = TcpStream::connect(self.addr).await?;

        let (reader, writer) = stream.into_split();

        self.reader = Some(BufReader::new(reader));
        self.writer = Some(BufWriter::new(writer));
        Ok(())
    }
}

/// Writes the given request to the given writer.
async fn write_request(mut writer: impl AsyncWriteExt + Unpin, request: &Request) -> std::io::Result<()> {
    writer.write_all(request.method.to_string().as_bytes()).await?;
    writer.write_all(b" ").await?;
    writer.write_all(request.uri.as_bytes()).await?;
    writer.write_all(b" ").await?;
    writer.write_all(HTTP_VERSION.as_bytes()).await?;
    writer.write_all(b"\r\n").await?;
    for (header, values) in request.headers.iter() {
        for value in values {
            writer.write_all(header.as_str().as_bytes()).await?;
            writer.write_all(b": ").await?;
            writer.write_all(value.as_bytes()).await?;
            writer.write_all(b"\r\n").await?;
        }
    }
    writer.write_all(b"\r\n").await?;
    writer.write_all(&request.body).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::client::{Client, Config};

    #[test]
    #[should_panic]
    fn zero_connections() {
        Client::new(Config {
            addr: "0.0.0.0:7878".parse().unwrap(),
            num_connections: 0,
        });
    }
}