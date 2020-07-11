use crate::common::request::Request;
use crate::common::response::Response;
use crate::server::router::ListenerResult::{Next, SendResponse};

pub enum ListenerResult {
    Next,
    SendResponse(Response),
}

impl ListenerResult {
    fn into_response(self) -> Option<Response> {
        match self {
            Next => None,
            SendResponse(response) => Some(response)
        }
    }
}

pub struct Router {
    listeners: Vec<(&'static str, Box<dyn Fn(&str, &Request) -> ListenerResult + 'static + Send + Sync>)>
}

impl Router {
    pub fn new() -> Router {
        Router {
            listeners: Vec::new()
        }
    }

    pub fn on_prefix(&mut self, uri: &'static str, listener: impl Fn(&str, &Request) -> ListenerResult + 'static + Send + Sync) {
        self.listeners.push((uri, Box::new(listener)))
    }

    pub fn on(&mut self, uri: &'static str, listener: impl Fn(&str, &Request) -> ListenerResult + 'static + Send + Sync) {
        let listener= move |router_uri: &str, request: &Request| {
            if uri == router_uri {
                return listener(router_uri, request);
            }
            Next
        };
        self.on_prefix(uri, listener);
    }

    pub fn route(&mut self, uri: &'static str, router: Router) {
        let listener = move |request_uri: &str, request: &Request| {
            router.process(&request_uri[uri.len()..], request)
        };
        self.on_prefix(uri, listener);
    }

    fn process(&self, request_uri: &str, request: &Request) -> ListenerResult {
        let listeners = self.listeners.iter()
            .filter(|(uri, _)| request_uri.starts_with(uri));

        for (_, listener) in listeners {
            let result = listener(request_uri, request);

            if let SendResponse(response) = result {
                return SendResponse(response);
            }
        }

        Next
    }

