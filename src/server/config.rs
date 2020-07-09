use std::time::Duration;
use crate::common::response::Response;

pub struct Config {
    pub addr: &'static str,
    pub connection_handler_threads: usize,
    pub read_timeout: Duration,
    pub no_route_response: Response
}