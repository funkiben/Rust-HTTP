use std::sync::Arc;

use crate::common::request::Request;
use crate::common::response::Response;
use crate::server::router::ListenerResult::Next;

/// The result of a request listener.
#[derive(Debug, PartialEq)]
pub enum ListenerResult {
    /// Continues to the next listener to be called on the request, if any.
    Next,
    /// Stops execution of listeners for the request and immediately sends the response.
    SendResponse(Response),
    SendResponseArc(Arc<Response>),
}

/// A router that calls functions when requests with certain URI's are received.
pub struct Router {
    listeners: Vec<(&'static str, Box<dyn Fn(&str, &Request) -> ListenerResult + 'static + Send + Sync>)>
}

impl Router {
    /// Creates a new empty router.
    pub fn new() -> Router {
        Router {
            listeners: Vec::new()
        }
    }

    /// Calls the given function on requests with URI's that start with uri.
    /// If uri is empty, then the function will be called on all requests directed to this router.
    /// The first argument to the listener function is the URI local to this router.
    pub fn on_prefix(&mut self, uri: &'static str, listener: impl Fn(&str, &Request) -> ListenerResult + 'static + Send + Sync) {
        self.listeners.push((uri, Box::new(listener)))
    }

    /// Calls the given function on only requests with URIs that equal the given URI.
    pub fn on(&mut self, uri: &'static str, listener: impl Fn(&str, &Request) -> ListenerResult + 'static + Send + Sync) {
        let listener = move |router_uri: &str, request: &Request| {
            if uri == router_uri {
                return listener(router_uri, request);
            }
            Next
        };
        self.on_prefix(uri, listener);
    }

    /// Like on_prefix, but instead passes all requests that start with the given URI to router.
    /// The prefix is removed from the URI before being passed to router.
    /// ```
    /// use my_http::server::router::Router;
    /// use my_http::server::router::ListenerResult::Next;
    /// use std::collections::HashMap;
    /// use my_http::common::request::Request;
    ///
    /// let mut router = Router::new();
    /// let mut sub_router = Router::new();
    /// sub_router.on("/bar", |_,_| { println!("will print on requests to /foo/bar"); Next });
    /// router.route("/foo", sub_router);
    /// ```
    pub fn route(&mut self, uri: &'static str, router: Router) {
        let listener = move |request_uri: &str, request: &Request| {
            router.result_internal(&request_uri[uri.len()..], request)
        };
        self.on_prefix(uri, listener);
    }

    /// Calls listeners on the given request based on request_uri and produces a listener result.
    fn result_internal(&self, request_uri: &str, request: &Request) -> ListenerResult {
        self.listeners.iter()
            .filter(|(uri, _)| request_uri.starts_with(uri))
            .map(|(_, listener)| listener(request_uri, request))
            .find(|result| *result != Next)
            .unwrap_or(Next)
    }

    /// Gets the result from listeners that are called on the given request.
    /// The result from the last listener to be called on the given request is returned.
    /// If no listeners were called, then "Next" is returned.
    pub fn result(&self, request: &Request) -> ListenerResult {
        self.result_internal(&request.uri, request)
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
    use crate::server::router::{ListenerResult, Router};
    use crate::server::router::ListenerResult::{Next, SendResponse, SendResponseArc};

    type FunctionCalls = Arc<Mutex<Vec<&'static str>>>;

    fn function_calls() -> FunctionCalls {
        Arc::new(Mutex::new(vec![]))
    }

    fn add_function_call(calls: &FunctionCalls, call: &'static str) {
        calls.lock().unwrap().push(call)
    }

    fn test_route(router: &Router, uri: &'static str, calls: &FunctionCalls, expected_response: ListenerResult, expected_function_calls: Vec<&'static str>) {
        let actual_response = router.result(&test_request(uri));
        assert_eq!(format!("{:?}", actual_response), format!("{:?}", expected_response));
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
            status: OK_200,
            headers: Default::default(),
            body: vec![],
        }
    }

