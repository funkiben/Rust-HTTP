use std::io::{BufReader, BufWriter, Error, Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

use crate::client::config::Config;
use crate::common::HTTP_VERSION;
use crate::common::request::Request;
use crate::common::response::Response;
use crate::common::status::{BAD_REQUEST_400, NOT_FOUND_404, OK_200, Status};
pub use crate::util::parse::ParsingError;
use crate::util::parse::read_message;

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
    ParsingResponse(ResponseParsingError),
    /// Error sending the request to the server.
    Sending(Error),
}


/// Error when parsing an HTTP response from a server.
#[derive(Debug)]
pub enum ResponseParsingError {
    /// Response was missing status code.
    MissingStatusCode,
    /// Response had an unknown status code.
    InvalidStatusCode,
    /// Base parsing error.
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
        read_next_response(self.reader.as_mut().unwrap()).map_err(ResponseParsingError::into)
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

/// Reads a response from the reader.
fn read_next_response(reader: &mut BufReader<impl Read>) -> Result<Response, ResponseParsingError> {
    let (first_line, headers, body) = read_message(reader, true)?;

    let (http_version, status) = parse_first_line(&first_line)?;

    if !http_version.eq(HTTP_VERSION) {
        return Err(ParsingError::WrongHttpVersion.into());
    }

    Ok(Response { status, headers, body })
}

/// Parses the first line of a response.
fn parse_first_line(line: &str) -> Result<(&str, Status), ResponseParsingError> {
    let mut split = line.split(" ");

    let http_version = split.next().ok_or(ParsingError::MissingHttpVersion)?;
    let status_code = split.next().ok_or(ResponseParsingError::MissingStatusCode)?;

    Ok((http_version, parse_status(status_code)?))
}

/// Parses the status code.
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

/// Writes the given request to the given writer.
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
    use std::io::{BufReader, Error, ErrorKind};
    use std::time::Duration;

    use crate::client::{Client, Config};
    use crate::client::client::{read_next_response, ResponseParsingError};
    use crate::client::ResponseParsingError::{InvalidStatusCode, MissingStatusCode};
    use crate::common::header::{CONTENT_LENGTH, Header, HeaderMapOps};
    use crate::common::response::Response;
    use crate::common::status::{BAD_REQUEST_400, NOT_FOUND_404, OK_200};
    use crate::util::mock::MockReader;
    use crate::util::parse::ParsingError::{BadHeader, EOF, InvalidHeaderValue, UnexpectedEOF, WrongHttpVersion, Reading};

