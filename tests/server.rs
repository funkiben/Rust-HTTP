extern crate my_http;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use my_http::client::write_request;
use my_http::common::header::{CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::response::Response;
use my_http::common::status;
use my_http::common::status::Status;
use my_http::server::{Config, write_response};
use my_http::server::ListenerResult::SendResponse;
use my_http::server::Server;

#[test]
fn multiple_concurrent_connections_with_many_requests() {
    stress_test(
        Config {
            addr: "localhost:7000",
            connection_handler_threads: 5,
            read_timeout: Duration::from_millis(500),
            tls_config: None,
        },
        13, 11,
        vec![
            (
                Request {
                    uri: "/".to_string(),
                    method: Method::GET,
                    headers: Default::default(),
                    body: vec![],
                },
                Response {
                    status: status::OK,
                    headers: Default::default(),
                    body: vec![],
                }
            ), (
                Request {
                    uri: "/foo".to_string(),
                    method: Method::GET,
                    headers: HeaderMap::from_pairs(vec![
                        (CONTENT_LENGTH, "5".to_string()),
                        (Header::Custom(String::from("custom-header")), "custom header value".to_string()),
                    ]),
                    body: b"hello".to_vec(),
                },
                Response {
                    status: Status {
                        code: 234,
                        reason: "hi",
                    },
                    headers: HeaderMap::from_pairs(vec![
                        (CONTENT_LENGTH, "7".to_string()),
                        (Header::Custom(String::from("custom-header-2")), "custom header value 2".to_string()),
                    ]),
                    body: b"welcome".to_vec(),
                }
            )
        ])
}

#[test]
fn many_concurrent_connections_with_one_simple_request() {
    stress_test(
        Config {
            addr: "localhost:7006",
            connection_handler_threads: 5,
            read_timeout: Duration::from_millis(500),
            tls_config: None,
        },
        200, 1,
        vec![
            (
                Request {
                    uri: "/".to_string(),
                    method: Method::GET,
                    headers: Default::default(),
                    body: vec![],
                },
                Response {
                    status: status::OK,
                    headers: Default::default(),
                    body: vec![],
                }
            )
        ])
}

#[test]
fn many_concurrent_connections_with_many_simple_requests() {
    stress_test(
        Config {
            addr: "localhost:7006",
            connection_handler_threads: 5,
            read_timeout: Duration::from_millis(500),
            tls_config: None,
        },
        10, 10,
        vec![
            (
                Request {
                    uri: "/".to_string(),
                    method: Method::GET,
                    headers: Default::default(),
                    body: vec![],
                },
                Response {
                    status: status::OK,
                    headers: Default::default(),
                    body: vec![],
                }
            )
        ])
}

#[test]
fn infinite_connection() {
    let server = Server::new(Config {
        addr: "localhost:7001",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7001").unwrap();

    loop {
        if let Err(_) = client.write(b"blah") {
            break;
        }
    }

    let mut response = String::new();
    let read_result = client.read_to_string(&mut response);

    assert!(read_result.is_err());
    assert_eq!("HTTP/1.1 400 Bad Request\r\n\r\n", response);
}

#[test]
fn infinite_headers() {
    let server = Server::new(Config {
        addr: "localhost:7002",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7002").unwrap();

    client.write(b"GET / HTTP/1.1\r\n").unwrap();

    loop {
        if let Err(_) = client.write(b"random: value\r\n") {
            break;
        }
    }

    let mut response = String::new();
    let read_result = client.read_to_string(&mut response);

    assert!(read_result.is_err());
    assert_eq!("HTTP/1.1 400 Bad Request\r\n\r\n", response);
}

#[test]
fn infinite_header_value() {
    let server = Server::new(Config {
        addr: "localhost:7003",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7003").unwrap();

    client.write(b"GET / HTTP/1.1\r\nheader: ").unwrap();

    loop {
        if let Err(_) = client.write(b"blah\r\n") {
            break;
        }
    }

    let mut response = String::new();
    let read_result = client.read_to_string(&mut response);

    assert!(read_result.is_err());
    assert_eq!("HTTP/1.1 400 Bad Request\r\n\r\n", response);
}

#[test]
fn infinite_chunked_body() {
    let server = Server::new(Config {
        addr: "localhost:7004",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7004").unwrap();

    client.write(b"GET / HTTP/1.1\r\ntransfer-encoding: chunked\r\n\r\n").unwrap();

    loop {
        if let Err(_) = client.write(b"5\r\nhello\r\n") {
            break;
        }
    }

    let mut response = String::new();
    let read_result = client.read_to_string(&mut response);

    assert!(read_result.is_err());
    assert_eq!("HTTP/1.1 400 Bad Request\r\n\r\n", response);
}

#[test]
fn insanely_huge_body() {
    let server = Server::new(Config {
        addr: "localhost:7005",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7005").unwrap();

    client.write(b"GET / HTTP/1.1\r\ncontent-length: 99999999\r\n\r\n").unwrap();

    loop {
        if let Err(_) = client.write(b"blah") {
            break;
        }
    }

    let mut response = String::new();
    let read_result = client.read_to_string(&mut response);

    assert!(read_result.is_err());
    assert_eq!("HTTP/1.1 400 Bad Request\r\n\r\n", response);
}

fn stress_test(server_config: Config, num_connections: usize, num_loops_per_connection: usize, messages: Vec<(Request, Response)>) {
    let addr = server_config.addr;
    let mut server = Server::new(server_config);

    for (request, response) in messages.iter() {
        let uri = &request.uri;
        let response = response.clone();
        let request = request.clone();
        server.router.on(uri, move |_, req| {
            assert_eq!(request, *req);
            SendResponse(response.clone())
        });
    }

    let messages: Vec<(Request, Vec<u8>)> = messages.into_iter().map(|(req, res)| {
        let mut bytes: Vec<u8> = vec![];
        write_response(&mut bytes, &res).unwrap();
        (req, bytes)
    }).collect();

    let messages = Arc::new(messages);

    spawn(|| server.start());
    sleep(Duration::from_millis(100));

    let mut handlers = vec![];
    for _ in 0..num_connections {
        let messages = Arc::clone(&messages);
        handlers.push(spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();

            for _ in 0..num_loops_per_connection {
                for (request, expected_response) in messages.iter() {
                    let mut actual_response = vec![0u8; expected_response.len()];

                    loop {
                        let result = write_request(&mut client, request)
                            .and_then(|_| client.read_exact(&mut actual_response));

                        if result.is_err() {
                            client = TcpStream::connect(addr).unwrap();
                            continue;
                        }

                        break;
                    }

                    assert_eq!(expected_response, &actual_response);

                    // sleep random fraction of a second
                    sleep(Duration::from_nanos(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos() as u64));
                }
            }
        }));
    }

    for handler in handlers {
        handler.join().unwrap();
    }
}