use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::SocketAddr;

use mio::{Events, Interest, Poll, Token};
use mio::event::Event;
use mio::net::{TcpListener, TcpStream};

/// Listens asynchronously on the given address. Calls make_connection for each new stream, and
/// calls on_readable_connection for each stream that is read ready.
/// The result of make_connection will be passed to on_readable_connection when the corresponding stream is ready for reading.
pub fn listen<T>(addr: SocketAddr, make_connection: impl Fn(TcpStream, SocketAddr) -> T, on_readable_connection: impl Fn(&T)) -> std::io::Result<()> {
    const SERVER_TOKEN: Token = Token(0);

    let mut listener = TcpListener::bind(addr)?;
    let mut connections = HashMap::with_capacity(128);

    let poll = Poll::new()?;
    poll.registry().register(&mut listener, SERVER_TOKEN, Interest::READABLE)?;

    let mut next_token = SERVER_TOKEN.0 + 1;

    poll_events(
        poll,
        |poll, event|
            match event.token() {
                SERVER_TOKEN => {
                    listen_until_blocked(&listener, |(mut stream, addr)| {
                        let token = Token(next_token);
                        next_token += 1;
                        poll.registry().register(&mut stream, token, Interest::READABLE)?;

                        connections.insert(token, make_connection(stream, addr));

                        Ok(())
                    });
                }
                token if event.is_read_closed() => {
                    connections.remove(&token);
                }
                token => {
                    connections.get(&token).map(&on_readable_connection);
                }
            },
    )
}

/// Pulls events out of the given poll and passes them to on_event. Loops indefinitely.
fn poll_events(mut poll: Poll, mut on_event: impl FnMut(&mut Poll, &Event)) -> std::io::Result<()> {
    let mut events = Events::with_capacity(128);

    loop {
        poll.poll(&mut events, None)?;

        for event in &events {
            on_event(&mut poll, event);
        }
    }
}

/// Accepts new connections to the given listener until blocked. Calls on_connection for each connection stream.
fn listen_until_blocked(listener: &TcpListener, mut on_connection: impl FnMut((TcpStream, SocketAddr)) -> std::io::Result<()>) {
    loop {
        match listener.accept() {
            Ok(conn) => {
                if let Some(err) = on_connection(conn).err() {
                    println!("Error initializing connection: {:?}", err)
                }
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => break,
            Err(err) => println!("Error unwrapping connection: {:?}", err)
        }
    }
}