    fn test_read_next_response(data: Vec<&str>, expected_result: Result<Response, ResponseParsingError>) {
        let reader = MockReader::from(data);
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
                body: vec![],
            }),
        );
    }

    #[test]
    fn read_request_headers_and_body() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello"],
            Ok(Response {
                status: OK_200,
                headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, "5".to_string())]),
                body: "hello".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn read_request_headers_and_body_fragmented() {
        test_read_next_response(
            vec!["HTT", "P/1.", "1 200 OK", "\r", "\nconte", "nt-length", ":", " 5\r\n\r\nh", "el", "lo"],
            Ok(Response {
                status: OK_200,
                headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, "5".to_string())]),
                body: "hello".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn read_only_one_request() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n", "HTTP/1.1 200 OK\r\n\r\n"],
            Ok(Response {
                status: OK_200,
                headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, "5".to_string())]),
                body: "hello".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn read_request_long_body() {
        let body = b"iuwrhgiuelrguihwleriughwleiruhglweiurhgliwerg fkwfowjeofjiwoefijwef \
        wergiuwehrgiuwehilrguwehlrgiuw fewfwferg wenrjg; weirng lwieurhg owieurhg oeiuwrhg oewirg er\
        gweuirghweiurhgleiwurhglwieurhglweiurhglewiurhto8w374yto8374yt9p18234u50982@#$%#$%^&%^*(^)&(\
        *)_)+__+*()*()&**^%&$##!~!@~``12]\n3'\']\\l[.'\"lk]/l;<:?<:}|?L:|?L|?|:?e       oivj        \
        \n\n\n\n\\\t\t\t\t\t\t\t\\\t\t\t\t                                                          \
        ioerjgfoiaejrogiaergq34t2345123`    oijrgoi wjergi jweorgi jweorgji                 eworigj \
        riogj ewoirgj oewirjg 934598ut6932458t\ruyo3485gh o4w589ghu w458                          9ghu\
        pw94358gh pw93458gh pw9345gh pw9438g\rhu pw3945hg pw43958gh pw495gh :::;wefwefwef wef we  e ;;\
        @#$%@#$^@#$%&#$@%^#$%@#$%@$^%$&$%^*^%&(^$%&*#%^$&@$%^#!#$!~```~~~```wefwef wef ee f efefe e{\
        @#$%@#$^@#$%&#$@%^#$%@#$%@$^%$&$%^*^%&(^$%&*#%^$&@$%^#!#$!~```~~~```wefwef wef ee f efefe e{\
        @#$%@#$^@#$%&#$@%^#$%@#$%@$^%$&$%^*^%&(^$%&*#%^$&@$%^#!#$!~```~~~```wefwef wef ee f efefe e{\
        P{P[p[p[][][][]{}{}][][%%%\n\n\n\n\n\n wefwfw e2123456768960798676reresdsxfbcgrtg eg erg   ";
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 1054\r\n\r\n", &String::from_utf8_lossy(body)],
            Ok(Response {
                status: OK_200,
                headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, "1054".to_string())]),
                body: body.to_vec(),
            }),
        );
    }

    #[test]
    fn read_no_content_length() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n", "HTTP/1.1 200 OK\r\n\r\n"],
            Ok(Response {
                status: OK_200,
                headers: Default::default(),
                body: "helloHTTP/1.1 200 OK\r\n\r\nHTTP/1.1 200 OK\r\n\r\n".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn read_custom_header() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\ncustom-header: custom header value\r\n\r\n"],
            Ok(Response {
                status: OK_200,
                headers: HeaderMapOps::from(vec![(Header::Custom("custom-header".to_string()), "custom header value".to_string())]),
                body: vec![],
            }),
        );
    }

    #[test]
    fn read_404_response() {
        test_read_next_response(
            vec!["HTTP/1.1 404 Not Found\r\n\r\n"],
            Ok(Response {
                status: NOT_FOUND_404,
                headers: Default::default(),
                body: vec![],
            }),
        );
    }

    #[test]
    fn no_status_reason() {
        test_read_next_response(
            vec!["HTTP/1.1 400\r\n\r\n"],
            Ok(Response {
                status: BAD_REQUEST_400,
                headers: Default::default(),
                body: vec![],
            }),
        );
    }

    #[test]
    fn read_gibberish_response() {
        test_read_next_response(
            vec!["ergejrogi jerogij eworfgjwoefjwof9wef wfw"],
            Err(UnexpectedEOF.into()),
        );
    }

    #[test]
    fn read_gibberish_response_with_newline() {
        test_read_next_response(
            vec!["ergejrogi jerogij ewo\nrfgjwoefjwof9wef wfw"],
            Err(UnexpectedEOF.into()),
        );
    }

    #[test]
    fn read_gibberish_with_crlf() {
        test_read_next_response(
            vec!["ergejrogi jerogij ewo\r\nrfgjwoefjwof9wef wfw\r\n\r\n"],
            Err(BadHeader.into()),
        );
    }

    #[test]
    fn read_gibberish_with_crlfs_at_end() {
        test_read_next_response(
            vec!["ergejrogi jerogij eworfgjwoefjwof9wef wfw\r\n\r\n"],
            Err(InvalidStatusCode),
        );
    }

    #[test]
    fn read_all_newlines() {
        test_read_next_response(
            vec!["\n\n\n\n\n\n\n\n\n\n\n"],
            Err(MissingStatusCode),
        );
    }

    #[test]
    fn read_all_crlfs() {
        test_read_next_response(
            vec!["\r\n\r\n\r\n\r\n"],
            Err(MissingStatusCode),
        );
    }

    #[test]
    fn wrong_http_version() {
        test_read_next_response(
            vec!["HTTP/2.0 404 Not Found\r\n\r\n"],
            Err(WrongHttpVersion.into()),
        );
    }

    #[test]
    fn no_status_code() {
        test_read_next_response(
            vec!["HTTP/1.1\r\n\r\n"],
            Err(MissingStatusCode),
        );
    }

    #[test]
    fn missing_crlfs() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK"],
            Err(UnexpectedEOF.into()),
        );
    }

    #[test]
    fn only_one_crlf() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\n"],
            Err(UnexpectedEOF.into()),
        );
    }

    #[test]
    fn bad_header() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\nbad header\r\n\r\n"],
            Err(BadHeader.into()),
        );
    }

    #[test]
    fn bad_content_length_value() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: five\r\n\r\nhello"],
            Err(InvalidHeaderValue.into()),
        );
    }

    #[test]
    fn no_data() {
        test_read_next_response(
            vec![],
            Err(EOF.into()),
        );
    }

    #[test]
    fn one_character() {
        test_read_next_response(
            vec!["a"],
            Err(UnexpectedEOF.into()),
        );
    }

    #[test]
    fn one_crlf_nothing_else() {
        test_read_next_response(
            vec!["\r\n"],
            Err(UnexpectedEOF.into()),
        );
    }

    #[test]
    fn content_length_too_long() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 7\r\n\r\nhello"],
            Err(Reading(Error::new(ErrorKind::UnexpectedEof, "failed to fill whole buffer")).into()),
        );
    }

    #[test]
    fn content_length_too_long_with_request_after() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 7\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n"],
            Ok(Response {
                status: OK_200,
                headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, "7".to_string())]),
                body: "helloHT".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn content_length_too_short() {
        test_read_next_response(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 3\r\n\r\nhello"],
            Ok(Response {
                status: OK_200,
                headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, "3".to_string())]),
                body: "hel".as_bytes().to_vec(),
            }),
        );
    }

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