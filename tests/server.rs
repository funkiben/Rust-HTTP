extern crate my_http;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread::{sleep, spawn};
use std::time::{Duration};

use my_http::common::header::{ACCEPT, ACCEPT_CHARSET, ACCEPT_ENCODING, ACCEPT_LANGUAGE, ACCEPT_RANGES, CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::response::Response;
use my_http::common::status;
use my_http::common::status::Status;
use my_http::header_map;
use my_http::server::{Config};
use my_http::server::Server;
use crate::util::test_server::test_server;

mod util;

#[test]
fn many_requests_with_short_headers_and_short_bodies() {
    test_server(
        Config {
            addr: "localhost:7000",
            connection_handler_threads: 5,
            read_timeout: Duration::from_millis(500),
            tls_config: None,
        },
        13, 11, true,
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
fn many_connections_and_many_large_messages() {
    let test_html = std::fs::read("./tests/files/test.html").unwrap();
    let test_jpg = std::fs::read("./tests/files/test.jpg").unwrap();
    test_server(
        Config {
            addr: "localhost:7001",
            connection_handler_threads: 5,
            read_timeout: Duration::from_millis(500),
            tls_config: None,
        },
        15, 15, true,
        vec![
            (
                Request {
                    uri: "/hello/world/html".to_string(),
                    method: Method::GET,
                    headers: header_map![
                        (CONTENT_LENGTH, test_jpg.len().to_string()),
                        ("custom-header", "custom header value"),
                        ("custom-header", "custom header value2"),
                        ("custom-header", "custom header value3"),
                        ("custom-header", "custom header value4"),
                        ("custom-header", "custom header value5"),
                        ("custom-header", "custom header value6"),
                        ("custom-header", "custom header value7"),
                        ("custom-header", "custom header value8"),
                        ("custom-header", "custom header value9"),
                        ("custom-header", "custom header value10"),
                        ("custom-header", "custom header value11"),
                        ("accept", "blah blah blah"),
                        ("hello", "bye"),
                        ("bye", "hello"),
                        ("heyy", "foijr ewoi fjeigruh jseliurgh seliug he fowiuejf oweifj oweijfow "),
                        ("host", "yahayah"),
                        ("date", "rwgwrfwef"),
                        ("time", "freg esrg erg"),
                        ("expect", "freg esrg iofj wioefj pweijfo weijfp qwiefj pqeifjperg"),
                        ("expires", "freg esrgeo urghj oeuirhgj oeiwjrgp wiejf pweifj pweijfpwrg erg"),
                        ("forwarded", "freg esrg erg"),
                    ],
                    body: test_jpg,
                },
                Response {
                    status: Status {
                        code: 505,
                        reason: "helloooo",
                    },
                    headers: header_map![
                        (CONTENT_LENGTH, test_html.len().to_string()),
                        (ACCEPT, "blah blah blah"),
                        (ACCEPT_CHARSET, "blah blah blah"),
                        (ACCEPT_ENCODING, "blah blah blah efwi jwef wef "),
                        (ACCEPT_LANGUAGE, "blah blah blah"),
                        (ACCEPT_RANGES, "blah blwef wefpoi wjefi wjepf wah blah"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 1"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 2"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 3"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 4"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 5"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 6"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 7"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 8"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 9"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 10"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 11"),
                        ("hello", "blah blwef wefpoi wjefi wjepf wah blah 12"),
                    ],
                    body: test_html
                }
            )
        ])
}

#[test]
fn many_connections_with_one_simple_request() {
    test_server(
        Config {
            addr: "localhost:7002",
            connection_handler_threads: 5,
            read_timeout: Duration::from_millis(500),
            tls_config: None,
        },
        200, 1, false,
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
fn many_connections_with_many_simple_requests() {
    test_server(
        Config {
            addr: "localhost:7003",
            connection_handler_threads: 5,
            read_timeout: Duration::from_millis(500),
            tls_config: None,
        },
        10, 10, true,
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
fn many_concurrent_connections_with_many_simple_requests_no_delay() {
    test_server(
        Config {
            addr: "localhost:7004",
            connection_handler_threads: 5,
            read_timeout: Duration::from_millis(500),
            tls_config: None,
        },
        10, 10, false,
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
        addr: "localhost:7006",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7006").unwrap();

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
        addr: "localhost:7007",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7007").unwrap();

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
        addr: "localhost:7008",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7008").unwrap();

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
        addr: "localhost:7009",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7009").unwrap();

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

