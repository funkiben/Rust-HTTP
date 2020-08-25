use std::fs;
use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;

use rustls::{Certificate, NoClientAuth, PrivateKey, ServerConfig};

use my_http::{header_map, server};
use my_http::common::header::CONTENT_LENGTH;
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::response::Response;
use my_http::common::status;
use my_http::server::{Config, Router};
use my_http::server::ListenerResult::SendResponse;

use crate::util::curl;
use crate::util::test_server::test_server_with_curl;

mod util;

#[test]
fn curl_request() {
    let mut router = Router::new();

    router.on_prefix("/", |_, _| {
        SendResponse(Response {
            status: status::OK,
            headers: header_map![(CONTENT_LENGTH, "6")],
            body: "i work".as_bytes().to_vec(),
        })
    });

    spawn(|| server::start(Config {
        addr: "0.0.0.0:8000",
        connection_handler_threads: 5,
        tls_config: Some(get_tsl_config()),
        router,
    }));

    sleep(Duration::from_millis(1000));

    let output = curl::request("0.0.0.0:8000", &Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: header_map![],
        body: vec![],
    }, true);

    assert_eq!("i work", output);
}

#[test]
fn curl_multiple_requests_same_connection() {
    let mut router = Router::new();

    router.on_prefix("/", |_, _| {
        SendResponse(Response {
            status: status::OK,
            headers: header_map![(CONTENT_LENGTH, "6")],
            body: "i work".as_bytes().to_vec(),
        })
    });

    spawn(|| server::start(Config {
        addr: "0.0.0.0:8001",
        connection_handler_threads: 5,
        tls_config: Some(get_tsl_config()),
        router,
    }));

    sleep(Duration::from_millis(1000));

    let output = curl::requests(
        "0.0.0.0:8001",
        &vec![&Request {
            uri: "/".to_string(),
            method: Method::GET,
            headers: header_map![],
            body: vec![],
        }; 6],
        true);

    assert_eq!("i worki worki worki worki worki work", output);
}

#[test]
fn curl_multiple_concurrent_connections_with_many_requests() {
    test_server_with_curl(
        Config {
            addr: "0.0.0.0:8002",
            connection_handler_threads: 5,
            tls_config: Some(get_tsl_config()),
            router: Router::new(),
        },
        50,
        vec![(
                 Request {
                     uri: "/".to_string(),
                     method: Method::GET,
                     headers: header_map![
                        ("content-length", "10"),
                        ("random", "blah"),
                        ("hello", "bye")
                     ],
                     body: b"0123456789".to_vec(),
                 },
                 Response {
                     status: status::OK,
                     headers: header_map![(CONTENT_LENGTH, "6")],
                     body: "i work".as_bytes().to_vec(),
                 }
             ); 10], true);
}

#[test]
fn curl_multiple_concurrent_connections_with_single_requests() {
    test_server_with_curl(
        Config {
            addr: "0.0.0.0:8005",
            connection_handler_threads: 5,
            tls_config: Some(get_tsl_config()),
            router: Router::new(),
        },
        200,
        vec![(
            Request {
                uri: "/".to_string(),
                method: Method::GET,
                headers: header_map![],
                body: vec![],
            },
            Response {
                status: status::OK,
                headers: header_map![(CONTENT_LENGTH, "6")],
                body: "i work".as_bytes().to_vec(),
            }
        )], true);
}

#[test]
fn infinite_connection() {
    spawn(|| server::start(Config {
        addr: "0.0.0.0:8003",
        connection_handler_threads: 5,
        tls_config: Some(get_tsl_config()),
        router: Router::new(),
    }).unwrap());

    sleep(Duration::from_millis(1000));

    let mut client = TcpStream::connect("0.0.0.0:8003").unwrap();

    loop {
        if let Err(_) = client.write_all(b"blahblahblah") {
            break;
        }
    }
}

#[test]
fn normal_http_message() {
    spawn(|| server::start(Config {
        addr: "0.0.0.0:8004",
        connection_handler_threads: 5,
        tls_config: Some(get_tsl_config()),
        router: Router::new(),
    }));

    sleep(Duration::from_millis(1000));

    let mut client = TcpStream::connect("0.0.0.0:8004").unwrap();

    client.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();

    let mut response = String::new();

    client.read_to_string(&mut response).unwrap();
}

#[test]
fn curl_many_big_responses_through_concurrent_connections() {
    let file_data = fs::read("./tests/files/big_image.jpg").unwrap();

    test_server_with_curl(
        Config {
            addr: "0.0.0.0:8006",
            connection_handler_threads: 5,
            tls_config: Some(get_tsl_config()),
            router: Router::new(),
        },
        10,
        vec![(
                 Request {
                     uri: "/".to_string(),
                     method: Method::GET,
                     headers: Default::default(),
                     body: vec![],
                 },
                 Response {
                     status: status::OK,
                     headers: header_map![(CONTENT_LENGTH, file_data.len().to_string())],
                     body: file_data,
                 }
             ); 3],
        true)
}

fn get_tsl_config() -> Arc<ServerConfig> {
    let mut config = ServerConfig::new(NoClientAuth::new());

    let certs = read_certs("./tests/certs/server.crt");
    let privkey = read_private_key("./tests/certs/server.key");

    config.set_single_cert(certs, privkey).unwrap();

    Arc::new(config)
}

fn read_certs(filename: &str) -> Vec<Certificate> {
    let certfile = fs::File::open(filename).expect("cannot open certificate file");
    let mut reader = BufReader::new(certfile);
    rustls::internal::pemfile::certs(&mut reader).unwrap()
}

fn read_private_key(filename: &str) -> PrivateKey {
    let rsa_keys = {
        let keyfile = fs::File::open(filename)
            .expect("cannot open private key file");
        let mut reader = BufReader::new(keyfile);
        rustls::internal::pemfile::rsa_private_keys(&mut reader)
            .expect("file contains invalid rsa private key")
    };

    let pkcs8_keys = {
        let keyfile = fs::File::open(filename)
            .expect("cannot open private key file");
        let mut reader = BufReader::new(keyfile);
        rustls::internal::pemfile::pkcs8_private_keys(&mut reader)
            .expect("file contains invalid pkcs8 private key (encrypted keys not supported)")
    };

    // prefer to load pkcs8 keys
    if !pkcs8_keys.is_empty() {
        pkcs8_keys[0].clone()
    } else {
        assert!(!rsa_keys.is_empty());
        rsa_keys[0].clone()
    }
}