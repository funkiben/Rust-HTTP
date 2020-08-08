use std::fs;
use std::io::{BufReader, Write, Read};
use std::process::Command;
use std::thread::{sleep, spawn};
use std::time::Duration;

use rustls::{Certificate, NoClientAuth, PrivateKey, ServerConfig};

use my_http::common::header::CONTENT_LENGTH;
use my_http::common::response::Response;
use my_http::common::status;
use my_http::header_map;
use my_http::server::{Config, Server};
use my_http::server::ListenerResult::SendResponse;
use std::net::TcpStream;

#[test]
fn curl_request() {
    let mut server = Server::new(Config {
        addr: "localhost:8000",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(10000),
        tls_config: Some(get_tsl_config()),
    });

    server.router.on_prefix("/", |_, _| {
        SendResponse(Response {
            status: status::OK,
            headers: header_map![(CONTENT_LENGTH, "6")],
            body: "i work".as_bytes().to_vec(),
        })
    });

    spawn(|| server.start());

    sleep(Duration::from_millis(1000));

    let output = Command::new("curl")
        .arg("-k")
        .arg("--request").arg("GET").arg("https://localhost:8000")
        .output().unwrap();

    assert_eq!("i work", String::from_utf8_lossy(&output.stdout));
}

#[test]
fn curl_multiple_requests_same_connection() {
    let mut server = Server::new(Config {
        addr: "localhost:8001",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(10000),
        tls_config: Some(get_tsl_config()),
    });

    server.router.on_prefix("/", |_, _| {
        SendResponse(Response {
            status: status::OK,
            headers: header_map![(CONTENT_LENGTH, "6")],
            body: "i work".as_bytes().to_vec(),
        })
    });

    spawn(|| server.start());

    sleep(Duration::from_millis(1000));

    let output = Command::new("curl")
        .arg("-k")
        .arg("--request").arg("GET").arg("https://localhost:8001")
        .arg("--request").arg("GET").arg("https://localhost:8001")
        .arg("--request").arg("GET").arg("https://localhost:8001")
        .arg("--request").arg("GET").arg("https://localhost:8001")
        .arg("--request").arg("GET").arg("https://localhost:8001")
        .arg("--request").arg("GET").arg("https://localhost:8001")
        .output().unwrap();

    assert_eq!("i worki worki worki worki worki work", String::from_utf8_lossy(&output.stdout));
}

#[test]
fn curl_multiple_concurrent_connections_with_many_requests() {
    let mut server = Server::new(Config {
        addr: "localhost:8002",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(10000),
        tls_config: Some(get_tsl_config()),
    });

    server.router.on_prefix("/", |_, _| {
        SendResponse(Response {
            status: status::OK,
            headers: header_map![(CONTENT_LENGTH, "6")],
            body: "i work".as_bytes().to_vec(),
        })
    });

    spawn(|| server.start());

    sleep(Duration::from_millis(1000));

    let mut handlers = vec![];
    for _ in 0..20 {
        handlers.push(spawn(|| {
            let output = Command::new("curl")
                .arg("-k")
                .arg("--request").arg("GET").arg("https://localhost:8002")
                .arg("--request").arg("GET").arg("https://localhost:8002")
                .arg("--request").arg("GET").arg("https://localhost:8002")
                .arg("--request").arg("GET").arg("https://localhost:8002")
                .arg("--request").arg("GET").arg("https://localhost:8002")
                .arg("--request").arg("GET").arg("https://localhost:8002")
                .arg("--request").arg("GET").arg("https://localhost:8002")
                .arg("--request").arg("GET").arg("https://localhost:8002")
                .output().unwrap();
            assert_eq!("i worki worki worki worki worki worki worki work", String::from_utf8_lossy(&output.stdout));
        }));
    }

    for handler in handlers {
        handler.join().unwrap();
    }
}

#[test]
fn curl_multiple_concurrent_connections_with_single_requests() {
    let mut server = Server::new(Config {
        addr: "localhost:8005",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(10000),
        tls_config: Some(get_tsl_config()),
    });

    server.router.on_prefix("/", |_, _| {
        SendResponse(Response {
            status: status::OK,
            headers: header_map![(CONTENT_LENGTH, "6")],
            body: "i work".as_bytes().to_vec(),
        })
    });

    spawn(|| server.start());

    sleep(Duration::from_millis(1000));

    let mut handlers = vec![];
    for _ in 0..200 {
        handlers.push(spawn(|| {
            let output = Command::new("curl")
                .arg("-k")
                .arg("--request").arg("GET").arg("https://localhost:8005")
                .output().unwrap();
            assert_eq!("i work", String::from_utf8_lossy(&output.stdout));
        }));
    }

    for handler in handlers {
        handler.join().unwrap();
    }
}

#[test]
fn infinite_connection() {
    let server = Server::new(Config {
        addr: "localhost:8003",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(500),
        tls_config: Some(get_tsl_config()),
    });

    spawn(|| server.start());

    sleep(Duration::from_millis(1000));

    let mut client = TcpStream::connect("localhost:8003").unwrap();

    loop {
        if let Err(_) = client.write_all(b"blahblahblah") {
            break;
        }
    }
}

#[test]
fn normal_http_message() {
    let server = Server::new(Config {
        addr: "localhost:8004",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(1000),
        tls_config: Some(get_tsl_config()),
    });

    spawn(|| server.start());

    sleep(Duration::from_millis(1000));

    let mut client = TcpStream::connect("localhost:8004").unwrap();

    client.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();

    let mut response = String::new();

    client.read_to_string(&mut response).unwrap();

}

fn get_tsl_config() -> ServerConfig {
    let mut config = ServerConfig::new(NoClientAuth::new());

    let certs = read_certs("./tests/certs/server.crt");
    let privkey = read_private_key("./tests/certs/server.key");

    config.set_single_cert(certs, privkey).unwrap();

    config
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