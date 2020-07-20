use std::io::{BufReader, BufWriter, Error, Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

use crate::client::config::Config;
use crate::common::HTTP_VERSION;
use crate::common::parse::{ParsingError, read_message};
use crate::common::request::Request;
use crate::common::response::Response;
use crate::common::status::{BAD_REQUEST_400, NOT_FOUND_404, OK_200, Status};

pub struct Client {
    pub config: Config,
    connections: Vec<Mutex<Connection>>,
}

pub enum RequestError {
    ParsingResponse(ResponseParsingError),
    Connection(Error),
    Writing(Error),
}

pub enum ResponseParsingError {
    MissingStatusCode,
    InvalidStatusCode,
    Base(ParsingError),
}

impl Client {
    pub fn new(config: Config) -> Client {
        assert!(config.max_connections > 0);

        let mut connections = Vec::with_capacity(config.max_connections);
        for _ in 0..config.max_connections {
            connections.push(Mutex::new(Connection::new(config.addr, config.read_timeout)))
        }

        Client { connections, config }
    }

    pub fn send(&self, request: &Request) -> Result<Response, RequestError> {
        for connection in &self.connections {
            if let Ok(mut connection) = connection.try_lock() {
                return connection.send(request);
            }
        };

        self.connections.get(0).unwrap().lock().unwrap().send(request)
    }
}

struct Connection {
    addr: &'static str,
    timeout: Duration,
    reader: Option<BufReader<TcpStream>>,
    writer: Option<BufWriter<TcpStream>>,
}

impl Connection {
    fn new(addr: &'static str, timeout: Duration) -> Connection {
        Connection { addr, timeout, reader: None, writer: None }
    }

    fn send(&mut self, request: &Request) -> Result<Response, RequestError> {
        self.try_write(request)?;
        read_next_response(self.reader.as_mut().unwrap()).map_err(|err| RequestError::ParsingResponse(err))
    }

    fn try_write(&mut self, request: &Request) -> Result<(), RequestError> {
        self.ensure_connected()?;
        if let Ok(_) = write_request(self.writer.as_mut().unwrap(), request) {
            Ok(())
        } else {
            self.connect()?;
            write_request(self.writer.as_mut().unwrap(), request).map_err(|err| RequestError::Writing(err))
        }
    }

    fn ensure_connected(&mut self) -> Result<(), RequestError> {
        if let None = self.reader {
            self.connect()?
        }
        Ok(())
    }

    fn connect(&mut self) -> Result<(), RequestError> {
        let stream = TcpStream::connect(self.addr).map_err(|err| RequestError::Connection(err))?;
        let stream_clone = stream.try_clone().map_err(|err| RequestError::Connection(err))?;
        stream.set_read_timeout(Some(self.timeout)).unwrap();

        self.reader = Some(BufReader::new(stream));
        self.writer = Some(BufWriter::new(stream_clone));
        Ok(())
    }
}

fn read_next_response(reader: &mut BufReader<impl Read>) -> Result<Response, ResponseParsingError> {
    let (first_line, headers, body) = read_message(reader, false).map_err(|err| ResponseParsingError::Base(err))?;

    let (http_version, status) = parse_first_line(&first_line)?;

    if !http_version.eq(HTTP_VERSION) {
        return Err(ResponseParsingError::Base(ParsingError::WrongHttpVersion));
    }

    Ok(Response { status, headers, body })
}

fn parse_first_line(line: &str) -> Result<(&str, Status), ResponseParsingError> {
    let mut split = line.split(" ");

    let http_version = split.next().ok_or(ResponseParsingError::Base(ParsingError::MissingHttpVersion))?;
    let status_code = split.next().ok_or(ResponseParsingError::MissingStatusCode)?;

    Ok((http_version, parse_status(status_code)?))
}

fn parse_status(code: &str) -> Result<Status, ResponseParsingError> {
    if code.eq("200") {
        Ok(OK_200)
    } else if code.eq("404") {
        Ok(NOT_FOUND_404)
    } else if code.eq("400") {
        Ok(BAD_REQUEST_400)
    } else {
        Err(ResponseParsingError::InvalidStatusCode)
    }
}

fn write_request(mut writer: impl Write, request: &Request) -> std::io::Result<()> {
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
