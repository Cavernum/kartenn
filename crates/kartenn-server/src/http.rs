//! Serving the API (spec 006, REQ-006-32): HTTP/1.1 and HTTP/2 over one accept loop,
//! TLS terminated by `tokio-rustls` (ALPN `h2`, `http/1.1`) or clear HTTP behind a proxy
//! (`h2c` by prior knowledge), the certificate read again without a restart, and a
//! bounded drain at the stop.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use arc_swap::ArcSwap;
use axum::Router;
use axum::extract::ConnectInfo;
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use hyper_util::server::conn::auto;
use hyper_util::server::graceful::{GracefulShutdown, Watcher};
use kartenn_rest::transport::{ConnectionKill, ConnectionTls};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::crypto::{CryptoProvider, ring};
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::server::{ClientHello, ResolvesServerCert};
use tokio_rustls::rustls::sign::CertifiedKey;
use tokio_rustls::rustls::{ServerConfig, version};
use tower_service::Service as _;

use crate::config;

/// Longest TLS handshake: a client that opens a connection and stays silent is dropped.
pub const HANDSHAKE_WITHIN: Duration = Duration::from_secs(10);
/// `SETTINGS_MAX_CONCURRENT_STREAMS` announced in HTTP/2 (REQ-022-14).
pub const MAX_CONCURRENT_STREAMS: u32 = 200;
/// Longest wait for the requests in flight at the stop, within the 10 s of REQ-012-34.
pub const DRAIN_WITHIN: Duration = Duration::from_secs(5);

/// The certificate served, read again from its files on demand (`SIGHUP`): the new
/// connections take the new one, the open ones keep theirs.
#[derive(Debug)]
pub struct Certificates {
    files: config::Tls,
    provider: Arc<CryptoProvider>,
    current: ArcSwap<CertifiedKey>,
}

impl Certificates {
    /// Reads the certificate chain and its key.
    ///
    /// # Errors
    ///
    /// An unreadable file, no certificate, or a key that does not match the certificate.
    pub fn load(files: &config::Tls) -> anyhow::Result<Self> {
        let provider = Arc::new(ring::default_provider());
        let current = ArcSwap::from_pointee(certified(files, &provider)?);
        Ok(Self {
            files: files.clone(),
            provider,
            current,
        })
    }

    /// Reads the files again; on failure, the current certificate stays.
    ///
    /// # Errors
    ///
    /// As [`Certificates::load`].
    pub fn reload(&self) -> anyhow::Result<()> {
        self.current
            .store(Arc::new(certified(&self.files, &self.provider)?));
        Ok(())
    }

    /// TLS 1.2 and 1.3 with the `ring` provider, ALPN `h2` then `http/1.1`.
    ///
    /// # Errors
    ///
    /// None in practice: the versions are those of the provider.
    pub fn acceptor(self: &Arc<Self>) -> anyhow::Result<TlsAcceptor> {
        let mut config = ServerConfig::builder_with_provider(self.provider.clone())
            .with_protocol_versions(&[&version::TLS13, &version::TLS12])?
            .with_no_client_auth()
            .with_cert_resolver(self.clone());
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Ok(TlsAcceptor::from(Arc::new(config)))
    }
}

impl ResolvesServerCert for Certificates {
    fn resolve(&self, _: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.current.load_full())
    }
}

fn certified(files: &config::Tls, provider: &CryptoProvider) -> anyhow::Result<CertifiedKey> {
    let chain = CertificateDer::pem_file_iter(&files.cert_file)
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .with_context(|| format!("reading {}", files.cert_file.display()))?;
    anyhow::ensure!(
        !chain.is_empty(),
        "no certificate in {}",
        files.cert_file.display()
    );
    let key = PrivateKeyDer::from_pem_file(&files.key_file)
        .with_context(|| format!("reading {}", files.key_file.display()))?;
    CertifiedKey::from_der(chain, key, provider).context("the key and the certificate")
}

/// Serves `app` on `listener` until `shutdown`, over TLS when `tls` is given; then stops
/// accepting and waits for the open connections at most [`DRAIN_WITHIN`]. Each request
/// carries the peer address (`ConnectInfo`) and whether its connection is TLS
/// ([`ConnectionTls`]).
pub async fn serve(
    listener: TcpListener,
    app: Router,
    tls: Option<TlsAcceptor>,
    shutdown: impl Future<Output = ()>,
) {
    let mut builder = auto::Builder::new(TokioExecutor::new());
    builder.http1().timer(TokioTimer::new());
    // At least 100 concurrent streams on a connection (RFC 9113 § 6.5.2): a browser opens
    // its event stream beside its requests (spec 022, REQ-022-14).
    builder
        .http2()
        .timer(TokioTimer::new())
        .max_concurrent_streams(MAX_CONCURRENT_STREAMS);
    let builder = Arc::new(builder);
    let graceful = GracefulShutdown::new();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    let _ = stream.set_nodelay(true);
                    let connection = Connection {
                        builder: builder.clone(),
                        app: app.clone(),
                        peer,
                        watcher: graceful.watcher(),
                    };
                    let tls = tls.clone();
                    tokio::spawn(async move {
                        match tls {
                            None => connection.serve(stream, false).await,
                            Some(tls) => match tokio::time::timeout(HANDSHAKE_WITHIN, tls.accept(stream)).await {
                                Ok(Ok(stream)) => connection.serve(stream, true).await,
                                Ok(Err(e)) => tracing::debug!(name: "http.tls.failed", peer = %peer, error = %e, "TLS handshake failed"),
                                Err(_) => tracing::debug!(name: "http.tls.failed", peer = %peer, "TLS handshake too slow"),
                            },
                        }
                    });
                }
                // Out of descriptors, mostly: wait instead of spinning.
                Err(e) => {
                    tracing::warn!(name: "http.accept.failed", error = %e, "a connection could not be accepted");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            },
        }
    }
    drop(listener);
    if tokio::time::timeout(DRAIN_WITHIN, graceful.shutdown())
        .await
        .is_err()
    {
        tracing::warn!(name: "http.drain.timeout", "requests still in flight at the stop");
    }
}

struct Connection {
    builder: Arc<auto::Builder<TokioExecutor>>,
    app: Router,
    peer: SocketAddr,
    watcher: Watcher,
}

impl Connection {
    async fn serve<S>(self, stream: S, tls: bool)
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let Self {
            builder,
            app,
            peer,
            watcher,
        } = self;
        let kill = ConnectionKill::default();
        let killed = kill.0.clone();
        let service = hyper::service::service_fn(move |mut request: hyper::Request<Incoming>| {
            request.extensions_mut().insert(ConnectInfo(peer));
            request.extensions_mut().insert(ConnectionTls(tls));
            request.extensions_mut().insert(kill.clone());
            app.clone().call(request)
        });
        let connection = builder
            .serve_connection(TokioIo::new(stream), service)
            .into_owned();
        tokio::select! {
            result = watcher.watch(connection) => {
                if let Err(e) = result {
                    tracing::debug!(name: "http.connection.failed", peer = %peer, error = %e, "connection closed on an error");
                }
            }
            // An event stream whose client stopped reading (REQ-022-21): the socket goes.
            () = killed.notified() => {
                tracing::debug!(name: "http.connection.killed", peer = %peer, "connection closed: a stream could not write");
            }
        }
    }
}
