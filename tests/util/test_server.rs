use my_http::server::{Config, Server, write_response};
use my_http::common::request::Request;
use my_http::common::response::Response;
use my_http::server::ListenerResult::SendResponse;
use std::time::{UNIX_EPOCH, SystemTime, Duration};
use std::thread::{sleep, spawn};
use std::net::TcpStream;
use my_http::client::write_request;
use std::sync::Arc;
use std::io::Read;

pub fn test_server(server_config: Config, num_connections: usize, num_loops_per_connection: usize, delays: bool, messages: Vec<(Request, Response)>) {
    let addr = server_config.addr;
    let mut server = Server::new(server_config);

    for (request, response) in messages.iter() {
        let uri = &request.uri;
        let response = response.clone();
        let request = request.clone();
        server.router.on(uri, move |_, req| {
            assert_eq!(request, *req);
            SendResponse(response.clone())
        });
    }

    let messages: Vec<(Request, Vec<u8>)> = messages.into_iter().map(|(req, res)| {
        let mut bytes: Vec<u8> = vec![];
        write_response(&mut bytes, &res).unwrap();
        (req, bytes)
    }).collect();

    let messages = Arc::new(messages);

    spawn(|| server.start());
    sleep(Duration::from_millis(100));

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

                    if delays {
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