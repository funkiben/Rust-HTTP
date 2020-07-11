use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use crate::common::header::{CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, Header, HeaderMap, HeaderMapOps};
use crate::common::header::Header::Custom;
use crate::common::method::Method;
use crate::common::method::Method::{Delete, Get, Post, Put};
use crate::common::request::Request;
use crate::common::response::Response;
use crate::common::thread_pool::ThreadPool;
use crate::server::config::Config;
use crate::server::router::Router;

const HTTP_VERSION: &str = "HTTP/1.1";

const REQUEST_PARSING_ERROR_RESPONSE: &[u8; 28] = b"HTTP/1.1 400 Bad Request\r\n\r\n";

/// An HTTP server.
pub struct Server {
    /// The config for the server.
    pub config: Config,
    /// The router used for handling requests received from connections
    pub router: Router,
    no_route_response_bytes: Vec<u8>,
}

impl Server {
    /// Creates a new HTTP server with the given config.
    pub fn new(config: Config) -> Server {
        Server {
            no_route_response_bytes: response_to_bytes(&config.no_route_response),
            config,
            router: Router::new(),
        }
    }

    /// Starts the HTTP server. This function will block and listen for new connections.
    pub fn start(self) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.config.addr)?;

        let thread_pool = ThreadPool::new(self.config.connection_handler_threads);

        let server = Arc::new(self);

        listener.incoming()
            .filter_map(|stream| {
                stream.map_err(|error| {
                    println!("Error unwrapping new connection: {}", error);
                    error
                }).ok()
            })
            .for_each(|stream| {
                let server = Arc::clone(&server);
                thread_pool.execute(move || {
                    if let Err(error) = server.handle_connection(stream) {
                        println!("Error handling connection: {}", error);
                    }
                })
            });

        Ok(())
    }

    /// Handles a new connection.
    fn handle_connection(&self, stream: TcpStream) -> std::io::Result<()> {
        stream.set_read_timeout(Some(self.config.read_timeout)).unwrap();

        respond_to_requests(&stream, &stream, |request|
            self.router.response(request)
                .map(|response| response_to_bytes(&response))
                .unwrap_or(self.no_route_response_bytes.clone()))
    }
}

/// Calls the "get_response" function while valid HTTP requests can be read from the given reader.
/// Will return as soon as the connection is closed or an invalid HTTP request is sent.
/// The result of the "get_response" function is written to the writer before the next request is read.
fn respond_to_requests<R: Read, W: Write>(reader: R, mut writer: W, get_response: impl Fn(Request) -> Vec<u8>) -> std::io::Result<()> {
    let result = read_requests(reader, |request| {
        let should_close_after_response = should_close_after_response(&request);
        let write_result = writer.write(&get_response(request));
        let flush_result = writer.flush();
        should_close_after_response || write_result.is_err() || flush_result.is_err()
    });

    if let Err(error) = result {
        println!("Error: {:?}", error);
        writer.write(REQUEST_PARSING_ERROR_RESPONSE)?;
        writer.flush()?;
    }

    Ok(())
}

/// Checks if the given connection should be closed after a response is sent to the given request.
fn should_close_after_response(request: &Request) -> bool {
    request.headers.contains_header_value(&CONNECTION, "close")
}

/// Reads requests from the given reader until there is an invalid request or the connection is closed.
/// Calls "on_request" for each request read.
/// If "on_request" returns true, the function will return with Ok.
fn read_requests<R: Read>(reader: R, mut on_request: impl FnMut(Request) -> bool) -> Result<(), RequestParsingError> {
    let mut reader = BufReader::new(reader);
    loop {
        let request = read_request(&mut reader);

        match request {
            Ok(request) => if on_request(request) { return Ok(()); },
            Err(RequestParsingError::EOF) => return Ok(()),
            err => return err.map(|_| {})
        }
    }
}

