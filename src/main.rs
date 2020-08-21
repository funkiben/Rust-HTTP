use std::collections::HashMap;
use std::fs;
use std::io::Error;
use std::sync::{Arc, RwLock};

use my_http::{header_map, server};
use my_http::common::{header, status};
use my_http::common::response::Response;
use my_http::server::{Config, Router};
use my_http::server::ListenerResult::{SendResponse, SendResponseArc};

fn main() -> Result<(), Error> {
    let mut router = Router::new();

    router.on("/secret/message/path", |_, _| {
        let message = b"You found the secret message!";
        SendResponse(Response {
            status: status::OK,
            headers: header_map![(header::CONTENT_LENGTH, "29")],
            body: message.to_vec(),
        })
    });

    router.route("/my/middleton/website/", file_router("/Users/Ben/Code/middletonSite/"));
    router.route("/", file_router("/Users/Ben/Code/ReactTetris/tetris-app/build/"));

    server::start(Config {
        addr: "0.0.0.0:80",
        connection_handler_threads: 5,
        tls_config: None,
        router,
    })
}

fn file_router(directory: &'static str) -> Router {
    let mut router = Router::new();

    let cache: RwLock<HashMap<String, Arc<Response>>> = RwLock::new(HashMap::new());

    router.on_prefix("", move |uri, _| {
        let mut path = String::from(directory);
        path.push_str(uri);

        if path.ends_with("/") {
            path.push_str("index.html")
        }

        if let Some(response) = cache.read().unwrap().get(&path) { // read lock gets dropped after if statement
            return SendResponseArc(Arc::clone(response));
        }

        let response = Arc::new(file_response(&path));

        cache.write().unwrap().insert(path, Arc::clone(&response));

        SendResponseArc(response)
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
    return status::NOT_FOUND.into();
}

fn get_content_type(path: &str) -> &'static str {
    if path.ends_with(".ico") {
        return "image/x-icon";
    } else if path.ends_with(".js") {
        return "application/javascript";
    } else if path.ends_with(".svg") {
        return "image/svg+xml";
    } else if path.ends_with(".html") {
        return "text/html";
    } else if path.ends_with(".css") {
        return "text/css";
    }
    "text/plain"
}