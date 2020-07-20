use std::time::Duration;

pub struct Config {
    pub addr: &'static str,
    pub read_timeout: Duration,
    pub max_connections: usize,
}