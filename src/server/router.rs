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
        let listeners = self.listeners.iter().filter(|(uri, _)| request_uri.starts_with(uri));

        for (uri, listener) in listeners {
            if request_uri.starts_with(uri) {
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
        }

        result
    }

    pub fn response(&self, request: Request) -> Option<Response> {
        self.process(&request.uri, &request, Next).into_response()
    }
}