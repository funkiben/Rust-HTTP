use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener, TcpStream};

use popol::{Event, Events, interest, Sources};

use crate::util::slab::Slab;

#[derive(Eq, PartialEq, Clone)]
enum Source {
    Client(usize),
    Listener,
}

/// Listens asynchronously on the given address. Calls make_connection for each new stream, and
/// calls on_readable_connection for each stream that is read ready.
/// The result of make_connection will be passed to on_readable_connection when the corresponding stream is ready for reading.
pub fn listen<T>(addr: SocketAddr, on_new_connection: impl Fn(TcpStream, SocketAddr) -> T, on_readable_connection: impl Fn(&T)) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;

    let mut sources = Sources::new();

    sources.register(Source::Listener, &listener, interest::READ);

    let mut connections = Slab::new();

    poll_events(
        sources,
        |sources, key, event|
            match key {
                Source::Listener => {
                    listen_until_blocked(&listener, |(stream, addr)| {
                        let key = connections.next_key();
                        stream.set_nonblocking(true)?;
                        sources.register(Source::Client(key), &stream, interest::READ);
                        connections.insert(on_new_connection(stream, addr));
                        Ok(())
                    });
                }
                Source::Client(key) if event.errored => {
                    connections.remove(*key);
                }
                Source::Client(key) => {
                    connections.get(*key).map(&on_readable_connection);
                }
            },
    )
}

/// Pulls events out of the given poll and passes them to on_event. Loops indefinitely.
fn poll_events<T: Clone + Eq>(mut sources: Sources<T>, mut on_event: impl FnMut(&mut Sources<T>, &T, Event)) -> std::io::Result<()> {
    let mut events = Events::new();

    loop {
        sources.wait(&mut events)?;

        for (key, event) in events.iter() {
            on_event(&mut sources, key, event);
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