/// Reads a request from the given buffered reader.
/// If the data from the reader does not form a valid request or the connection has been closed, returns an error.
fn read_request(reader: &mut BufReader<impl Read>) -> Result<Request, RequestParsingError> {
    let first_line = read_line(reader).map_err(|err|
        if let RequestParsingError::UnexpectedEOF = err { RequestParsingError::EOF } else { err }
    )?;

    let (method, uri, http_version) = parse_first_line(first_line)?;
    let headers = parse_headers(read_lines_until_empty_line(reader)?)?;

    let body = if let Some(value) = headers.get_first_header_value(&CONTENT_LENGTH) {
        let body_length = value.parse().or(Err(RequestParsingError::InvalidHeaderValue))?;
        read_body(reader, body_length)?
    } else {
        Vec::new()
    };

    if !http_version.eq(HTTP_VERSION) {
        return Err(RequestParsingError::WrongHttpVersion);
    }

    Ok(Request { method, uri, headers, body })
}

/// Reads a request body from the reader. The body_length is used to determine how much to read.
fn read_body(reader: &mut impl Read, body_length: usize) -> Result<Vec<u8>, RequestParsingError> {
    let mut buf = vec![0; body_length];
    reader.read_exact(&mut buf).or_else(|e| Err(RequestParsingError::Reading(e)))?;
    Ok(buf)
}

/// Reads a single line, assuming the line ends in a CRLF ("\r\n").
/// The CRLF is not included in the returned string.
fn read_line(reader: &mut BufReader<impl Read>) -> Result<String, RequestParsingError> {
    let mut line = String::new();
    reader.read_line(&mut line).or_else(|e| Err(RequestParsingError::Reading(e)))?;

    if line.is_empty() {
        return Err(RequestParsingError::UnexpectedEOF);
    }

    // pop the \r\n off the end of the line
    line.pop();
    line.pop();

    Ok(line)
}

/// Reads lines from the buffered reader. The returned lines do not include a CRLF.
fn read_lines_until_empty_line(reader: &mut BufReader<impl Read>) -> Result<Vec<String>, RequestParsingError> {
    let mut lines = Vec::new();

    loop {
        let line = read_line(reader)?;

        if line.is_empty() {
            return Ok(lines);
        }

        lines.push(line);
    }
}

/// Tries to parse the given lines as headers.
/// Each line is parsed with the format "V: K" where V is the header name and K is the header value.
fn parse_headers(lines: Vec<String>) -> Result<HeaderMap, RequestParsingError> {
    let mut headers = HashMap::new();
    for line in lines {
        let (header, value) = parse_header(line)?;
        headers.add_header(header, value);
    }
    Ok(headers)
}

/// Parses the given line as a header. Splits the line at the first ": " pattern.
fn parse_header(raw: String) -> Result<(Header, String), RequestParsingError> {
    let mut split = raw.splitn(2, ": ");

    let header_raw = split.next().ok_or(RequestParsingError::ParsingHeader)?;
    let value = split.next().ok_or(RequestParsingError::ParsingHeader)?;

    Ok((parse_header_name(header_raw), String::from(value)))
}

/// Parses the given header name. Will try to use a predefined header constant if possible to save memory.
/// Otherwise, will return a Custom header.
fn parse_header_name(raw: &str) -> Header {
    // TODO avoid ignore case eq
    if raw.eq_ignore_ascii_case("Connection") {
        return CONNECTION;
    } else if raw.eq_ignore_ascii_case("Content-Length") {
        return CONTENT_LENGTH;
    } else if raw.eq_ignore_ascii_case("Content-Type") {
        return CONTENT_TYPE;
    }
    Custom(String::from(raw))
}

/// Parses the given line as the first line of a request.
/// The first lines of requests have the form: "Method Request-URI HTTP-Version CRLF"
fn parse_first_line(line: String) -> Result<(Method, String, String), RequestParsingError> {
    let mut split = line.split(" ");

    let method_raw = split.next().ok_or(RequestParsingError::MissingMethod)?;
    let uri = split.next().ok_or(RequestParsingError::MissingURI)?;
    let http_version = split.next().ok_or(RequestParsingError::MissingHttpVersion)?;

    Ok((parse_method(method_raw)?, String::from(uri), String::from(http_version)))
}

/// Parses the given string into a method. If the method is not recognized, will return an error.
fn parse_method(raw: &str) -> Result<Method, RequestParsingError> {
    if raw.eq("GET") {
        Ok(Get)
    } else if raw.eq("POST") {
        Ok(Post)
    } else if raw.eq("DELETE") {
        Ok(Delete)
    } else if raw.eq("PUT") {
        Ok(Put)
    } else {
        Err(RequestParsingError::UnrecognizedMethod(String::from(raw)))
    }
}

