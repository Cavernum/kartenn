//! Sending formatted messages: framing (RFC 6587 §3.4) and transports, asynchronous on
//! tokio, with reconnection.
//!
//! A [`Sender`] owns at most one connection. [`Sender::send`] writes a batch and returns
//! once it is written and flushed; on failure it drops the connection and the next call
//! reconnects, no sooner than an exponential back-off allows. It keeps no queue: queueing,
//! and what to do when a destination stays unreachable, belong to the caller.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::Instant;
#[cfg(feature = "tls")]
use tokio_rustls::TlsConnector;
#[cfg(feature = "tls")]
use tokio_rustls::rustls::ClientConfig;
#[cfg(feature = "tls")]
use tokio_rustls::rustls::pki_types::ServerName;

/// How messages are delimited on a stream (datagrams carry one message each).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Framing {
    /// `MSG-LEN SP SYSLOG-MSG` (RFC 6587 §3.4.1), required over TLS (RFC 5425 §4.3):
    /// any octet may appear in a message.
    #[default]
    OctetCounting,
    /// The message followed by a line feed (RFC 6587 §3.4.2), for receivers that know no
    /// other framing; a message holding a line feed is refused.
    LineFeed,
}

/// Where messages go.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Target {
    /// Local datagram socket, such as `/dev/log` (RFC 3164 messages, usually).
    #[cfg(unix)]
    UnixDatagram(PathBuf),
    /// Local stream socket.
    #[cfg(unix)]
    UnixStream(PathBuf),
    /// UDP (RFC 5426), `host:port`: no delivery guarantee.
    Udp(String),
    /// TCP, `host:port`.
    Tcp(String),
    /// TLS over TCP (RFC 5425), `host:port`, checked against `server_name`.
    #[cfg(feature = "tls")]
    Tls {
        /// `host:port`.
        address: String,
        /// Name the certificate must carry.
        server_name: ServerName<'static>,
        /// Roots, client certificate and provider; rustls only speaks TLS 1.2 and 1.3,
        /// and a client config sends no early data unless asked to.
        config: Arc<ClientConfig>,
    },
}

/// Bounds of the wait between two connection attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    /// Wait after a first failure.
    pub min: Duration,
    /// Longest wait.
    pub max: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            min: Duration::from_millis(100),
            max: Duration::from_secs(30),
        }
    }
}

/// An open connection.
enum Connection {
    #[cfg(unix)]
    UnixDatagram(tokio::net::UnixDatagram),
    Udp(UdpSocket),
    Stream(BufWriter<Box<dyn AsyncWrite + Send + Unpin>>),
}

/// Sends formatted messages to one [`Target`].
pub struct Sender {
    target: Target,
    framing: Framing,
    backoff: Backoff,
    connection: Option<Connection>,
    /// Earliest next attempt, and the wait that set it.
    retry: Option<(Instant, Duration)>,
}

impl std::fmt::Debug for Sender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender")
            .field("target", &self.target)
            .field("framing", &self.framing)
            .field("connected", &self.connection.is_some())
            .finish_non_exhaustive()
    }
}

impl Sender {
    /// A sender, not yet connected: the first [`Sender::send`] connects.
    #[must_use]
    pub fn new(target: Target) -> Self {
        Self {
            target,
            framing: Framing::default(),
            backoff: Backoff::default(),
            connection: None,
            retry: None,
        }
    }

    /// Framing on streams (octet counting by default).
    #[must_use]
    pub const fn framing(mut self, framing: Framing) -> Self {
        self.framing = framing;
        self
    }

    /// Bounds of the reconnection back-off.
    #[must_use]
    pub const fn backoff(mut self, backoff: Backoff) -> Self {
        self.backoff = backoff;
        self
    }

