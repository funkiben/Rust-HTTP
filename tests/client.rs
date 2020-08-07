use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use my_http::client::{Client, Config};
use my_http::common::header::{Header, HeaderMap, HeaderMapOps};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::status;

#[tokio::test]
async fn single_connection_google() {
    let client = Client::new(Config {
        addr: "google.com:80".to_socket_addrs().unwrap().next().unwrap(),
        num_connections: 1,
    });

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).await.unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());
}

#[tokio::test]
async fn reuse_connection_google() {
    let client = Client::new(Config {
        addr: "google.com:80".to_socket_addrs().unwrap().next().unwrap(),
        num_connections: 1,
    });

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).await.unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).await.unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());


    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).await.unwrap();

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());
}

#[tokio::test]
#[ignore]
async fn single_connection_northeastern() {
    let client = Client::new(Config {
        addr: "northeastern.edu:80".to_socket_addrs().unwrap().next().unwrap(),
        num_connections: 1,
    });

    let response = client.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HashMap::new(),
        body: vec![],
    }).await.unwrap();

    println!("{}", String::from_utf8_lossy(&response.body));

    assert_eq!(response.status, status::OK);
    assert!(!response.body.is_empty());
}

#[tokio::test]
async fn small_connection_pool() {
    test_connection_pool("google.com:80", 13, 50).await;
}

#[tokio::test]
async fn large_connection_pool() {
    test_connection_pool("google.com:80", 123, 50).await;
}

#[tokio::test]
#[ignore]
async fn many_websites_with_small_connection_pool() {
    test_connection_pool("www.northeastern.edu:80", 13, 50).await;
    test_connection_pool("www.reddit.com:80", 13, 50).await;
    test_connection_pool("www.stackoverflow.com:80", 13, 50).await;
    test_connection_pool("www.facebook.com:80", 13, 50).await;
    test_connection_pool("www.instagram.com:80", 13, 50).await;
    test_connection_pool("www.twitter.com:80", 13, 50).await;
}

async fn test_connection_pool(addr: &'static str, num_connections: usize, requests: usize) {
    println!("Sending {} requests to {} over {} connections", requests, num_connections, addr);
    let client = Client::new(Config {
        addr: addr.to_socket_addrs().unwrap().next().unwrap(),
        num_connections,
    });

    let client = Arc::new(client);

    let mut handlers = vec![];

    for _ in 0..requests {
        let client = Arc::clone(&client);
        let handler = tokio::spawn(async move {
            let response = client.send(&Request {
                uri: "/".to_string(),
                method: Method::GET,
                headers: HeaderMap::from_pairs(vec![
                    (Header::Custom("accept-encoding".to_string()), "identity".to_string())
                ]),
                body: vec![],
            }).await.unwrap();

            assert_eq!(response.status, status::OK);
            assert!(!response.body.is_empty());
        });
        handlers.push(handler);
    }

    for handler in handlers {
        handler.await.unwrap();
    }
}