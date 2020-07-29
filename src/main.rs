use std::collections::HashMap;
use std::fs;
use std::io::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use my_http::common::header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderMapOps};
use my_http::common::response::Response;
use my_http::common::status::{NOT_FOUND_404, OK_200};
use my_http::server::{Config, Server};
use my_http::server::ListenerResult::{SendResponse, SendResponseArc};
use my_http::server::Router;
use my_http::header_map;

fn main() -> Result<(), Error> {
    let mut server = Server::new(Config {
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(1000),
    });

    server.router.on("/secret/message/path", |_, _| {
        let message = b"You found the secret message!";
        SendResponse(Response {
            status: OK_200,
            headers: header_map![(CONTENT_LENGTH, "29")],
            body: message.to_vec(),
        })
    });

    server.router.route("/my/middleton/website/", file_router("/Users/Ben/Code/middletonSite/"));
    server.router.route("/", file_router("/Users/Ben/Code/ReactTetris/tetris-app/build/"));

    server.start()
}

fn file_router(directory: &'static str) -> Router {
    let mut router = Router::new();

    let cache: Mutex<HashMap<String, Arc<Response>>> = Mutex::new(HashMap::new());

    router.on_prefix("", move |uri, _| {
        let mut path = String::from(directory);
        path.push_str(uri);

        if path.ends_with("/") {
            path.push_str("index.html")
        }

        let mut cache = cache.lock().unwrap();

        let response = cache.entry(path.clone()).or_insert_with(|| Arc::new(file_response(&path)));

        SendResponseArc(Arc::clone(&response))
        // SendResponse(file_response(&path))
    });

    router
}

fn file_response(file_path: &str) -> Response {
    if let Ok(contents) = fs::read(file_path) {
        let mut headers = header_map![(CONTENT_LENGTH, contents.len().to_string())];

        if let Some(content_type) = get_content_type(file_path) {
            headers.add_header(CONTENT_TYPE, String::from(content_type));
        }

        return Response { status: OK_200, headers, body: contents };
    }
    return Response::from_status(NOT_FOUND_404);
}

fn get_content_type(path: &str) -> Option<&'static str> {
    if path.ends_with(".ico") {
        return Some("image/x-icon");
    } else if path.ends_with(".js") {
        return Some("application/javascript");
    } else if path.ends_with(".svg") {
        return Some("image/svg+xml");
    }
    None
}