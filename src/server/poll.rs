use std::io::ErrorKind;
use std::net::SocketAddr;

use mio::{Events, Interest, Poll, Token};
use mio::event::Event;
use mio::net::{TcpListener, TcpStream};

use crate::server::slab::Slab;

/// The number of IO events processed at a time.
const POLL_EVENT_CAPACITY: usize = 128;

/// Initial number of connections to allocate space for.
const INITIAL_CONNECTION_CAPACITY: usize = 128;

/// Token used for the listener.
const LISTENER_TOKEN: Token = Token(usize::MAX);

/// Listens asynchronously on the given address. Calls make_connection for each new stream, and
/// calls on_readable_connection for each stream that is read ready.
/// The result of make_connection will be passed to on_readable_connection when the corresponding stream is ready for reading.
pub fn listen<T>(addr: SocketAddr,
                 on_new_connection: impl Fn(TcpStream, SocketAddr) -> T,
                 on_io_ready: impl Fn(&T)) -> std::io::Result<()> {
    let mut listener = TcpListener::bind(addr)?;

    let poll = Poll::new()?;
    poll.registry().register(&mut listener, LISTENER_TOKEN, Interest::READABLE)?;

    let mut connections = Slab::with_capacity(INITIAL_CONNECTION_CAPACITY);

    poll_events(
        poll,
        |poll, event|
            match event.token() {
                LISTENER_TOKEN => {
                    listen_until_blocked(&listener, |(mut stream, addr)| {
                        let token = connections.next_key();
                        poll.registry().register(&mut stream, Token(token), Interest::READABLE | Interest::WRITABLE)?;
                        connections.insert(on_new_connection(stream, addr));
                        Ok(())
                    });
                }
                token if event.is_write_closed() => { connections.remove(token.0); }
                token => { connections.get(token.0).map(&on_io_ready); }
            },
    )
}

/// Pulls events out of the given poll and passes them to on_event. Loops indefinitely.
fn poll_events(mut poll: Poll, mut on_event: impl FnMut(&mut Poll, &Event)) -> std::io::Result<()> {
    let mut events = Events::with_capacity(POLL_EVENT_CAPACITY);

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