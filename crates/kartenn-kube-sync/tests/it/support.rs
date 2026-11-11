//! What the tests share: a cluster with a token, a local TLS API server, a capture of
//! every trace and log.

use std::fmt::Write as _;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use kartenn_kube_sync::Cluster;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// A cluster whose token is in `dir/token`, holding `token`.
pub(crate) fn cluster(dir: &Path, api_server: &str, token: &str) -> Cluster {
    let token_file = dir.join("token");
    std::fs::write(&token_file, format!("{token}\n")).unwrap();
    Cluster {
        slug: "lab".into(),
        api_server: api_server.into(),
        ca_file: Some(dir.join("ca.crt")),
        token_file: Some(token_file),
        cert_file: None,
        key_file: None,
        fallback_period_s: 300,
        allow_excess_rights: false,
    }
}

/// A self-signed certificate for `127.0.0.1` and `localhost`, its PEM written to
/// `dir/<name>.crt`.
pub(crate) fn certificate(
    dir: &Path,
    name: &str,
) -> (rcgen::CertifiedKey<rcgen::KeyPair>, PathBuf) {
    let certified =
        rcgen::generate_simple_self_signed(vec!["127.0.0.1".into(), "localhost".into()]).unwrap();
    let file = dir.join(format!("{name}.crt"));
    std::fs::write(&file, certified.cert.pem()).unwrap();
    (certified, file)
}

/// An API server over TLS on the loopback, that answers an empty list of namespaces and
/// keeps the `Authorization` headers it receives.
pub(crate) struct TlsServer {
    pub(crate) address: SocketAddr,
    pub(crate) authorizations: Arc<Mutex<Vec<String>>>,
}

pub(crate) async fn tls_server(certified: &rcgen::CertifiedKey<rcgen::KeyPair>) -> TlsServer {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let key =
        rustls::pki_types::PrivateKeyDer::try_from(certified.signing_key.serialize_der()).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![certified.cert.der().clone()], key)
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let authorizations = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&authorizations);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            let seen = Arc::clone(&seen);
            tokio::spawn(async move {
                let Ok(stream) = acceptor.accept(stream).await else {
                    return;
                };
                let service = service_fn(move |request: http::Request<hyper::body::Incoming>| {
                    let seen = Arc::clone(&seen);
                    async move {
                        if let Some(value) = request.headers().get(http::header::AUTHORIZATION) {
                            seen.lock()
                                .unwrap()
                                .push(value.to_str().unwrap().to_owned());
                        }
                        let body = r#"{"apiVersion":"v1","kind":"NamespaceList","metadata":{"resourceVersion":"7"},"items":[]}"#;
                        Ok::<_, std::convert::Infallible>(
                            http::Response::builder()
                                .header(http::header::CONTENT_TYPE, "application/json")
                                .body(Full::new(Bytes::from_static(body.as_bytes())))
                                .unwrap(),
                        )
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });
    TlsServer {
        address,
        authorizations,
    }
}

/// Every field of every span and event, written as `name=value`.
#[derive(Clone, Default)]
pub(crate) struct Capture(Arc<Mutex<String>>);

impl Capture {
    pub(crate) fn text(&self) -> String {
        self.0.lock().unwrap().clone()
    }
}

struct Writer<'a>(&'a Mutex<String>);

impl Visit for Writer<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let mut text = self.0.lock().unwrap();
        let _ = write!(text, "{}={value:?} ", field.name());
    }
}

impl<S> Layer<S> for Capture
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attributes: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _ctx: Context<'_, S>,
    ) {
        attributes.record(&mut Writer(&self.0));
    }

    fn on_record(
        &self,
        _id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        _ctx: Context<'_, S>,
    ) {
        values.record(&mut Writer(&self.0));
    }

    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut writer = Writer(&self.0);
        event.record(&mut writer);
        self.0.lock().unwrap().push('\n');
    }
}
