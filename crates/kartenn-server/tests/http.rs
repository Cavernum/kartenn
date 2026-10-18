//! Serving the API over real TCP (spec 006, REQ-006-32; task 006-2): HTTP/2 chosen by
//! ALPN, HTTP/1.1, `h2c` behind a proxy, the certificate read again without cutting the
//! open connections, the stop within the 10 s of REQ-012-34.

use std::future::Future;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ConnectInfo;
use axum::routing::get;
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::client::conn::{http1, http2};
use hyper::{Request, StatusCode, Version};
use hyper_util::rt::{TokioExecutor, TokioIo};
use kartenn_rest::authn::{Authenticator, Proof, Refusal};
use kartenn_rest::health::Readiness;
use kartenn_server::config::Tls;
use kartenn_server::http::{Certificates, serve};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use sqlx::PgPool;
use sqlx::types::Uuid;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::crypto::ring;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

struct Cert {
    pem: String,
    key: String,
    der: CertificateDer<'static>,
}

fn cert() -> Cert {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    Cert {
        pem: certified.cert.pem(),
        key: certified.signing_key.serialize_pem(),
        der: certified.cert.der().clone(),
    }
}

fn dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kartenn-http-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, cert: &Cert) -> Tls {
    let files = Tls {
        cert_file: dir.join("tls.crt"),
        key_file: dir.join("tls.key"),
    };
    std::fs::write(&files.cert_file, &cert.pem).unwrap();
    std::fs::write(&files.key_file, &cert.key).unwrap();
    files
}

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

fn app() -> axum::Router {
    let api = axum::Router::new().route(
        "/peer",
        get(|ConnectInfo(peer): ConnectInfo<SocketAddr>| async move { peer.to_string() }),
    );
    kartenn_rest::router(
        api,
        Arc::new(Ready),
        kartenn_rest::transport::Settings::default(),
        Arc::new(kartenn_rest::quota::Quotas::new(
            kartenn_rest::quota::Rates::default(),
        )),
        Arc::new(Anyone),
    )
}

/// Any access token is the same person: the authentication is not the subject here.
struct Anyone;

impl Authenticator for Anyone {
    fn authenticate<'a>(
        &'a self,
        _: Proof,
        _: kartenn_core::audit::Source,
    ) -> Pin<Box<dyn Future<Output = Result<Principal, Refusal>> + Send + 'a>> {
        Box::pin(std::future::ready(Ok(Principal {
            identity: Uuid::nil(),
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })))
    }
}

/// Serves [`app`] on a local port until the sender is dropped or fires.
async fn start(
    certificates: Option<&Arc<Certificates>>,
) -> (
    SocketAddr,
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let tls = certificates.map(|c| c.acceptor().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(serve(listener, app(), tls, async {
        let _ = stopped.await;
    }));
    (address, stop, server)
}

async fn tls(
    address: SocketAddr,
    trusted: &CertificateDer<'static>,
    alpn: &[&[u8]],
) -> std::io::Result<TlsStream<TcpStream>> {
    let mut roots = RootCertStore::empty();
    roots.add(trusted.clone()).unwrap();
    let mut config = ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = alpn.iter().map(|p| p.to_vec()).collect();
    TlsConnector::from(Arc::new(config))
        .connect(
            ServerName::try_from("localhost").unwrap(),
            TcpStream::connect(address).await?,
        )
        .await
}

async fn h1<S>(io: S) -> http1::SendRequest<Empty<Bytes>>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (sender, connection) = http1::handshake(TokioIo::new(io)).await.unwrap();
    tokio::spawn(connection);
    sender
}

async fn h2<S>(io: S) -> http2::SendRequest<Empty<Bytes>>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (sender, connection) = http2::handshake(TokioExecutor::new(), TokioIo::new(io))
        .await
        .unwrap();
    tokio::spawn(connection);
    sender
}

fn get_h1(path: &str) -> Request<Empty<Bytes>> {
    Request::get(path)
        .header("host", "localhost")
        .header("authorization", "Bearer test")
        .body(Empty::new())
        .unwrap()
}

fn get_h2(path: &str) -> Request<Empty<Bytes>> {
    Request::get(format!("https://localhost{path}"))
        .header("authorization", "Bearer test")
        .body(Empty::new())
        .unwrap()
}

async fn text(response: hyper::Response<hyper::body::Incoming>) -> String {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(body.to_vec()).unwrap()
}

