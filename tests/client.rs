use std::sync::Arc;
use std::thread::spawn;
use std::time::Duration;

use my_http::client::{Client, Config, RequestError};
use my_http::common::header::{Header, HeaderMapOps};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::status::OK_200;

#[test]
fn test_single_connection() {
    let client = Client::new(Config {
        addr: "www.google.com:80",
        read_timeout: Duration::from_secs(1),
        num_connections: 1,
    });

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HeaderMapOps::from(vec![
            (Header::Custom("accept-encoding".to_string()), "identity".to_string())
        ]),
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, OK_200);
    assert!(!response.body.is_empty());
}

#[test]
fn test_small_connection_pool() {
    test_connection_pool(13, 50);
}

#[test]
fn test_large_connection_pool() {
    test_connection_pool(100, 50);
}

fn test_connection_pool(num_connections: usize, connections_to_open: usize) {
    let client = Client::new(Config {
        addr: "www.google.com:80",
        read_timeout: Duration::from_secs(5),
        num_connections,
    });

    let client = Arc::new(client);

    let mut handlers = vec![];

    for _ in 0..connections_to_open {
        let client = Arc::clone(&client);
        let handler = spawn(move || {
            let response = client.send(&Request {
                uri: "/".to_string(),
                method: Method::GET,
                headers: HeaderMapOps::from(vec![
                    (Header::Custom("accept-encoding".to_string()), "identity".to_string())
                ]),
                body: vec![],
            }).unwrap();

            assert_eq!(response.status, OK_200);
            assert!(!response.body.is_empty());
        });

        handlers.push(handler);
    }

    for handler in handlers {
        handler.join().unwrap();
    }
}