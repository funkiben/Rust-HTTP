use std::io::{BufRead, Read, Write};
use std::sync::{Arc, Mutex};

use mio::net::TcpStream;
use rustls::ServerSession;

use crate::common::header::{CONNECTION, HeaderMapOps};
use crate::common::HTTP_VERSION;
use crate::common::request::Request;
use crate::common::response::Response;
use crate::server::buf_stream::BufStream;
use crate::server::config::Config;
use crate::server::connection::{Connection, ReadRequestError};
use crate::server::connection::ReadRequestResult::{Closed, Error, NotReady, Ready};
use crate::server::poll::listen;
use crate::server::router::ListenerResult::{Next, SendResponse, SendResponseArc};
use crate::server::router::Router;
use crate::util::thread_pool::ThreadPool;
use crate::util::tls_stream::TlsStream;

/// Raw bytes for a request parsing error response.
const REQUEST_PARSING_ERROR_RESPONSE: &[u8; 28] = b"HTTP/1.1 400 Bad Request\r\n\r\n";

/// Raw bytes for a 404 not found response.
const NOT_FOUND_RESPONSE: &[u8; 26] = b"HTTP/1.1 404 Not Found\r\n\r\n";

/// Size of connection read buffers.
const READ_BUF_SIZE: usize = 4096;

/// Size of connection write buffers.
const WRITE_BUF_SIZE: usize = 2048;

/// Starts the HTTP server. This function will block and listen for new connections.
pub fn start(config: Config) -> std::io::Result<()> {
    let config = Arc::new(config);

    if let Some(tls_config) = &config.tls_config {
        listen_with(&config, |stream| TlsStream::new(ServerSession::new(tls_config), stream))
    } else {
        listen_with(&config, |stream| stream)
    }
}

/// Starts the server with the given config, and uses the given on_new_connection function to get streams for the incoming connections.
/// This abstraction is necessary since HTTP and HTTPS connections use different underlying streams.
fn listen_with<T: Read + Write + Send + 'static>(config: &Arc<Config>, on_new_connection: impl Fn(TcpStream) -> T) -> std::io::Result<()> {
    let addr = config.addr.parse().expect("Invalid socket address");
    let thread_pool = ThreadPool::new(config.connection_handler_threads);

    listen(addr,
           |socket, addr| {
               let stream = on_new_connection(socket);
               let connection = Connection::new(addr, BufStream::with_capacity(stream, READ_BUF_SIZE, WRITE_BUF_SIZE));
               Arc::new(Mutex::new(Some(connection)))
           },
           |connection| {
               let connection = connection.clone();
               let config = config.clone();
               thread_pool.execute(move || handle_read_ready_connection(config, connection));
           },
           |connection| {
               let connection = connection.clone();
               thread_pool.execute(move || handle_write_ready_connection(connection));
           })
}

/// Tries reading requests and responding for the given connection. May drop the given connection if it should be closed.
fn handle_read_ready_connection<T: BufRead + Write>(config: Arc<Config>, connection: Arc<Mutex<Option<Connection<T>>>>) {
    let mut lock = connection.lock().unwrap();

    if let Some(mut connection) = lock.take() {
        let should_close = respond_to_requests(&mut connection, &config.router);
        if !should_close {
            lock.replace(connection);
        }
    }
}

/// Tries reading requests and responding for the given connection. May drop the given connection if it should be closed.
fn handle_write_ready_connection<T: BufRead + Write>(connection: Arc<Mutex<Option<Connection<T>>>>) {
    let mut lock = connection.lock().unwrap();

    if let Some(mut connection) = lock.take() {
        if connection.flush().is_ok() {
            lock.replace(connection);
        }
    }
}

/// Responds to requests in the given connection using the given router. Returns true if the connection should be dropped.
fn respond_to_requests<T: BufRead + Write>(connection: &mut Connection<T>, router: &Router) -> bool {
    loop {
        match connection.read_request() {
            Ready(request) => {
                let write_result = write_response_from_router(connection, router, &request);
                if write_result.is_err() || should_close_after_response(&request) { return true; }
            }
            NotReady => return false,
            Closed => return true,
            Error(error) => {
                write_error_response(connection, error).unwrap_or_default();
                return true;
            }
        }
    }
}