/// Converts the given response into an HTTP response as bytes.
fn response_to_bytes(response: &Response) -> Vec<u8> {
    let mut buf = format!("{} {} {}\r\n", HTTP_VERSION, response.status.code, response.status.reason);

    for (header, values) in response.headers.iter() {
        for value in values {
            buf.push_str(format!("{}: {}\r\n", header.as_str(), value).as_str());
        }
    }

    buf.push_str("\r\n");

    let mut buf = buf.into_bytes();

    buf.extend(&response.body);

    buf
}

/// The possible errors that can be encountered when trying to parse a request.
#[derive(Debug)]
enum RequestParsingError {
    /// Error reading from the reader.
    Reading(std::io::Error),
    /// Missing method from first line of request.
    MissingMethod,
    /// Missing URI from first line of request.
    MissingURI,
    /// Missing HTTP version from first line of request.
    MissingHttpVersion,
    /// Request has wrong HTTP version.
    WrongHttpVersion,
    /// Problem parsing a request header.
    ParsingHeader,
    /// Method is unrecognized.
    UnrecognizedMethod(String),
    /// Header has invalid value.
    InvalidHeaderValue,
    /// Unexpected EOF will be thrown when EOF is found in the middle of reading a request.
    UnexpectedEOF,
    /// EOF found before any request can be read.
    EOF,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::cmp::min;
    use std::collections::HashMap;
    use std::io::{Read, Write};

    use crate::common::header::{CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, Header, HeaderMap, HeaderMapOps};
    use crate::common::method::Method;
    use crate::common::request::Request;
    use crate::common::response::Response;
    use crate::common::status::OK_200;
    use crate::server::server::{respond_to_requests, response_to_bytes};

    struct MockReader {
        data: Vec<Vec<u8>>
    }

    impl Read for MockReader {
        fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
            if self.data.is_empty() {
                return Ok(0);
            }

            let next = self.data.first_mut().unwrap();

            let amount = min(buf.len(), next.len());
            let to_read: Vec<u8> = next.drain(0..amount).collect();
            buf.write(&to_read).unwrap();

            if next.is_empty() {
                self.data.remove(0);
            }