    /// Whether a connection is open.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    /// Writes `messages` in order, each framed, then flushes; returns once all are
    /// written. Connects first if needed.
    ///
    /// # Errors
    ///
    /// - [`io::ErrorKind::InvalidInput`]: a message holds a line feed under
    ///   [`Framing::LineFeed`]; nothing is written.
    /// - [`io::ErrorKind::WouldBlock`]: the back-off forbids a new attempt yet.
    /// - Any connection or write error: the connection is dropped and the batch may be
    ///   partly written; the caller sends it again (at-least-once).
    pub async fn send<M: AsRef<[u8]>>(&mut self, messages: &[M]) -> io::Result<()> {
        if self.framing == Framing::LineFeed && messages.iter().any(|m| m.as_ref().contains(&b'\n'))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a message holds a line feed, which line-feed framing cannot carry",
            ));
        }
        if self.connection.is_none() {
            self.connect().await?;
        }
        let result = self.write(messages).await;
        if result.is_err() {
            self.connection = None;
        }
        result
    }

    /// Flushes and closes the connection, if any: over TLS, sends `close_notify`, without
    /// which the receiver cannot tell a complete stream from a truncated one. Dropping a
    /// sender closes the socket without it.
    ///
    /// # Errors
    ///
    /// Write errors while flushing or shutting down.
    pub async fn close(&mut self) -> io::Result<()> {
        match self.connection.take() {
            Some(Connection::Stream(mut stream)) => stream.shutdown().await,
            _ => Ok(()),
        }
    }

    async fn connect(&mut self) -> io::Result<()> {
        if let Some((at, _)) = self.retry
            && Instant::now() < at
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "waiting before reconnecting",
            ));
        }
        match open(&self.target).await {
            Ok(connection) => {
                self.connection = Some(connection);
                self.retry = None;
                Ok(())
            }
            Err(e) => {
                let wait = self.retry.map_or(self.backoff.min, |(_, wait)| {
                    (wait * 2).min(self.backoff.max)
                });
                self.retry = Some((Instant::now() + wait, wait));
                Err(e)
            }
        }
    }

    async fn write<M: AsRef<[u8]>>(&mut self, messages: &[M]) -> io::Result<()> {
        let framing = self.framing;
        match self.connection.as_mut() {
            None => Err(io::Error::from(io::ErrorKind::NotConnected)),
            #[cfg(unix)]
            Some(Connection::UnixDatagram(socket)) => {
                for m in messages {
                    socket.send(m.as_ref()).await?;
                }
                Ok(())
            }
            Some(Connection::Udp(socket)) => {
                for m in messages {
                    socket.send(m.as_ref()).await?;
                }
                Ok(())
            }
            Some(Connection::Stream(stream)) => {
                for m in messages {
                    let m = m.as_ref();
                    match framing {
                        Framing::OctetCounting => {
                            stream.write_all(format!("{} ", m.len()).as_bytes()).await?;
                            stream.write_all(m).await?;
                        }
                        Framing::LineFeed => {
                            stream.write_all(m).await?;
                            stream.write_all(b"\n").await?;
                        }
                    }
                }
                stream.flush().await
            }
        }
    }
}

async fn open(target: &Target) -> io::Result<Connection> {
    let stream: Box<dyn AsyncWrite + Send + Unpin> = match target {
        #[cfg(unix)]
        Target::UnixDatagram(path) => {
            let socket = tokio::net::UnixDatagram::unbound()?;
            socket.connect(path)?;
            return Ok(Connection::UnixDatagram(socket));
        }
        #[cfg(unix)]
        Target::UnixStream(path) => Box::new(tokio::net::UnixStream::connect(path).await?),
        Target::Udp(address) => {
            let remote = tokio::net::lookup_host(address)
                .await?
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no address"))?;
            let local = if remote.is_ipv4() {
                "0.0.0.0:0"
            } else {
                "[::]:0"
            };
            let socket = UdpSocket::bind(local).await?;
            socket.connect(remote).await?;
            return Ok(Connection::Udp(socket));
        }
        Target::Tcp(address) => {
            let stream = TcpStream::connect(address).await?;
            stream.set_nodelay(true)?;
            Box::new(stream)
        }
        #[cfg(feature = "tls")]
        Target::Tls {
            address,
            server_name,
            config,
        } => {
            let tcp = TcpStream::connect(address).await?;
            tcp.set_nodelay(true)?;
            let connector = TlsConnector::from(Arc::clone(config));
            Box::new(connector.connect(server_name.clone(), tcp).await?)
        }
    };
    Ok(Connection::Stream(BufWriter::new(stream)))
}
