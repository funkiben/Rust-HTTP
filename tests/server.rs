extern crate my_http;

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::thread::{JoinHandle, sleep, spawn};
use std::time::Duration;

use my_http::common::header::{CONTENT_LENGTH, Header, HeaderMapOps};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::response::Response;
use my_http::common::status::{OK_200, Status};
use my_http::server::Config;
use my_http::server::router::ListenerResult::SendResponse;
use my_http::server::Server;

#[test]
fn test_multiple_concurrent_connections() {
    let mut server = Server::new(Config {
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(10000),
    });

    let request1 = Request {
        uri: "/".to_string(),
        method: Method::Get,
        headers: Default::default(),
        body: vec![],
    };

    let request2 = Request {
        uri: "/foo".to_string(),
        method: Method::Get,
        headers: HeaderMapOps::from(vec![
            (CONTENT_LENGTH, "5".to_string()),
            (Header::Custom(String::from("custom-header")), "custom header value".to_string()),
        ]),
        body: b"hello".to_vec(),
    };

    server.router.on("/", move |uri, request| {
        assert_eq!("/", uri);
        assert_eq!(*request, request1);
        SendResponse(Response {
            status: OK_200,
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
            headers: HeaderMapOps::from(vec![
                (CONTENT_LENGTH, "7".to_string()),
                (Header::Custom(String::from("custom-header-2")), "custom header value 2".to_string()),
            ]),
            body: b"welcome".to_vec(),
        })
    });

    spawn(move || server.start());

    sleep(Duration::from_millis(50));

    let mut handlers = vec![];
    for _ in 0..100 {
        handlers.push(spawn(|| {
            let mut client = TcpStream::connect("localhost:7878").unwrap();
            client.write(b"GET / HTTP/1.1\r\n\r\n").unwrap();
            client.flush().unwrap();

            let mut actual = [0u8; 19];
            client.read_exact(&mut actual).unwrap();

            assert_eq!(String::from_utf8_lossy(&actual), String::from_utf8_lossy(b"HTTP/1.1 200 OK\r\n\r\n"));

            client.write(b"GET /foo HTTP/1.1\r\ncontent-length: 5\r\ncustom-header: custom header value\r\n\r\nhello").unwrap();
            client.flush().unwrap();

            let mut actual = [0u8; 85];
            client.read(&mut actual).unwrap();

            assert!(String::from_utf8_lossy(&actual) == String::from_utf8_lossy(b"HTTP/1.1 234 hi\r\ncustom-header-2: custom header value 2\r\ncontent-length: 7\r\n\r\nwelcome")
                || String::from_utf8_lossy(&actual) == String::from_utf8_lossy(b"HTTP/1.1 234 hi\r\ncontent-length: 7\r\ncustom-header-2: custom header value 2\r\n\r\nwelcome"));

            client.shutdown(Shutdown::Both);

        }));
    }

    for handler in handlers {
        handler.join().unwrap()
    }
}