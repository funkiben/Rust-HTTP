use crate::common::request::Request;
use crate::common::response::Response;
use crate::server::router::RequestHandlerResult::{Next, SendImmediately, SetResponse};

pub enum RequestHandlerResult {
    Next,
    SetResponse(Response),
    SendImmediately(Response),
}

impl RequestHandlerResult {
    fn into_response(self) -> Option<Response> {
        match self {
            SetResponse(response) | SendImmediately(response) => Some(response),
            _ => None
        }
    }
}

pub struct Router {
    listeners: Vec<(&'static str, RouteListener)>
}

enum RouteListener {
    Function(Box<dyn Fn(&str, &Request, Option<Response>) -> RequestHandlerResult + 'static + Send + Sync>),
    Router(Router),
}

impl Router {
    pub fn new() -> Router {
        Router {
            listeners: Vec::new()
        }
    }

    pub fn on(&mut self, uri: &'static str, handler: impl Fn(&str, &Request, Option<Response>) -> RequestHandlerResult + 'static + Send + Sync) {
        self.listeners.push((uri, RouteListener::Function(Box::new(handler))))
    }

    pub fn route(&mut self, uri: &'static str, router: Router) {
        self.listeners.push((uri, RouteListener::Router(router)))
    }

    fn process(&self, request_uri: &str, request: &Request, mut result: RequestHandlerResult) -> RequestHandlerResult {
        let listeners = self.listeners.iter()
            .filter(|(uri, _)| request_uri.starts_with(uri));

        for (uri, listener) in listeners {
            result = match listener {
                RouteListener::Function(handler) => {
                    handler(request_uri, request, result.into_response())
                }
                RouteListener::Router(router) => {
                    router.process(&request_uri[uri.len()..], request, result)
                }
            };

            result = match result {
                SendImmediately(response) => return SendImmediately(response),
                x => x
            }
        }

        result
    }

    pub fn response(&self, request: Request) -> Option<Response> {
        self.process(&request.uri, &request, Next).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::method::Method;
    use crate::common::request::Request;
    use crate::common::response::Response;
    use crate::common::status::OK_200;
    use crate::server::router::Router;
    use crate::server::router::RequestHandlerResult::{Next, SendImmediately, SetResponse};

    fn test_route(router: Router, uri: &'static str, expected_response: Option<Response>) {
        let result = router.response(test_request(uri));
        assert_eq!(format!("{:?}", result), format!("{:?}", expected_response));
    }

    fn test_route_function_args(actual_uri: &str, actual_request: &Request, actual_response: Option<Response>,
                                expected_uri: &'static str, expected_request: Request, expected_response: Option<Response>) {
        assert_eq!(actual_uri, expected_uri);
        assert_eq!(format!("{:?}", actual_request), format!("{:?}", expected_request));
        assert_eq!(format!("{:?}", actual_response), format!("{:?}", expected_response));
    }

    fn test_request(uri: &'static str) -> Request {
        Request {
            uri: String::from(uri),
            method: Method::Get,
            headers: HashMap::new(),
            body: vec![],
        }
    }

    fn test_response() -> Response {
        Response {
            status: &OK_200,
            headers: Default::default(),
            body: vec![],
        }
    }

    #[test]
    fn no_routes() {
        test_route(Router::new(), "", None)
    }

    #[test]
    fn listener_function_args() {
        let mut router = Router::new();

        router.on("/hello", |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                None);
            Next
        });

        test_route(router, "/hello", None);
    }

    #[test]
    fn send_immediately_blocks() {
        let mut router = Router::new();

        router.on("/hello", |_,_,_| {
            SendImmediately(test_response())
        });

        router.on("/hello", |_,_,_| {
            panic!()
        });

        test_route(router, "/hello", Some(test_response()));
    }

    #[test]
    fn uri_with_no_routes() {
        let mut router = Router::new();

        router.on("/hello", |_, _, _| {
            panic!("Should not have been called")
        });

        router.on("/bye", |_, _, _| {
            panic!("Should not have been called")
        });

        test_route(router, "/goodbye", None);
    }

    #[test]
    fn prefix() {
        let mut router = Router::new();

        router.on("/he", |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                None);
            SetResponse(test_response())
        });

        test_route(router, "/hello", Some(test_response()));
    }

    #[test]
    fn test_next_propagates() {
        let mut router = Router::new();

        router.on("/hello", |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                None);
            Next
        });

        router.on("/hello", |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                None);
            SetResponse(test_response())
        });

        test_route(router, "/hello",  Some(test_response()));
    }

    #[test]
    fn test_set_response_propagates() {
        let mut router = Router::new();

        router.on("/hello", |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                None);
            SetResponse(test_response())
        });

        router.on("/hello", |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                Some(test_response()));
            Next
        });

        test_route(router, "/hello", None);
    }

}