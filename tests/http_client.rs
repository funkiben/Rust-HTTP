use std::collections::HashMap;
use std::time::Duration;

use my_http::client::{Client, Config};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::status;
use my_http::common::status::Status;
use my_http::header_map;

mod util;

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
    test_empty_requests("google.com:80", 13, 50, status::MOVED_PERMANENTLY, true);
}

#[test]
fn large_connection_pool() {
    test_empty_requests("google.com:80", 123, 50, status::MOVED_PERMANENTLY, true);
}

#[test]
#[ignore] // this test doesn't work on Github
fn many_websites_with_small_connection_pool() {
    test_empty_requests("northeastern.edu:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_empty_requests("reddit.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_empty_requests("facebook.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_empty_requests("instagram.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_empty_requests("twitter.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_empty_requests("wikipedia.org:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_empty_requests("youtube.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
    test_empty_requests("amazon.com:80", 13, 50, status::MOVED_PERMANENTLY, true);
    test_empty_requests("yahoo.com:80", 13, 50, status::MOVED_PERMANENTLY, true);
    test_empty_requests("apple.com:80", 13, 50, status::MOVED_PERMANENTLY, false);
}

fn test_empty_requests(addr: &'static str, num_connections: usize, requests: usize, expected_status: Status, should_have_body: bool) {
    let client = Client::new_http(Config {
        addr,
        read_timeout: Duration::from_millis(2000),
        num_connections,
    });

    util::test_client::test_empty_requests(client, requests, expected_status, should_have_body);
}