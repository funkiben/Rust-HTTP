use std::collections::HashMap;
use std::sync::Arc;
use std::thread::spawn;
use std::time::Duration;

use my_http::client::{Client, Config};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::status;
use my_http::common::status::Status;
use my_http::header_map;

#[test]
fn single_connection_google() {
    let client = Client::new_http(Config {
        addr: "google.com:80",
        read_timeout: Duration::from_secs(1),
        num_connections: 1,
    });

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());
}

#[test]
fn reuse_connection_google() {
    let client = Client::new_http(Config {
        addr: "google.com:80",
        read_timeout: Duration::from_secs(1),
        num_connections: 1,
    });

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());
}

#[test]
fn single_connection_northeastern() {
    let client = Client::new_http(Config {
        addr: "northeastern.edu:80",
        read_timeout: Duration::from_secs(1),
        num_connections: 1,
    });

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, status::MOVED_PERMANENTLY);
    assert!(response.body.is_empty());
}

#[test]
fn single_connection_reddit() {
    let client = Client::new_http(Config {
        addr: "reddit.com:80",
        read_timeout: Duration::from_secs(1),
        num_connections: 1,
    });

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: header_map![
            ("host", "reddit.com")
        ],
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, status::MOVED_PERMANENTLY);
    assert!(response.body.is_empty());
}

#[test]
fn small_connection_pool() {
    test_connection_pool("google.com:80", 13, 50, status::MOVED_PERMANENTLY, true);
}

#[test]
fn large_connection_pool() {
    test_connection_pool("google.com:80", 123, 50, status::MOVED_PERMANENTLY, true);
}

#[test]
fn many_websites_with_small_connection_pool() {
    test_connection_pool("northeastern.edu:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_connection_pool("reddit.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_connection_pool("facebook.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_connection_pool("instagram.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_connection_pool("twitter.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_connection_pool("wikipedia.org:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_connection_pool("youtube.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_connection_pool("amazon.com:80", 13, 50, status::MOVED_PERMANENTLY, true);
    test_connection_pool("yahoo.com:80", 13, 50, status::MOVED_PERMANENTLY, true);
    test_connection_pool("apple.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
}

fn test_connection_pool(addr: &'static str, num_connections: usize, requests: usize, expected_status: Status, should_have_body: bool) {
    println!("sending requests to {}", addr);
    let client = Client::new_http(Config {
        addr,
        read_timeout: Duration::from_secs(5),
        num_connections,
    });

    let website = &addr[..(addr.len() - 3)];

    let client = Arc::new(client);

    let mut handlers = vec![];

    for _ in 0..requests {
        let client = Arc::clone(&client);
        let handler = spawn(move || {
            let response = client.send(&Request {
                uri: "/".to_string(),
                method: Method::GET,
                headers: header_map![
                    ("host", website)
                ],
                body: vec![],
            }).unwrap();

            assert_eq!(response.status, expected_status);
            assert_eq!(should_have_body, !response.body.is_empty());
        });

        handlers.push(handler);
    }

    for handler in handlers {
        handler.join().unwrap();
    }
}