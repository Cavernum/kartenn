//! The router on a real port (task 006-24): served by `kartenn_server::http::serve`, in
//! clear or in TLS (`h2` and `http/1.1` chosen by ALPN), and a client in HTTP/1.1 or
//! HTTP/2 (`h2c` by prior knowledge in clear). The peer is `127.0.0.1`.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::http::{Request, Response, header, request};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::client::conn::{http1, http2};
use hyper_util::rt::{TokioExecutor, TokioIo};
use kartenn_server::config::Tls;
use kartenn_server::http::{Certificates, serve};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::crypto::ring;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Version {
    Http1,
    Http2,
}

pub(crate) const VERSIONS: [Version; 2] = [Version::Http1, Version::Http2];

impl Version {
    pub(crate) fn http(self) -> axum::http::Version {
        match self {
            Self::Http1 => axum::http::Version::HTTP_11,
            Self::Http2 => axum::http::Version::HTTP_2,
        }
    }

    fn alpn(self) -> &'static [u8] {
        match self {
            Self::Http1 => b"http/1.1",
            Self::Http2 => b"h2",
        }
    }
}

/// A router served until the server is dropped.
pub(crate) struct Server {
    address: SocketAddr,
    certificate: Option<CertificateDer<'static>>,
    _stop: oneshot::Sender<()>,
}

/// Serves `app` on a local port, in TLS with a self-signed certificate for `localhost`
/// or in clear.
pub(crate) async fn start(app: Router, tls: bool) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (acceptor, certificate) = if tls {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let dir = std::env::temp_dir().join(format!("kartenn-wire-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();
        let files = Tls {
            cert_file: dir.join("tls.crt"),
            key_file: dir.join("tls.key"),
        };
        std::fs::write(&files.cert_file, certified.cert.pem()).unwrap();
        std::fs::write(&files.key_file, certified.signing_key.serialize_pem()).unwrap();
        let certificates = Arc::new(Certificates::load(&files).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
        (
            Some(certificates.acceptor().unwrap()),
            Some(certified.cert.der().clone()),
        )
    } else {
        (None, None)
    };
    let (stop, stopped) = oneshot::channel::<()>();
    tokio::spawn(serve(listener, app, acceptor, async {
        let _ = stopped.await;
    }));
    Server {
        address,
        certificate,
        _stop: stop,
    }
}

enum Sender {
    Http1(http1::SendRequest<Full<Bytes>>),
    Http2(http2::SendRequest<Full<Bytes>>),
}

/// One connection to a [`Server`].
pub(crate) struct Client {
    sender: Sender,
    version: Version,
    tls: bool,
}

impl Server {
    /// A new connection in `version`; in TLS, the version is the one ALPN chose.
    pub(crate) async fn connect(&self, version: Version) -> Client {
        let tcp = TcpStream::connect(self.address).await.unwrap();
        let sender = match &self.certificate {
            None => handshake(tcp, version).await,
            Some(certificate) => {
                let mut roots = RootCertStore::empty();
                roots.add(certificate.clone()).unwrap();
                let mut config =
                    ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
                        .with_safe_default_protocol_versions()
                        .unwrap()
                        .with_root_certificates(roots)
                        .with_no_client_auth();
                config.alpn_protocols = vec![version.alpn().to_vec()];
                let stream = TlsConnector::from(Arc::new(config))
                    .connect(ServerName::try_from("localhost").unwrap(), tcp)
                    .await
                    .unwrap();
                assert_eq!(stream.get_ref().1.alpn_protocol(), Some(version.alpn()));
                handshake(stream, version).await
            }
        };
        Client {
            sender,
            version,
            tls: self.certificate.is_some(),
        }
    }
}

async fn handshake<S>(io: S, version: Version) -> Sender
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    match version {
        Version::Http1 => {
            let (sender, connection) = http1::handshake(TokioIo::new(io)).await.unwrap();
            tokio::spawn(connection);
            Sender::Http1(sender)
        }
        Version::Http2 => {
            let (sender, connection) = http2::handshake(TokioExecutor::new(), TokioIo::new(io))
                .await
                .unwrap();
            tokio::spawn(connection);
            Sender::Http2(sender)
        }
    }
}

impl Client {
    pub(crate) fn version(&self) -> Version {
        self.version
    }

    /// A request for `path`: `Host` in HTTP/1.1, the authority of the URI in HTTP/2.
    pub(crate) fn request(&self, method: &str, path: &str) -> request::Builder {
        let builder = Request::builder().method(method);
        match self.version {
            Version::Http1 => builder.uri(path).header(header::HOST, "localhost"),
            Version::Http2 => {
                let scheme = if self.tls { "https" } else { "http" };
                builder.uri(format!("{scheme}://localhost{path}"))
            }
        }
    }

    /// Sends `request` and reads the whole response.
    pub(crate) async fn send(&mut self, request: Request<Full<Bytes>>) -> Response<Bytes> {
        let response = match &mut self.sender {
            Sender::Http1(sender) => {
                sender.ready().await.unwrap();
                sender.send_request(request).await
            }
            Sender::Http2(sender) => {
                sender.ready().await.unwrap();
                sender.send_request(request).await
            }
        }
        .unwrap();
        assert_eq!(response.version(), self.version.http());
        let (parts, body) = response.into_parts();
        Response::from_parts(parts, body.collect().await.unwrap().to_bytes())
    }
}

pub(crate) fn empty() -> Full<Bytes> {
    Full::new(Bytes::new())
}

pub(crate) fn body(bytes: impl Into<Bytes>) -> Full<Bytes> {
    Full::new(bytes.into())
}
