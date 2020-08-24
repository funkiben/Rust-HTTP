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

/// Listens asynchronously on the given address. Calls make_connection for each new stream, and
/// calls on_readable_connection for each stream that is read ready.
/// The result of make_connection will be passed to on_readable_connection when the corresponding stream is ready for reading.
pub fn listen<T>(addr: SocketAddr, on_new_connection: impl Fn(TcpStream, SocketAddr) -> T, on_readable_connection: impl Fn(&T), on_writeable_connection: impl Fn(&T)) -> std::io::Result<()> {
    const SERVER_TOKEN: Token = Token(usize::MAX);

    let mut listener = TcpListener::bind(addr)?;

    let mut connection_slab = Slab::with_capacity(INITIAL_CONNECTION_CAPACITY);

    let poll = Poll::new()?;
    poll.registry().register(&mut listener, SERVER_TOKEN, Interest::READABLE)?;

    poll_events(
        poll,
        |poll, event|
            match event.token() {
                SERVER_TOKEN => {
                    listen_until_blocked(&listener, |(mut stream, addr)| {
                        let token = connection_slab.next_key();
                        poll.registry().register(&mut stream, Token(token), Interest::READABLE | Interest::WRITABLE)?;
                        connection_slab.insert(on_new_connection(stream, addr));

                        Ok(())
                    });
                }
                token if event.is_write_closed() => {
                    connection_slab.remove(token.0);
                }
                token if event.is_readable() => {
                    connection_slab.get(token.0).map(&on_readable_connection);
                }
                token if event.is_writable() => {
                    connection_slab.get(token.0).map(&on_writeable_connection);
                }
                _ => {}
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