use std::collections::HashMap;
use std::fs;
use std::io::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use my_http::common::{header, status};
use my_http::common::response::Response;
use my_http::header_map;
use my_http::server::{Config, Server};
use my_http::server::ListenerResult::{SendResponse, SendResponseArc};
use my_http::server::Router;

fn main() -> Result<(), Error> {
    let mut server = Server::new(Config {
        addr: "0.0.0.0:80",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(1000),
        tls_config: None,
    });

    server.router.on("/secret/message/path", |_, _| {
        let message = b"You found the secret message!";
        SendResponse(Response {
            status: status::OK,
            headers: header_map![(header::CONTENT_LENGTH, "29")],
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
    });

    router
}

fn file_response(file_path: &str) -> Response {
    if let Ok(contents) = fs::read(file_path) {
        let headers = header_map![
            (header::CONTENT_LENGTH, contents.len().to_string()),
            (header::CONTENT_TYPE, get_content_type(file_path))
        ];

        return Response { status: status::OK, headers, body: contents };
    }
    return Response::from_status(status::NOT_FOUND);
}

fn get_content_type(path: &str) -> &'static str {
    if path.ends_with(".ico") {
        return "image/x-icon";
    } else if path.ends_with(".js") {
        return "application/javascript";
    } else if path.ends_with(".svg") {
        return "image/svg+xml";
    } else if path.ends_with(".html") {
        return "text/html"
    } else if path.ends_with(".css") {
        return "text/css"
    }
    "text/plain"
}