    #[test]
    fn no_routes() {
        test_route(&Router::new(), "", &function_calls(), Next, vec![])
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

        test_route(&router, "/hello", &function_calls(), Next, vec![]);
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

        test_route(&router, "/hello", &calls, Next, vec!["called"]);
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

        test_route(&router, "/hello", &calls, Next, vec!["called"]);
        test_route(&router, "/hello", &calls, Next, vec!["called", "called"]);
        test_route(&router, "/hello", &calls, Next, vec!["called", "called", "called"]);
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

        test_route(&router, "/hello", &calls, Next, vec!["called 1", "called 2", "called 3"]);
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

        test_route(&router, "/hello", &function_calls(), SendResponse(test_response()), vec![]);
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

        test_route(&router, "/goodbye", &function_calls(), Next, vec![]);
        test_route(&router, "blahblah", &function_calls(), Next, vec![]);
        test_route(&router, "/hihihi", &function_calls(), Next, vec![]);
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

        test_route(&router, "/hello", &calls, Next, vec!["called /he", "called /hel", "called /hell"]);
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

        test_route(&router, "/hello", &calls, Next, vec!["called"]);
        test_route(&router, "/hi", &calls, Next, vec!["called", "called"]);
        test_route(&router, "/hola", &calls, Next, vec!["called", "called", "called"]);
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

        test_route(&router, "/hello", &calls, Next, vec!["called"]);
        test_route(&router, "/goodbye", &calls, Next, vec!["called", "called"]);
        test_route(&router, "blahblah", &calls, Next, vec!["called", "called", "called"]);
        test_route(&router, "/ewf/rg/wef", &calls, Next, vec!["called", "called", "called", "called"]);
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

        test_route(&router, "/foo/bar", &calls, Next, vec!["called"]);
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

        test_route(&router, "/baz/foo/bar", &calls, Next, vec!["called sub router", "called sub sub router"]);
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


        test_route(&router, "/foo/bar", &calls, Next, vec!["call 1", "call 2", "call 3"]);
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


        test_route(&router, "/foo/bar", &function_calls(), SendResponse(test_response()), vec![]);
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

        test_route(&router, "/baz/foo/bar", &function_calls(), SendResponse(test_response()), vec![]);
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

        test_route(&router, "/hello", &calls, Next, vec!["called"]);
        test_route(&router, "/hello/hello", &calls, Next, vec!["called"]);
        test_route(&router, "/bye", &calls, Next, vec!["called"]);
    }

    #[test]
    fn strict_uri_match_sub_router() {
        let mut router = Router::new();
        let mut sub_router = Router::new();

        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        sub_router.on("/bar", move |_, _| {
            add_function_call(&calls_clone, "called");
            Next
        });

        router.route("/foo", sub_router);

        test_route(&router, "/foo/bar", &calls, Next, vec!["called"]);
        test_route(&router, "/foo/bar/baz", &calls, Next, vec!["called"]);
        test_route(&router, "/foo/bariugw", &calls, Next, vec!["called"]);
        test_route(&router, "/foofoo", &calls, Next, vec!["called"]);
    }

    #[test]
    fn send_response_arc_blocks() {
        let mut router = Router::new();

        let response = Arc::new(test_response());

        let response_clone = Arc::clone(&response);
        router.on_prefix("/hello", move |_, _| {
            SendResponseArc(Arc::clone(&response_clone))
        });

        router.on_prefix("/hello", move |_, _| {
            panic!()
        });

        test_route(&router, "/hello", &function_calls(), SendResponseArc(response), vec![]);
    }

    #[test]
    fn listeners_called_until_response_sent() {
        let mut router = Router::new();

        let calls = function_calls();

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |_, _| {
            add_function_call(&calls_clone, "call 1");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |_, _| {
            add_function_call(&calls_clone, "call 2");
            Next
        });

        let calls_clone = Arc::clone(&calls);
        router.on("/hello", move |_, _| {
            add_function_call(&calls_clone, "call 3");
            SendResponse(test_response())
        });

        router.on("/hello", move |_, _| {
            panic!()
        });

        test_route(&router, "/hello", &calls, SendResponse(test_response()), vec!["call 1", "call 2", "call 3"]);
    }
}