use std::io::{Read, Write};
use std::sync::Arc;
use std::thread::spawn;

use my_http::client::{Client, StreamFactory};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::status::Status;
use my_http::header_map;

pub fn test_empty_requests<S: Read + Write + Send + 'static, F: StreamFactory<S> + 'static>(client: Client<S, F>, requests: usize, expected_status: Status, should_have_body: bool) {
    println!("sending requests to {}", client.config.addr);

    let website = client.config.addr.split(":").next().unwrap();

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