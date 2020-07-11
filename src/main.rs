use std::fs;
use std::io::Error;
use std::time::Duration;

use my_http::common::header::{CONTENT_LENGTH, HeaderMapOps, CONTENT_TYPE};
use my_http::common::response::Response;
use my_http::common::status::{NOT_FOUND_404, OK_200};
use my_http::server::config::Config;
use my_http::server::server::Server;
use my_http::server::router::Router;
use std::collections::HashMap;
use my_http::server::router::ListenerResult::SendResponse;

fn main() -> Result<(), Error> {
    let mut server = Server::new(Config {
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(10000),
        no_route_response: Response::from(&NOT_FOUND_404),
    });

    server.root_router.route("/middleton", file_router("/Users/Ben/Code/middletonSite"));
    server.root_router.route("", file_router("/Users/Ben/Code/ReactTetris/tetris-app/build"));

    server.start()
}

fn file_router(file_path: &'static str) -> Router {
    let mut router = Router::new();
    router.on("/", move |uri, _| {
        let mut path = String::from(file_path);
        path.push_str(uri);

        if path.ends_with("/") {
            path.push_str("index.html")
        }

        SendResponse(file_response(&path))
    });
    router
}

fn file_response(path: &str) -> Response {
    if let Ok(contents) = fs::read(path) {
        let mut headers = HashMap::new();
        headers.add_header(CONTENT_LENGTH, contents.len().to_string());

        if let Some(content_type) = get_content_type(path) {
            headers.add_header(CONTENT_TYPE, String::from(content_type));
        }

        return Response {
            status: &OK_200,
            headers,
            body: contents,
        };
    }
    return Response::from(&NOT_FOUND_404);
}

fn get_content_type(path: &str) -> Option<&'static str> {
    if path.ends_with(".ico") {
        return Some("image/x-icon")
    } else if path.ends_with(".js") {
        return Some("application/javascript")
    } else if path.ends_with(".svg") {
        return Some("image/svg+xml")
    }
    None
}