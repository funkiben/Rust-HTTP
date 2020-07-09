use std::fs;
use std::io::Error;
use std::time::Duration;

use my_http::common::header::{CONTENT_LENGTH, HeaderMapOps};
use my_http::common::response::Response;
use my_http::common::status::{NOT_FOUND_404, OK_200};
use my_http::server::config::Config;
use my_http::server::router::Router;
use my_http::server::router::RequestHandlerResult::SendImmediately;
use my_http::server::server::Server;

fn main() -> Result<(), Error> {
    let mut server = Server::new(Config {
        addr: "localhost:7878",
        connection_handler_threads: 5,
        read_timeout: Duration::from_millis(10000),
        no_route_response: Response::from(&NOT_FOUND_404),
    });

    server.root_router.route("/middleton", serve_files("/Users/Ben/Code/middletonSite"));
    server.root_router.route("", serve_files("/Users/Ben/Code/ReactTetris/tetris-app/build"));

    server.start()
}

fn serve_files(file_path: &'static str) -> Router {
    let mut router = Router::new();
    router.on("/", move |uri, _, _| {
        let mut path = String::from(file_path);
        path.push_str(uri);

        if path.ends_with("/") {
            path.push_str("index.html")
        }

        SendImmediately(file_response(&path))
    });
    router
}

fn file_response(path: &str) -> Response {
    if let Ok(contents) = fs::read(path) {
        return Response {
            status: &OK_200,
            headers: HeaderMapOps::from(vec![(CONTENT_LENGTH, contents.len().to_string())]),
            body: contents,
        };
    }
    return Response::from(&NOT_FOUND_404);
}