use crate::common::request::Request;
use crate::common::response::Response;
use crate::server::router::ListenerResult::{Next, SendImmediately, SetResponse};

pub enum ListenerResult {
    Next,
    SetResponse(Response),
    SendImmediately(Response),
}

impl ListenerResult {
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
    Function(Box<dyn Fn(&str, &Request, Option<Response>) -> ListenerResult + 'static + Send + Sync>),
    Router(Router),
}

impl Router {
    pub fn new() -> Router {
        Router {
            listeners: Vec::new()
        }
    }

    pub fn on(&mut self, uri: &'static str, listener: impl Fn(&str, &Request, Option<Response>) -> ListenerResult + 'static + Send + Sync) {
        self.listeners.push((uri, RouteListener::Function(Box::new(listener))))
    }

    pub fn route(&mut self, uri: &'static str, router: Router) {
        self.listeners.push((uri, RouteListener::Router(router)))
    }

    fn process(&self, request_uri: &str, request: &Request, mut result: ListenerResult) -> ListenerResult {
        let listeners = self.listeners.iter()
            .filter(|(uri, _)| request_uri.starts_with(uri));

        for (uri, listener) in listeners {
            result = match listener {
                RouteListener::Function(function) => {
                    function(request_uri, request, result.into_response())
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
    use std::sync::{Arc, Mutex};

    use crate::common::method::Method;
    use crate::common::request::Request;
    use crate::common::response::Response;
    use crate::common::status::OK_200;
    use crate::server::router::ListenerResult::{Next, SendImmediately, SetResponse};
    use crate::server::router::Router;

    type FunctionCalls = Arc<Mutex<Vec<&'static str>>>;

    fn function_calls() -> FunctionCalls {
        Arc::new(Mutex::new(vec![]))
    }

    fn add_function_call(calls: &FunctionCalls, call: &'static str) {
        calls.lock().unwrap().push(call)
    }

    fn test_route(router: &Router, uri: &'static str, calls: &FunctionCalls, expected_response: Option<Response>, expected_function_calls: Vec<&'static str>) {
        let result = router.response(test_request(uri));
        assert_eq!(format!("{:?}", result), format!("{:?}", expected_response));
        assert_eq!(format!("{:?}", calls.lock().unwrap()), format!("{:?}", expected_function_calls));
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
        test_route(&Router::new(), "", &function_calls(), None, vec![])
    }

    #[test]
    fn listener_args() {
        let mut router = Router::new();

        router.on("/hello", |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                None);
            Next
        });

        test_route(&router, "/hello", &function_calls(), None, vec![]);
    }

    #[test]
    fn listener_called() {
        let mut router = Router::new();
        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |_, _, _| {
            add_function_call(&calls_clone, "called");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called"]);
    }

    #[test]
    fn listener_called_multiple_times() {
        let mut router = Router::new();
        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |_, _, _| {
            add_function_call(&calls_clone, "called");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called"]);
        test_route(&router, "/hello", &calls, None, vec!["called", "called"]);
        test_route(&router, "/hello", &calls, None, vec!["called", "called", "called"]);
    }

    #[test]
    fn multiple_listeners_called() {
        let mut router = Router::new();
        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |_, _, _| {
            add_function_call(&calls_clone, "called 1");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |_, _, _| {
            add_function_call(&calls_clone, "called 2");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |_, _, _| {
            add_function_call(&calls_clone, "called 3");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called 1", "called 2", "called 3"]);
    }

    #[test]
    fn send_immediately_blocks() {
        let mut router = Router::new();

        router.on("/hello", |_, _, _| {
            SendImmediately(test_response())
        });

        router.on("/hello", |_, _, _| {
            panic!()
        });

        test_route(&router, "/hello", &function_calls(), Some(test_response()), vec![]);
    }

    #[test]
    fn no_routes_hit() {
        let mut router = Router::new();

        router.on("/hello", |_, _, _| {
            panic!("Should not have been called")
        });

        router.on("/bye", |_, _, _| {
            panic!("Should not have been called")
        });

        test_route(&router, "/goodbye", &function_calls(), None, vec![]);
        test_route(&router, "blahblah", &function_calls(), None, vec![]);
        test_route(&router, "/hihihi", &function_calls(), None, vec![]);
    }

    #[test]
    fn listener_with_prefix_uri() {
        let mut router = Router::new();
        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        router.on("/he", move |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                None);
            add_function_call(&calls_clone, "called /he");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on("/hel", move |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                None);
            add_function_call(&calls_clone, "called /hel");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on("/hell", move |uri, request, response| {
            test_route_function_args(
                uri, request, response,
                "/hello",
                test_request("/hello"),
                None);
            add_function_call(&calls_clone, "called /hell");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called /he", "called /hel", "called /hell"]);
    }

    #[test]
    fn listener_with_prefix_uri_called_multiple_times() {
        let mut router = Router::new();
        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        router.on("/h", move |_, _, _| {
            add_function_call(&calls_clone, "called");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called"]);
        test_route(&router, "/hi", &calls, None, vec!["called", "called"]);
        test_route(&router, "/hola", &calls, None, vec!["called", "called", "called"]);
    }

    #[test]
    fn listener_with_empty_uri_always_called() {
        let mut router = Router::new();
        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        router.on("", move |_, _, _| {
            add_function_call(&calls_clone, "called");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called"]);
        test_route(&router, "/goodbye", &calls, None, vec!["called", "called"]);
        test_route(&router, "blahblah", &calls, None, vec!["called", "called", "called"]);
        test_route(&router, "/ewf/rg/wef", &calls, None, vec!["called", "called", "called", "called"]);
    }

    #[test]
    fn set_response() {
        let mut router = Router::new();

        router.on("/hello", move |_, _, _| {
            SetResponse(test_response())
        });

        test_route(&router, "/hello", &function_calls(), Some(test_response()), vec![]);
    }

    #[test]
    fn response_unset() {
        let mut router = Router::new();
        let calls = function_calls();

        router.on("/hello", move |_, _, _| {
            SetResponse(test_response())
        });

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |uri, request, response| {
            test_route_function_args(uri, request, response,
                                     "/hello", test_request("/hello"), Some(test_response()));
            add_function_call(&calls_clone, "called");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called"]);
    }
}