use std::time::Duration;

use my_http::client::{Client, Config, RequestError};
use my_http::common::header::{HeaderMapOps, Header};
use my_http::common::method::Method;
use my_http::common::request::Request;
use my_http::common::status::OK_200;

#[test]
fn test_single_connection() -> Result<(), RequestError> {
    let c = Client::new(Config {
        addr: "www.google.com:80",
        read_timeout: Duration::from_secs(1),
        num_connections: 1,
    });

    let response = c.send(&Request {
        uri: "/".to_string(),
        method: Method::GET,
        headers: HeaderMapOps::from(vec![
            (Header::Custom("accept-encoding".to_string()), "identity".to_string())
        ]),
        body: vec![],
    })?;

    println!("{}", String::from_utf8_lossy(&response.body));

    assert_eq!(response.status, OK_200);
    assert!(!response.body.is_empty());

    Ok(())
}