/// Gets a response from the router and writes. If the router has no response, then writes a 404 response.
fn write_response_from_router(writer: &mut impl Write, router: &Router, request: &Request) -> std::io::Result<()> {
    match router.result(&request) {
        SendResponse(response) => write_response(writer, &response),
        SendResponseArc(response) => write_response(writer, &response),
        Next => writer.write_all(NOT_FOUND_RESPONSE).and_then(|_| writer.flush())
    }
}

/// Writes a response to the given request parsing error.
fn write_error_response(writer: &mut impl Write, error: ReadRequestError) -> std::io::Result<()> {
    println!("Error: {:?}", error);
    writer.write_all(REQUEST_PARSING_ERROR_RESPONSE)?;
    writer.flush()
}

/// Checks if the given connection should be closed after a response is sent to the given request.
fn should_close_after_response(request: &Request) -> bool {
    request.headers.contains_header_value(&CONNECTION, "close")
}

/// Writes the response as bytes to the given writer.
pub fn write_response(writer: &mut impl Write, response: &Response) -> std::io::Result<()> {
    // write! will call write multiple times and does not flush
    write!(writer, "{} {} {}\r\n", HTTP_VERSION, response.status.code, response.status.reason)?;
    for (header, values) in response.headers.iter() {
        for value in values {
            write!(writer, "{}: {}\r\n", header, value)?;
        }
    }
    writer.write(b"\r\n")?;
    writer.write(&response.body)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use crate::common::header::{CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, Header, HeaderMap, HeaderMapOps};
    use crate::common::method::Method;
    use crate::common::request::Request;
    use crate::common::response::Response;
    use crate::common::status;
    use crate::common::status::Status;
    use crate::server::buf_stream::BufStream;
    use crate::server::connection::Connection;
    use crate::server::router::ListenerResult::SendResponse;
    use crate::server::router::Router;
    use crate::server::server::{respond_to_requests, write_response};
    use crate::util::mock::{MockReader, MockStream, MockWriter};

    fn test_respond_to_requests(input: Vec<&str>, responses: Vec<Response>, expected_requests: Vec<Request>, expected_output: &str) {
        let reader = MockReader::from_strs(input);
        let writer = MockWriter::new();
        let stream = MockStream::new(reader, writer);

        let mut router = Router::new();

        let actual_requests = Arc::new(Mutex::new(vec![]));
        let responses = Arc::new(Mutex::new(responses));

        let actual_requests_clone = Arc::clone(&actual_requests);
        router.on_prefix("", move |_, request| {
            actual_requests_clone.lock().unwrap().push(request.clone());
            SendResponse(responses.lock().unwrap().remove(0))
        });

        let mut connection = Connection::new("0.0.0.0:80".parse().unwrap(), BufStream::new(stream));

        respond_to_requests(&mut connection, &router);

        let actual_output = connection.stream_ref().inner_ref().writer.flushed.concat();
        let actual_output = String::from_utf8(actual_output).unwrap();

        assert_eq!(expected_output, actual_output);
        assert_eq!(expected_requests, actual_requests.lock().unwrap().to_vec());
    }

    fn test_respond_to_requests_no_bad(input: Vec<&str>, expected_requests: Vec<Request>) {
        test_respond_to_requests_with_last_response(input, expected_requests, "");
    }

    fn test_respond_to_requests_with_last_response(input: Vec<&str>, expected_requests: Vec<Request>, last_response: &str) {
        let responses: Vec<Response> =
            (0..expected_requests.len())
                .map(|code| Response {
                    status: Status { code: code as u16, reason: "" },
                    headers: HashMap::new(),
                    body: vec![],
                })
                .collect();
        let mut expected_output: String = responses.iter().map(|res| {
            let mut buf: Vec<u8> = vec![];
            write_response(&mut buf, res).unwrap();
            String::from_utf8_lossy(&buf).into_owned()
        }).collect();
        expected_output.push_str(last_response);
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
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }])
    }

    #[test]
    fn one_request_fragmented() {
        test_respond_to_requests_no_bad(
            vec!["G", "ET / ", "HTTP/1", ".1\r\n", "\r", "\n"],
            vec![Request {
                uri: String::from("/"),
                method: Method::GET,
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
                method: Method::GET,
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
                method: Method::GET,
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
                method: Method::GET,
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
                    method: Method::GET,
                    headers: HeaderMap::new(),
                    body: vec![],
                },
                Request {
                    uri: String::from("/"),
                    method: Method::POST,
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
                method: Method::GET,
                headers: HeaderMap::from_pairs(vec![
                    (CONTENT_LENGTH, String::from("0")),
                    (CONNECTION, String::from("close")),
                    (Header::Custom(String::from("something")), String::from("hello there goodbye")),
                ]),
                body: vec![],
            }])
    }

    #[test]
    fn one_request_with_repeated_headers() {
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\ncontent-length: 0\r\ncontent-length: 0\r\nsomething: value 1\r\nsomething: value 2\r\n\r\n"],
            vec![Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::from_pairs(vec![
                    (CONTENT_LENGTH, String::from("0")),
                    (CONTENT_LENGTH, String::from("0")),
                    (Header::Custom(String::from("something")), String::from("value 1")),
                    (Header::Custom(String::from("something")), String::from("value 2")),
                ]),
                body: vec![],
            }])
    }

    #[test]
    fn one_request_with_headers_weird_case() {
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\ncoNtEnt-lEngtH: 0\r\nCoNNECTION: close\r\nsoMetHing: hello there goodbye\r\n\r\n"],
            vec![Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::from_pairs(vec![
                    (CONTENT_LENGTH, String::from("0")),
                    (CONNECTION, String::from("close")),
                    (Header::Custom(String::from("something")), String::from("hello there goodbye")),
                ]),
                body: vec![],
            }])
    }

    #[test]
    fn one_request_with_headers_only_colon_and_space() {
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\n: \r\n: \r\n\r\n"],
            vec![Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::from_pairs(vec![
                    (Header::Custom(String::from("")), String::from("")),
                    (Header::Custom(String::from("")), String::from("")),
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
                method: Method::GET,
                headers: HeaderMap::from_pairs(vec![
                    (CONTENT_LENGTH, String::from("5")),
                ]),
                body: body.to_vec(),
            }])
    }

    #[test]
    fn one_request_with_body_fragmented() {
        let body = b"hello";
        test_respond_to_requests_no_bad(
            vec!["GE", "T / ", "HTT", "P/1.", "1\r", "\nconte", "nt-le", "n", "gth: ", "5\r\n\r", "\nhe", "ll", "o"],
            vec![Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::from_pairs(vec![
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
                    method: Method::GET,
                    headers: HeaderMap::from_pairs(vec![
                        (CONTENT_LENGTH, String::from("5")),
                    ]),
                    body: body1.to_vec(),
                },
                Request {
                    uri: String::from("/body2"),
                    method: Method::GET,
                    headers: HeaderMap::from_pairs(vec![
                        (CONTENT_LENGTH, String::from("7")),
                    ]),
                    body: body2.to_vec(),
                }
            ],
        )
    }

    #[test]
    fn one_request_with_large_body() {
        let body = b"ergiergjhlisuehrlgisuehrlgisuehrlgiushelrgiushelriguheisurhgl ise\
        uhrg laiuwe````hrg ;aoiwhg aw4tyg 8o3w74go 8w475g\no 8w475hgo 8w475hgo 84w75hgo 8w347hfo g83qw7h4go\
         q837hgp 9q384h~~~gp 9qw\r\n385hgp q9384htpq9 38\r\nwuhf iwourehafgliweurhglaieruhgq9w348gh q9384ufhq\
         uerhgfq 934g\\hq934h|][;[.',,/.fg 9w`234145365uerhfg iawo!@#$$%#^$%&^$%^(&*^)(_)+_){P.;o\\/]'o;\n\n\r\n
         \r\n\n\r\n\r]/li][.                                                                       \
         \n\n\n\n\n\n\n\n\n     ^$%@#%!@%!@$%@#$^&%*&&^&()&)(|>wiuerghwiefujwouegowogjoe rijgoe rg\
         eriopgjeorgj eorgij woergij owgj 9348t9 348uqwtp 3874hg ow3489ghqp 9348ghf qp3498ugh pq\
         3q489g pq3498gf qp3948fh qp39ruhgwirughp9q34ughpq34u9gh pq3g\
         3q498g7 hq3o84g7h q3o847gh qp3948fh pq9wufhp q9w4hgpq9w47hgpq39wu4hfqw4ufhwq4\
         3q8974fh q3489fh qopw4389fhpq9w4ghqpw94ghpqw94ufghpw9fhupq9w4ghpqw94ghpq\
         woeifjoweifjowijfow ejf owijf ejf qefasfoP OJP JP JE FPIJEPF IWJEPFI JWPEF W\
         WEIOFJ WEFJ WPEIGJH 0348HG39 84GHJF039 84JF0394JF0 384G0348HGOWEIRGJPRGOJPE\
         WEIFOJ WEOFIJ PQIEGHQPIGH024UHG034IUHJG0WIUEJF0EIWJGF0WEGH 0WEGH W0IEJF PWIEJFG PWEF\
         W0EFJ 0WEFJ -WIJF-024JG0F34IGJ03 4I JG03W4IJG02HG0IQJGW-EIGJWPIEJGWeuf";
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\ncontent-length: 1131\r\n\r\n", &String::from_utf8_lossy(body)],
            vec![Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::from_pairs(vec![
                    (CONTENT_LENGTH, String::from("1131")),
                ]),
                body: body.to_vec(),
            }])
    }

    #[test]
    fn two_requests_connection_close_header() {
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\nconnection: close\r\n\r\n", "POST / HTTP/1.1\r\n\r\n"],
            vec![
                Request {
                    uri: String::from("/"),
                    method: Method::GET,
                    headers: HeaderMap::from_pairs(vec![(CONNECTION, String::from("close"))]),
                    body: vec![],
                }
            ])
    }


    #[test]
    fn header_with_multiple_colons() {
        test_respond_to_requests_no_bad(
            vec!["GET / HTTP/1.1\r\nhello: value: foo\r\n\r\n"],
            vec![
                Request {
                    uri: String::from("/"),
                    method: Method::GET,
                    headers: HeaderMap::from_pairs(vec![
                        (Header::Custom(String::from("hello")), String::from("value: foo"))
                    ]),
                    body: vec![],
                }
            ]);
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
                    method: Method::GET,
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
                    method: Method::GET,
                    headers: HeaderMap::from_pairs(vec![(CONTENT_LENGTH, String::from("3"))]),
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
    fn request_with_negative_content_length() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1\r\ncontent-length: -5\r\n\r\nhello"],
            vec![],
            "HTTP/1.1 400 Bad Request\r\n\r\n");
    }

    #[test]
    fn request_with_0_content_length() {
        test_respond_to_requests_with_last_response(
            vec!["GET / HTTP/1.1\r\ncontent-length: 0\r\n\r\nhello"],
            vec![
                Request {
                    uri: String::from("/"),
                    method: Method::GET,
                    headers: HeaderMap::from_pairs(vec![(CONTENT_LENGTH, String::from("0"))]),
                    body: vec![],
                }],
            "HTTP/1.1 400 Bad Request\r\n\r\n");
    }

    #[test]
    fn write_response_with_headers_and_body() {
        let response = Response {
            status: status::OK,
            headers: HeaderMap::from_pairs(vec![
                (CONTENT_TYPE, String::from("hello")),
                (CONNECTION, String::from("bye")),
            ]),
            body: Vec::from("the body".as_bytes()),
        };

        let mut writer = MockWriter::new();

        write_response(&mut writer, &response).unwrap();

        let bytes = writer.flushed.concat();
        let response_bytes_as_string = String::from_utf8_lossy(&bytes);

        assert!(
            response_bytes_as_string.eq("HTTP/1.1 200 OK\r\ncontent-type: hello\r\nconnection: bye\r\n\r\nthe body")
                || response_bytes_as_string.eq("HTTP/1.1 200 OK\r\nconnection: bye\r\ncontent-type: hello\r\n\r\nthe body")
        )
    }

    #[test]
    fn response_no_header_or_body_to_bytes() {
        let response = Response {
            status: status::OK,
            headers: HashMap::new(),
            body: vec![],
        };
        let mut buf: Vec<u8> = vec![];
        write_response(&mut buf, &response).unwrap();
        assert_eq!(String::from_utf8_lossy(&buf), "HTTP/1.1 200 OK\r\n\r\n")
    }

    #[test]
    fn response_one_header_no_body_to_bytes() {
        let response = Response {
            status: status::OK,
            headers: HeaderMap::from_pairs(vec![
                (Header::Custom(String::from("custom header")), String::from("header value"))
            ]),
            body: vec![],
        };
        let mut buf: Vec<u8> = vec![];
        write_response(&mut buf, &response).unwrap();
        assert_eq!(String::from_utf8_lossy(&buf), "HTTP/1.1 200 OK\r\ncustom header: header value\r\n\r\n")
    }
}
