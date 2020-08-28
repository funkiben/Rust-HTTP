use std::time::Duration;

use rustls::ClientConfig;

use my_http::client::{Client, Config};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::status;
use my_http::header_map;
use my_http::common::status::Status;

mod util;

#[test]
fn single_google_request() {
    let client = Client::new_https(
        Config {
            addr: "google.com:443",
            read_timeout: Duration::from_millis(500),
            num_connections: 5,
        },
        get_tls_config(),
    );

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: header_map![],
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());
}

#[test]
fn test_single_reddit_request() {
    let client = Client::new_https(
        Config {
            addr: "www.reddit.com:443",
            read_timeout: Duration::from_millis(5000),
            num_connections: 5,
        },
        get_tls_config(),
    );

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: header_map![
            ("host", "www.reddit.com")
        ],
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());
}

#[test]
fn test_single_northeastern_request() {
    let client = Client::new_https(
        Config {
            addr: "www.northeastern.edu:443",
            read_timeout: Duration::from_millis(1000),
            num_connections: 5,
        },
        get_tls_config(),
    );

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: header_map![
            ("host", "www.northeastern.edu")
        ],
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());
}

#[test]
fn small_connection_pool() {
    test_empty_requests("www.google.com:443", 13, 50, status::OK, true);
}

#[test]
fn large_connection_pool() {
    test_empty_requests("www.google.com:443", 123, 50, status::OK, true);
}

#[test]
fn many_websites_with_small_connection_pool() {
    test_empty_requests("www.northeastern.edu:443", 5, 13, status::OK, true);
    test_empty_requests("www.reddit.com:443", 5, 13, status::OK, true);
    test_empty_requests("www.wikipedia.org:443", 5, 13, status::OK, true);
    test_empty_requests("www.amazon.com:443", 5, 13, status::OK, true);
}

fn test_empty_requests(addr: &'static str, num_connections: usize, requests: usize, expected_status: Status, should_have_body: bool) {
    let client = Client::new_https(
        Config {
            addr,
            read_timeout: Duration::from_secs(5),
            num_connections,
        },
        get_tls_config(),
    );

    util::test_client::test_empty_requests(client, requests, expected_status, should_have_body);
}

fn get_tls_config() -> ClientConfig {
    let mut config = ClientConfig::new();
    config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    config
}