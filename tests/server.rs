extern crate my_http;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread::{sleep, spawn};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use my_http::common::header::{CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::response::Response;
use my_http::common::status;
use my_http::common::status::Status;
use my_http::server::Config;
use my_http::server::ListenerResult::SendResponse;
use my_http::server::Server;

#[test]
fn multiple_concurrent_connections_with_many_requests() {
    let mut server = Server::new(Config {
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    let request1 = Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: Default::default(),
        body: vec![],
    };

    let request2 = Request {
        uri: "/foo".to_string(),
        method: Method::GET,
        headers: HeaderMap::from_pairs(vec![
            (CONTENT_LENGTH, "5".to_string()),
            (Header::Custom(String::from("custom-header")), "custom header value".to_string()),
        ]),
        body: b"hello".to_vec(),
    };

    server.router.on("/", move |uri, request| {
        assert_eq!("/", uri);
        assert_eq!(*request, request1);
        SendResponse(Response {
            status: status::OK,
            headers: Default::default(),
            body: vec![],
        })
    });

    server.router.on("/foo", move |uri, request| {
        assert_eq!("/foo", uri);
        assert_eq!(*request, request2);
        SendResponse(Response {
            status: Status {
                code: 234,
                reason: "hi",
            },
            headers: HeaderMap::from_pairs(vec![
                (CONTENT_LENGTH, "7".to_string()),
                (Header::Custom(String::from("custom-header-2")), "custom header value 2".to_string()),
            ]),
            body: b"welcome".to_vec(),
        })
    });

    spawn(move || server.start());

    sleep(Duration::from_millis(1000));

    let mut handlers = vec![];
    for _ in 0..13 {
        handlers.push(spawn(|| {

            let mut client = TcpStream::connect("localhost:7878").unwrap();

            for i in 0..11 {
                client.write(b"GET / HTTP/1.1\r\n\r\n").unwrap();

                let mut actual = [0u8; 19];
                if let Err(_) = client.read_exact(&mut actual) {
                    client = TcpStream::connect("localhost:7878").unwrap();
                    continue;
                }

                assert_eq!(String::from_utf8_lossy(&actual), String::from_utf8_lossy(b"HTTP/1.1 200 OK\r\n\r\n"));

                client.write(b"GET /foo HTTP/1.1\r\ncontent-length: 5\r\ncustom-header: custom header value\r\n\r\nhello").unwrap();

                let mut actual = [0u8; 85];
                client.read(&mut actual).unwrap();

                assert!(String::from_utf8_lossy(&actual) == String::from_utf8_lossy(b"HTTP/1.1 234 hi\r\ncustom-header-2: custom header value 2\r\ncontent-length: 7\r\n\r\nwelcome")
                    || String::from_utf8_lossy(&actual) == String::from_utf8_lossy(b"HTTP/1.1 234 hi\r\ncontent-length: 7\r\ncustom-header-2: custom header value 2\r\n\r\nwelcome"));

                if i < 14 {
                    sleep(Duration::from_nanos(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos() as u64));
                }
            }
        }));
    }

    for handler in handlers {
        handler.join().unwrap()
    }
}

#[test]
fn infinite_connection() {
    let server = Server::new(Config {
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7878").unwrap();

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
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7878").unwrap();

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
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7878").unwrap();

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
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7878").unwrap();

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
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: None,
    });

    spawn(|| {
        server.start().unwrap();
    });

    sleep(Duration::from_millis(500));

    let mut client = TcpStream::connect("localhost:7878").unwrap();

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