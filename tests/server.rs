extern crate my_http;

use std::net::TcpStream;
use std::thread::spawn;

use my_http::common::header::{CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::response::Response;
use my_http::common::status::{NOT_FOUND_404, OK_200, Status};
use my_http::server::Config;
use my_http::server::router::ListenerResult::SendResponse;
use my_http::server::Server;
use std::collections::HashMap;
use std::borrow::Cow;
use std::sync::Arc;

#[test]
fn test_multiple_concurrent_connections() {
    let mut server = Server::new(Config {
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Default::default()
    });

    let request1 = Request {
        uri: "/".to_string(),
        method: Method::Get,
        headers: Default::default(),
        body: vec![]
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
                (Header::Custom(String::from("custom-header")), "custom header value".to_string()),
            ]),
            body: b"welcome".to_vec(),
        })
    });

    // spawn(move || server.start());

    // TcpStream
    // client = TcpStream::connect("localhost:7878");
}