            Ok(amount)
        }
    }

    struct MockWriter {
        data: Vec<Vec<u8>>,
        flushed: Vec<Vec<u8>>,
    }

    impl Write for MockWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.data.push(Vec::from(buf));
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.flushed.append(&mut self.data);
            Ok(())
        }
    }

    fn test_respond_to_requests(input: Vec<&str>, responses: Vec<&str>, expected_requests: Vec<Request>, expected_output: &str) {
        let reader = MockReader {
            data: input.into_iter().map(String::from).map(String::into_bytes).collect()
        };

        let mut writer = MockWriter { data: vec![], flushed: vec![] };

        let actual_requests = RefCell::new(Vec::new());

        let responses = RefCell::new(responses);
        let result = respond_to_requests(reader, &mut writer, |request| {
            actual_requests.borrow_mut().push(request);
            Vec::from(responses.borrow_mut().remove(0).as_bytes())
        });

        assert!(result.is_ok());

        let actual_output = writer.flushed.concat();
        let actual_output = String::from_utf8_lossy(&actual_output);

        assert_eq!(expected_output, actual_output);
        assert_eq!(expected_requests, actual_requests.into_inner());
    }

    fn test_respond_to_requests_no_bad(input: Vec<&str>, expected_requests: Vec<Request>) {
        test_respond_to_requests_with_last_response(input, expected_requests, "");
    }

    fn test_respond_to_requests_with_last_response(input: Vec<&str>, expected_requests: Vec<Request>, last_response: &str) {
        let mut responses: Vec<String> = (0..expected_requests.len()).map(|n| n.to_string()).collect();
        responses.push(String::from(last_response));
        let responses: Vec<&str> = responses.iter().map(|s| s.as_str()).collect();
        let expected_output: String = responses.concat();
        test_respond_to_requests(input, responses, expected_requests, &expected_output);
    }

    #[test]
    fn no_data() {
        test_respond_to_requests(vec![], vec![], vec![], "");
    }

    #[test]
    fn one_request() {
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\n\r\n"],
            vec![Request {
                uri: String::from("/"),
                method: Method::Get,
                headers: HeaderMap::new(),
                body: vec![],
            }])
    }

    #[test]
    fn one_request_multiple_fragments() {
        test_respond_to_requests_no_bad(
            vec!["G", "ET / ", "HTTP/1", ".1\r\n", "\r", "\n"],
            vec![Request {
                uri: String::from("/"),
                method: Method::Get,
                headers: HeaderMap::new(),
                body: vec![],
            }])
    }

    #[test]
    fn one_request_interesting_uri() {
        test_respond_to_requests_no_bad(
            vec!["GET /hello/world/ HTTP/1.1\r\n\r\n"],
            vec![Request {
                uri: String::from("/hello/world/"),
                method: Method::Get,
                headers: HeaderMap::new(),
                body: vec![],
            }])
    }

    #[test]
    fn one_request_weird_uri() {
        test_respond_to_requests_no_bad(
            vec!["GET !#%$#/-+=_$+[]{}\\%&$ HTTP/1.1\r\n\r\n"],
            vec![Request {
                uri: String::from("!#%$#/-+=_$+[]{}\\%&$"),
                method: Method::Get,
                headers: HeaderMap::new(),
                body: vec![],
            }])
    }

    #[test]
    fn one_request_many_spaces_in_first_line() {
        test_respond_to_requests_no_bad(
            vec!["GET /hello/world/ HTTP/1.1 hello there blah blah\r\n\r\n"],
            vec![Request {
                uri: String::from("/hello/world/"),
                method: Method::Get,
                headers: HeaderMap::new(),
                body: vec![],
            }])
    }

    #[test]
    fn two_requests() {
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\n\r\n", "POST / HTTP/1.1\r\n\r\n"],
            vec![
                Request {
                    uri: String::from("/"),
                    method: Method::Get,
                    headers: HeaderMap::new(),
                    body: vec![],
                },
                Request {
                    uri: String::from("/"),
                    method: Method::Post,
                    headers: HeaderMap::new(),
                    body: vec![],
                }
            ])
    }

    #[test]
    fn one_request_with_headers() {
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\ncontent-length: 0\r\nconnection: close\r\nsomething: hello there goodbye\r\n\r\n"],
            vec![Request {
                uri: String::from("/"),
                method: Method::Get,
                headers: HeaderMapOps::from(vec![
                    (CONTENT_LENGTH, String::from("0")),
                    (CONNECTION, String::from("close")),
                    (Header::Custom(String::from("something")), String::from("hello there goodbye")),
                ]),
                body: vec![],
            }])
    }

    #[test]
    fn one_request_with_body() {
        let body = b"hello";
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\ncontent-length: 5\r\n\r\nhello"],
            vec![Request {
                uri: String::from("/"),
                method: Method::Get,
                headers: HeaderMapOps::from(vec![
                    (CONTENT_LENGTH, String::from("5")),
                ]),
                body: body.to_vec(),
            }])
    }

    #[test]
    fn two_requests_with_bodies() {
        let body1 = b"hello";
        let body2 = b"goodbye";
        test_respond_to_requests_no_bad(
            vec![
                "GET /body1 HTTP/1.1\r\ncontent-length: 5\r\n\r\nhello",
                "GET /body2 HTTP/1.1\r\ncontent-length: 7\r\n\r\ngoodbye"
            ],
            vec![
                Request {
                    uri: String::from("/body1"),
                    method: Method::Get,
                    headers: HeaderMapOps::from(vec![
                        (CONTENT_LENGTH, String::from("5")),
                    ]),
                    body: body1.to_vec(),
                },
                Request {
                    uri: String::from("/body2"),
                    method: Method::Get,
                    headers: HeaderMapOps::from(vec![
                        (CONTENT_LENGTH, String::from("7")),
                    ]),
                    body: body2.to_vec(),
                }
            ],
        )
    }

    #[test]
    fn two_requests_connection_close_header() {
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\nconnection: close\r\n\r\n", "POST / HTTP/1.1\r\n\r\n"],
            vec![
                Request {
                    uri: String::from("/"),
                    method: Method::Get,
                    headers: HeaderMapOps::from(vec![(CONNECTION, String::from("close"))]),
                    body: vec![],
                }
            ])
    }

    #[test]
    fn bad_request_gibberish() {
        test_respond_to_requests_with_last_response(
            vec!["regw", "\nergrg\n", "ie\n\n\nwof"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn no_requests_read_after_bad_request() {
        test_respond_to_requests_with_last_response(
            vec!["regw", "\nergrg\n", "ie\n\n\nwof\r\n\r\n", "POST / HTTP/1.1\r\n\r\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn bad_request_lots_of_newlines() {
        test_respond_to_requests_with_last_response(
            vec!["\n\n\n\n\n", "\n\n\n", "\n\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn bad_request_no_newlines() {
        test_respond_to_requests_with_last_response(
            vec!["wuirghuiwuhfwf", "iouwejf", "ioerjgiowjergiuhwelriugh"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn invalid_method() {
        test_respond_to_requests_with_last_response(
            vec!["yadadada / HTTP/1.1\r\n\r\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn wrong_http_version() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.0\r\n\r\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn missing_uri_and_version() {
        test_respond_to_requests_with_last_response(
            vec!["GET\r\n\r\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn missing_http_version() {
        test_respond_to_requests_with_last_response(
            vec!["GET /\r\n\r\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn bad_crlf() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1\n\r\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn bad_header() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1\r\nyadadada\r\n\r\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn header_with_newline() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1\r\nhello: wgwf\niwjfw\r\n\r\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn missing_crlf_after_last_header() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1\r\nhello: wgwf\r\n"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn missing_crlfs() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n")
    }

    #[test]
    fn request_with_body_and_no_content_length() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1\r\n\r\nhello"],
            vec![
                Request {
                    uri: String::from("/"),
                    method: Method::Get,
                    headers: HeaderMap::new(),
                    body: vec![],
                }
            ],
            "HTTP/1.1 400 Bad Request\r\n\r\n");
    }

    #[test]
    fn request_with_body_and_too_short_content_length() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1\r\ncontent-length: 3\r\n\r\nhello"],
            vec![
                Request {
                    uri: String::from("/"),
                    method: Method::Get,
                    headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, String::from("3"))]),
                    body: b"hel".to_vec(),
                }
            ],
            "HTTP/1.1 400 Bad Request\r\n\r\n");
    }

    #[test]
    fn request_with_body_and_too_long_content_length() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1\r\ncontent-length: 10\r\n\r\nhello"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n");
    }

    #[test]
    fn response_with_headers_and_body_to_bytes() {
        let response = Response {
            status: &OK_200,
            headers: HeaderMapOps::from(vec![
                (CONTENT_TYPE, String::from("hello")),
                (CONNECTION, String::from("bye")),
            ]),
            body: Vec::from("the body".as_bytes()),
        };

        let response_as_bytes = response_to_bytes(&response);
        let response_bytes_as_string = String::from_utf8_lossy(&response_as_bytes);

        assert!(
            response_bytes_as_string.eq("HTTP/1.1 200 OK\r\nContent-Type: hello\r\nConnection: bye\r\n\r\nthe body")
                || response_bytes_as_string.eq("HTTP/1.1 200 OK\r\nConnection: bye\r\nContent-Type: hello\r\n\r\nthe body")
        )
    }

    #[test]
    fn response_no_header_or_body_to_bytes() {
        let response = Response {
            status: &OK_200,
            headers: HashMap::new(),
            body: vec![],
        };
        assert_eq!(String::from_utf8_lossy(&response_to_bytes(&response)),
                   "HTTP/1.1 200 OK\r\n\r\n")
    }

    #[test]
    fn response_one_header_no_body_to_bytes() {
        let response = Response {
            status: &OK_200,
            headers: HeaderMapOps::from(vec![
                (Header::Custom(String::from("custom header")), String::from("header value"))
            ]),
            body: vec![],
        };
        assert_eq!(String::from_utf8_lossy(&response_to_bytes(&response)),
                   "HTTP/1.1 200 OK\r\ncustom header: header value\r\n\r\n")
    }
}