    pub fn response(&self, request: Request) -> Option<Response> {
        self.process(&request.uri, &request).into_response()
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
    use crate::server::router::ListenerResult::{Next, SendResponse};
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

    fn test_route_function_args(actual_uri: &str, actual_request: &Request,
                                expected_uri: &'static str, expected_request: Request) {
        assert_eq!(actual_uri, expected_uri);
        assert_eq!(format!("{:?}", actual_request), format!("{:?}", expected_request));
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

        router.on_prefix("/hello", |uri, request| {
            test_route_function_args(
                uri, request,
                "/hello", test_request("/hello"));
            Next
        });

        test_route(&router, "/hello", &function_calls(), None, vec![]);
    }

    #[test]
    fn listener_called() {
        let mut router = Router::new();
        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        router.on_prefix("/hello", move |_, _| {
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
        router.on_prefix("/hello", move |_, _| {
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
        router.on_prefix("/hello", move |_, _| {
            add_function_call(&calls_clone, "called 1");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on_prefix("/hello", move |_, _| {
            add_function_call(&calls_clone, "called 2");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on_prefix("/hello", move |_, _| {
            add_function_call(&calls_clone, "called 3");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called 1", "called 2", "called 3"]);
    }

    #[test]
    fn send_response_blocks() {
        let mut router = Router::new();

        router.on_prefix("/hello", |_, _| {
            SendResponse(test_response())
        });

        router.on_prefix("/hello", |_, _| {
            panic!()
        });

        test_route(&router, "/hello", &function_calls(), Some(test_response()), vec![]);
    }

    #[test]
    fn no_routes_hit() {
        let mut router = Router::new();

        router.on_prefix("/hello", |_, _| {
            panic!("Should not have been called")
        });

        router.on_prefix("/bye", |_, _| {
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
        router.on_prefix("/he", move |uri, request| {
            test_route_function_args(
                uri, request,
                "/hello", test_request("/hello"));
            add_function_call(&calls_clone, "called /he");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on_prefix("/hel", move |uri, request| {
            test_route_function_args(
                uri, request,
                "/hello", test_request("/hello"));
            add_function_call(&calls_clone, "called /hel");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on_prefix("/hell", move |uri, request| {
            test_route_function_args(
                uri, request,
                "/hello", test_request("/hello"));
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
        router.on_prefix("/h", move |_, _| {
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
        router.on_prefix("", move |_, _| {
            add_function_call(&calls_clone, "called");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called"]);
        test_route(&router, "/goodbye", &calls, None, vec!["called", "called"]);
        test_route(&router, "blahblah", &calls, None, vec!["called", "called", "called"]);
        test_route(&router, "/ewf/rg/wef", &calls, None, vec!["called", "called", "called", "called"]);
    }

    #[test]
    fn sub_router() {
        let mut router = Router::new();
        let mut sub_router = Router::new();

        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        sub_router.on_prefix("/bar", move |uri, request| {
            test_route_function_args(uri, request,
                                     "/bar", test_request("/foo/bar"));
            add_function_call(&calls_clone, "called");
            Next
        });

        router.route("/foo", sub_router);

        test_route(&router, "/foo/bar", &calls, None, vec!["called"]);
    }

    #[test]
    fn sub_sub_router() {
        let mut router = Router::new();
        let mut sub_router = Router::new();
        let mut sub_sub_router = Router::new();

        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        sub_sub_router.on_prefix("/bar", move |uri, request| {
            test_route_function_args(uri, request,
                                     "/bar", test_request("/baz/foo/bar"));
            add_function_call(&calls_clone, "called sub sub router");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        sub_router.on_prefix("/foo", move |uri, request| {
            test_route_function_args(uri, request,
                                     "/foo/bar", test_request("/baz/foo/bar"));
            add_function_call(&calls_clone, "called sub router");
            Next
        });


        sub_router.route("/foo", sub_sub_router);
        router.route("/baz", sub_router);

        test_route(&router, "/baz/foo/bar", &calls, None, vec!["called sub router", "called sub sub router"]);
    }

    #[test]
    fn sub_router_order_maintained() {
        let mut router = Router::new();
        let mut sub_router = Router::new();

        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        sub_router.on_prefix("/bar", move |_, _| {
            add_function_call(&calls_clone, "call 1");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        sub_router.on_prefix("/bar", move |_, _| {
            add_function_call(&calls_clone, "call 2");
            Next
        });

        router.route("/foo", sub_router);

        let calls_clone = Arc::clone(&calls);
        router.on_prefix("/foo", move |_, _| {
            add_function_call(&calls_clone, "call 3");
            Next
        });


        test_route(&router, "/foo/bar", &calls, None, vec!["call 1", "call 2", "call 3"]);
    }

    #[test]
    fn sub_router_sends_response() {
        let mut router = Router::new();
        let mut sub_router = Router::new();

        sub_router.on_prefix("/bar", move |_, _| {
            SendResponse(test_response())
        });

        sub_router.on_prefix("/bar", move |_, _| {
            panic!("Should not call this listener")
        });

        router.route("/foo", sub_router);

        router.on_prefix("/foo", move |_, _| {
            panic!("Should not call this listener")
        });


        test_route(&router, "/foo/bar", &function_calls(), Some(test_response()), vec![]);
    }

    #[test]
    fn sub_sub_router_sends_response() {
        let mut router = Router::new();
        let mut sub_router = Router::new();
        let mut sub_sub_router = Router::new();

        sub_sub_router.on_prefix("/bar", |_, _| {
            SendResponse(test_response())
        });

        sub_router.route("/foo", sub_sub_router);

        sub_router.on_prefix("/foo", |_, _| {
            panic!("Should not call this listener")
        });

        router.route("/baz", sub_router);

        router.on_prefix("/baz", |_, _| {
            panic!("Should not call this listener")
        });

        test_route(&router, "/baz/foo/bar", &function_calls(), Some(test_response()), vec![]);
    }

    #[test]
    fn strict_uri_match_listener() {
        let mut router = Router::new();
        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |_, _| {
            add_function_call(&calls_clone, "called");
            Next
        });

        test_route(&router, "/hello", &calls, None, vec!["called"]);
        test_route(&router, "/hello/hello", &calls, None, vec!["called"]);
        test_route(&router, "/bye", &calls, None, vec!["called"]);
    }

    #[test]
    fn strict_uri_match_sub_router() {
        let mut router = Router::new();
        let mut sub_router = Router::new();

        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        sub_router.on("/bar", move |_,_| {
            add_function_call(&calls_clone, "called");
            Next
        });

        router.route("/foo", sub_router);

        test_route(&router, "/foo/bar", &calls, None, vec!["called"]);
        test_route(&router, "/foo/bar/baz", &calls, None, vec!["called"]);
        test_route(&router, "/foo/bariugw", &calls, None, vec!["called"]);
        test_route(&router, "/foofoo", &calls, None, vec!["called"]);
    }
}