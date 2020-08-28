use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rustls::{ClientConfig, ClientSession};

use crate::client::Config;
use crate::util::tls_stream::TlsStream;

/// A client TLS stream.
pub type ClientTlsStream = TlsStream<ClientSession, TcpStream>;

/// A factory that produces new streams to a server.
pub trait StreamFactory<T>: Send + Sync {
    fn create(&self) -> std::io::Result<T>;
}

/// A stream factory for producing plain TCP streams.
pub struct TcpStreamFactory {
    addr: &'static str,
    read_timeout: Duration,
}

impl TcpStreamFactory {
    /// Creates a new TCP stream factory using the given config,
    pub fn new(config: &Config) -> TcpStreamFactory {
        TcpStreamFactory { addr: config.addr, read_timeout: config.read_timeout }
    }
}

impl StreamFactory<TcpStream> for TcpStreamFactory {
    fn create(&self) -> std::io::Result<TcpStream> {
        let stream = TcpStream::connect(self.addr)?;
        stream.set_read_timeout(Some(self.read_timeout)).unwrap();

        Ok(stream)
    }
}

/// A stream factory for producing TLS encrypted streams to a server.
pub struct TlsStreamFactory {
    tcp_stream_factory: TcpStreamFactory,
    tls_config: Arc<ClientConfig>,
    dns_name: webpki::DNSName,
}

impl TlsStreamFactory {
    /// Creates a new TLS stream factory with the given configs.
    pub fn new(config: &Config, tls_config: ClientConfig) -> TlsStreamFactory {
        let dns_name = config.addr.split(":").next().expect("Invalid address.");
        let dns_name = webpki::DNSNameRef::try_from_ascii_str(dns_name).expect("Failed to look up address.").into();

        TlsStreamFactory {
            tcp_stream_factory: TcpStreamFactory::new(config),
            tls_config: Arc::new(tls_config),
            dns_name,
        }
    }
}

impl StreamFactory<ClientTlsStream> for TlsStreamFactory {
    fn create(&self) -> std::io::Result<ClientTlsStream> {
        let stream = self.tcp_stream_factory.create()?;
        let session = ClientSession::new(&self.tls_config, self.dns_name.as_ref());
        Ok(ClientTlsStream::new(session, stream))
    }
}
