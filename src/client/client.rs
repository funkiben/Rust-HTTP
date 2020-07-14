use std::io::{BufReader, Error, Read, Write};
use std::net::TcpStream;
use std::sync::{Mutex, MutexGuard};

use crate::client::client::RequestError::CantSend;
use crate::client::config::Config;
use crate::common::request::Request;
use crate::common::response::Response;

pub struct Client {
    pub config: Config,
    connections: Vec<Mutex<TcpStream>>,
}

impl Client {
    pub fn new(config: Config) -> Client {
        assert!(config.max_connections > 0);
        Client { connections: Vec::with_capacity(config.max_connections), config }
    }

    pub fn send(&mut self, request: &Request) -> Result<Response, RequestError> {
        let request = request_to_bytes(request);
        let connection = self.find_connection_and_write(&request).map_err(|err| => CantSend(err)) ?
    }

    fn find_connection_and_write(&mut self, bytes: &[u8]) -> Result<MutexGuard<TcpStream>, Error> {
        if let Some(connection) = self.try_write_to_existing_connection(bytes) {
            return Ok(connection);
        }

        if self.connections.len() < self.config.max_connections {
            let mut connection = self.open_new_connection()?;
            connection.write(bytes)?;
            return Ok(lock);
        } else {
            return Ok(self.connections.get(0).unwrap().lock().unwrap());
        }
    }

    fn try_write_to_existing_connection(&mut self, bytes: &[u8]) -> Option<MutexGuard<TcpStream>> {
        for (index, mutex) in self.connections.iter().enumerate() {
            if let Ok(mut connection) = mutex.try_lock() {
                if let Ok(size) = connection.write(bytes) {
                    return Some(connection);
                } else {
                    self.connections.remove(index);
                }
            }
        }
        None
    }

    fn open_new_connection(&mut self) -> Result<MutexGuard<TcpStream>, Error> {
        let connection = Mutex::new(TcpStream::connect(self.config.addr)?);
        let lock = connection.lock().unwrap();
        self.connections.push(connection);
        return Ok(lock);
    }
}

fn request_to_bytes(request: &Request) -> Vec<u8> {
    // TODO
    vec![]
}

fn read_next_response(reader: impl Read) -> Result<Response, ResponseParsingError> {
    let reader = BufReader::from(reader);
}

pub enum RequestError {
    Timeout,
    InvalidResponse(ResponseParsingError),
    CantSend(Error),
}

pub enum ResponseParsingError {}