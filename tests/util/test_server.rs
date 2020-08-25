use std::io::Read;
use std::net::TcpStream;
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use my_http::client::write_request;
use my_http::common::request::Request;
use my_http::common::response::Response;
use my_http::server::{Config, Router, write_response};
use my_http::server;
use my_http::server::ListenerResult::{Next, SendResponseArc};

use crate::util::curl;

pub fn test_server(config: Config, num_connections: usize, num_loops_per_connection: usize, sleeps_between_requests: bool, messages: Vec<(Request, Response)>) {
    let addr = config.addr;
    start_server(config, &messages);

    let messages: Vec<(Request, Vec<u8>)> = messages.into_iter().map(|(req, res)| {
        let mut bytes: Vec<u8> = vec![];
        write_response(&mut bytes, &res).unwrap();
        (req, bytes)
    }).collect();

    let messages = Arc::new(messages);

    let mut handlers = vec![];
    for _ in 0..num_connections {
        let messages = Arc::clone(&messages);
        handlers.push(spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();

            for _ in 0..num_loops_per_connection {
                for (request, expected_response) in messages.iter() {
                    let mut actual_response = vec![0u8; expected_response.len()];

                    loop {
                        let result = write_request(&mut client, request)
                            .and_then(|_| client.read_exact(&mut actual_response));

                        if result.is_err() {
                            client = TcpStream::connect(addr).unwrap();
                            continue;
                        }

                        break;
                    }

                    assert_eq!(expected_response, &actual_response);

                    if sleeps_between_requests {
                        // sleep random fraction of a second
                        sleep(Duration::from_nanos(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos() as u64));
                    }
                }
            }
        }));
    }

    for handler in handlers {
        handler.join().unwrap();
    }
}

pub fn test_server_with_curl(config: Config, num_connections: usize, messages: Vec<(Request, Response)>, https: bool) {
    let addr = config.addr;

    start_server(config, &messages);

    let messages = Arc::new(messages);

    let mut handlers = vec![];
    for _ in 0..num_connections {
        let messages = Arc::clone(&messages);
        handlers.push(spawn(move || {
            let requests: Vec<&Request> = messages.iter().map(|(req, _)| req).collect();
            let expected_output: Vec<u8> = messages.iter().flat_map(|(_, res)| &res.body).map(|x| *x).collect();
            let expected_output = String::from_utf8_lossy(&expected_output).to_string();

            let actual_output = curl::requests(addr, &requests, https);
            assert_eq!(actual_output, expected_output);
        }));
    }

    for handler in handlers {
        handler.join().unwrap();
    }
}

fn start_server(mut server_config: Config, messages: &Vec<(Request, Response)>) {
    server_config.router = get_router(messages);

    spawn(|| server::start(server_config));
    sleep(Duration::from_millis(100));
}

fn get_router(messages: &Vec<(Request, Response)>) -> Router {
    let mut router = Router::new();

    for (request, response) in messages {
        let uri = &request.uri;
        let response = Arc::new(response.clone());
        let request = request.clone();
        router.on(uri, move |_, req|
            if request.eq(req) {
                SendResponseArc(response.clone())
            } else {
                Next
            },
        );
    }

    router
}