#[tokio::test]
async fn alpn_chooses_http2_or_http1_over_tls() {
    let dir = dir();
    let a = cert();
    let certificates = Arc::new(Certificates::load(&write(&dir, &a)).unwrap());
    let (address, _stop, _server) = start(Some(&certificates)).await;

    let stream = tls(address, &a.der, &[b"h2", b"http/1.1"]).await.unwrap();
    assert_eq!(stream.get_ref().1.alpn_protocol(), Some(&b"h2"[..]));
    let response = h2(stream)
        .await
        .send_request(get_h2("/health/live"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.version(), Version::HTTP_2);
    assert!(response.headers().contains_key("request-id"));

    let stream = tls(address, &a.der, &[b"http/1.1"]).await.unwrap();
    assert_eq!(stream.get_ref().1.alpn_protocol(), Some(&b"http/1.1"[..]));
    let mut sender = h1(stream).await;
    let response = sender.send_request(get_h1("/v1/peer")).await.unwrap();
    assert_eq!(response.version(), Version::HTTP_11);
    let peer: SocketAddr = text(response).await.parse().unwrap();
    assert!(
        peer.ip().is_loopback(),
        "the client address reaches the routes"
    );
}

#[tokio::test]
async fn clear_http_serves_http1_and_h2c_by_prior_knowledge() {
    let (address, _stop, _server) = start(None).await;
    let response = h2(TcpStream::connect(address).await.unwrap())
        .await
        .send_request(get_h2("/health/live"))
        .await
        .unwrap();
    assert_eq!(response.version(), Version::HTTP_2);
    assert_eq!(response.status(), StatusCode::OK);
    let response = h1(TcpStream::connect(address).await.unwrap())
        .await
        .send_request(get_h1("/health/live"))
        .await
        .unwrap();
    assert_eq!(response.version(), Version::HTTP_11);
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_certificate_is_read_again_without_cutting_the_open_connections() {
    let dir = dir();
    let (a, b) = (cert(), cert());
    let files = write(&dir, &a);
    let certificates = Arc::new(Certificates::load(&files).unwrap());
    let (address, _stop, _server) = start(Some(&certificates)).await;
    let mut open = h1(tls(address, &a.der, &[b"http/1.1"]).await.unwrap()).await;
    assert_eq!(
        open.send_request(get_h1("/health/live"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    write(&dir, &b);
    certificates.reload().unwrap();
    assert_eq!(
        open.send_request(get_h1("/health/live"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK,
        "the open connection goes on"
    );
    let stream = tls(address, &b.der, &[b"http/1.1"]).await.unwrap();
    assert_eq!(
        stream.get_ref().1.peer_certificates().unwrap()[0],
        b.der,
        "a new connection gets the new certificate"
    );
    assert!(tls(address, &a.der, &[b"http/1.1"]).await.is_err());

    std::fs::write(&files.key_file, "not a key").unwrap();
    assert!(certificates.reload().is_err());
    assert!(
        tls(address, &b.der, &[b"http/1.1"]).await.is_ok(),
        "a faulty certificate leaves the current one"
    );
}

#[tokio::test]
async fn a_key_that_does_not_match_the_certificate_is_refused() {
    let dir = dir();
    let (a, b) = (cert(), cert());
    let files = write(&dir, &a);
    std::fs::write(&files.key_file, &b.key).unwrap();
    let error = Certificates::load(&files).unwrap_err();
    assert!(format!("{error:#}").contains("key"), "{error:#}");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_instance_serves_the_api_reloads_on_sighup_and_stops_within_ten_seconds(pool: PgPool) {
    let dir = dir();
    let (a, b) = (cert(), cert());
    let files = write(&dir, &a);
    let address = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let config = kartenn_server::config::from_toml(&format!(
        r#"
        [telemetry]
        outputs = [{{ kind = "file", path = "{}", filter = "info" }}]
        [http]
        listen = "{address}"
        [http.tls]
        cert_file = "{}"
        key_file = "{}"
        "#,
        dir.join("technical.jsonl").display(),
        files.cert_file.display(),
        files.key_file.display(),
    ))
    .unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let run = tokio::spawn(async move {
        kartenn_server::app::run(pool, &config, async {
            let _ = stopped.await;
        })
        .await
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let stream = loop {
        match tls(address, &a.der, &[b"h2"]).await {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => panic!("the API never answered: {e}"),
        }
    };
    let mut open = h2(stream).await;
    let response = open.send_request(get_h2("/health/ready")).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "migrated base, no audit output"
    );

    write(&dir, &b);
    let status = std::process::Command::new("kill")
        .args(["-HUP", &std::process::id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let deadline = Instant::now() + Duration::from_secs(5);
    while tls(address, &b.der, &[b"h2"]).await.is_err() {
        assert!(Instant::now() < deadline, "SIGHUP did not reload");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let started = Instant::now();
    stop.send(()).unwrap();
    run.await.unwrap().unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "stopped in {:?}",
        started.elapsed()
    );
    assert!(
        TcpStream::connect(address).await.is_err(),
        "no longer listening"
    );
    assert!(
        open.send_request(get_h2("/health/live")).await.is_err(),
        "the idle connection was closed"
    );
}
