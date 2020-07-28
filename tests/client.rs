use std::sync::Arc;
use std::thread::spawn;
use std::time::Duration;

use my_http::common::header::{Header, HeaderMapOps, HeaderMap};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::status::OK_200;
use my_http::client::{Client, Config};
use std::collections::HashMap;

#[test]
fn single_connection_google() {
    let client = Client::new(Config {
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

    assert_eq!(response.status, OK_200);
    assert!(!response.body.is_empty());
}

#[test]
fn reuse_connection_google() {
    let client = Client::new(Config {
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

    assert_eq!(response.status, OK_200);
    assert!(!response.body.is_empty());

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, OK_200);
    assert!(!response.body.is_empty());


    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).unwrap();

    assert_eq!(response.status, OK_200);
    assert!(!response.body.is_empty());
}

#[test]
#[ignore]
fn single_connection_northeastern() {
    let client = Client::new(Config {
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

    println!("{}", String::from_utf8_lossy(&response.body));

    assert_eq!(response.status, OK_200);
    assert!(!response.body.is_empty());
}

#[test]
fn small_connection_pool() {
    test_connection_pool("google.com:80", 13, 50);
}

#[test]
fn large_connection_pool() {
    test_connection_pool("google.com:80", 123, 50);
}

#[test]
#[ignore]
fn many_websites_with_small_connection_pool() {
    test_connection_pool("www.northeastern.edu:80", 13, 50);
    test_connection_pool("www.reddit.com:80", 13, 50);
    test_connection_pool("www.stackoverflow.com:80", 13, 50);
    test_connection_pool("www.facebook.com:80", 13, 50);
    test_connection_pool("www.instagram.com:80", 13, 50);
    test_connection_pool("www.twitter.com:80", 13, 50);
}

fn test_connection_pool(addr: &'static str, num_connections: usize, requests: usize) {
    println!("Sending {} requests to {} over {} connections", requests, num_connections, addr);
    let client = Client::new(Config {
        addr,
        read_timeout: Duration::from_secs(5),
        num_connections,
    });

    let client = Arc::new(client);

    let mut handlers = vec![];

    for _ in 0..requests {
        let client = Arc::clone(&client);
        let handler = spawn(move || {
            let response = client.send(&Request {
                uri: "/".to_string(),
                method: Method::GET,
                headers: HeaderMap::from_pairs(vec![
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