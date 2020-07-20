use std::io::{BufReader, BufWriter, Error, Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

use crate::client::config::Config;
use crate::common::HTTP_VERSION;
use crate::common::request::Request;
use crate::common::response::Response;
use crate::common::status::{BAD_REQUEST_400, NOT_FOUND_404, OK_200, Status};
use crate::util::parse::{ParsingError, read_message};

pub struct Client {
    pub config: Config,
    connections: Vec<Mutex<Connection>>,
}

#[derive(Debug)]
pub enum RequestError {
    ParsingResponse(ResponseParsingError),
    Writing(Error),
}

#[derive(Debug)]
pub enum ResponseParsingError {
    MissingStatusCode,
    InvalidStatusCode,
    Base(ParsingError),
}

impl From<ParsingError> for ResponseParsingError {
    fn from(err: ParsingError) -> Self {
        ResponseParsingError::Base(err)
    }
}

impl From<ResponseParsingError> for RequestError {
    fn from(err: ResponseParsingError) -> Self {
        RequestError::ParsingResponse(err)
    }
}

impl From<Error> for RequestError {
    fn from(err: Error) -> Self {
        RequestError::Writing(err)
    }
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
        self.connections.iter()
            .filter_map(|conn| conn.try_lock().ok())
            .next()
            .unwrap_or(self.connections.get(0).unwrap().lock().unwrap())
            .send(request)
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
        read_next_response(self.reader.as_mut().unwrap()).map_err(ResponseParsingError::into)
    }

    fn try_write(&mut self, request: &Request) -> Result<(), RequestError> {
        self.ensure_connected()?;
        if let Ok(_) = write_request(self.writer.as_mut().unwrap(), request) {
            Ok(())
        } else {
            self.connect()?;
            write_request(self.writer.as_mut().unwrap(), request).map_err(Error::into)
        }
    }

    fn ensure_connected(&mut self) -> Result<(), RequestError> {
        if let None = self.reader {
            self.connect()?
        }
        Ok(())
    }

    fn connect(&mut self) -> Result<(), RequestError> {
        let stream = TcpStream::connect(self.addr)?;
        let stream_clone = stream.try_clone()?;
        stream.set_read_timeout(Some(self.timeout)).unwrap();

        self.reader = Some(BufReader::new(stream));
        self.writer = Some(BufWriter::new(stream_clone));
        Ok(())
    }
}

fn read_next_response(reader: &mut BufReader<impl Read>) -> Result<Response, ResponseParsingError> {
    let (first_line, headers, body) = read_message(reader, false)?;

    let (http_version, status) = parse_first_line(&first_line)?;

    if !http_version.eq(HTTP_VERSION) {
        return Err(ParsingError::WrongHttpVersion.into());
    }

    Ok(Response { status, headers, body })
}

fn parse_first_line(line: &str) -> Result<(&str, Status), ResponseParsingError> {
    let mut split = line.split(" ");

    let http_version = split.next().ok_or(ParsingError::MissingHttpVersion)?;
    let status_code = split.next().ok_or(ResponseParsingError::MissingStatusCode)?;

    Ok((http_version, parse_status(status_code)?))
}

fn parse_status(code: &str) -> Result<Status, ResponseParsingError> {
    // TODO
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

#[cfg(test)]
mod tests {
    use crate::client::client::{ResponseParsingError, read_next_response};
    use crate::common::response::Response;
    use crate::util::mock::MockReader;
    use std::io::BufReader;
    use crate::common::status::OK_200;
    use crate::common::header::{HeaderMap, CONTENT_LENGTH, HeaderMapOps};

    fn test_read_next_response(data: Vec<&str>, expected_result: Result<Response, ResponseParsingError>) {
        let reader = MockReader { data: data.into_iter().map(|s| s.as_bytes().to_vec()).collect() };
        let mut reader = BufReader::new(reader);
        let actual_result = read_next_response(&mut reader);
        assert_eq!(format!("{:?}", expected_result), format!("{:?}", actual_result));
    }

    #[test]
    fn read_request_no_headers_or_body() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\n\r\n"],
            Ok(Response {
                status: OK_200,
                headers: Default::default(),
                body: vec![]
            })
        );
    }

    #[test]
    fn read_request_headers_and_body() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello"],
            Ok(Response {
                status: OK_200,
                headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, "5".to_string())]),
                body: "hello".as_bytes().to_vec()
            })
        );
    }
}