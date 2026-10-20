//! An instance against the development issuer, end to end on local ports (spec 007,
//! REQ-007-35; task 007-9): kartenn-server trusts the development authority through
//! `SSL_CERT_FILE`, the browser is played by hand (choice page, form, callback), the
//! groups of the TOML reach `/v1/me`, a user whose login fails is audited.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper::{Request, StatusCode, header};
use hyper_util::rt::TokioIo;
use kartenn_dev_idp::certs;
use kartenn_dev_idp::config::Config;
use kartenn_dev_idp::server::Server;
use sqlx::PgPool;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::crypto::ring;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

/// A response, read whole.
struct Answer {
    status: StatusCode,
    location: Option<String>,
    cookies: Vec<String>,
    body: String,
}

/// A client of the loopback that trusts the development authority only.
struct Browser {
    tls: TlsConnector,
}

impl Browser {
    fn new(ca: &Path) -> Self {
        let mut roots = RootCertStore::empty();
        for cert in CertificateDer::pem_file_iter(ca).unwrap() {
            roots.add(cert.unwrap()).unwrap();
        }
        let config = ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        Self {
            tls: TlsConnector::from(Arc::new(config)),
        }
    }

    async fn send(
        &self,
        address: SocketAddr,
        method: &str,
        target: &str,
        cookies: &[String],
        form: Option<String>,
    ) -> Answer {
        let stream = self
            .tls
            .connect(
                ServerName::try_from("127.0.0.1").unwrap(),
                TcpStream::connect(address).await.unwrap(),
            )
            .await
            .unwrap();
        let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
        tokio::spawn(connection);
        let mut request = Request::builder()
            .method(method)
            .uri(target)
            .header(header::HOST, address.to_string());
        if !cookies.is_empty() {
            request = request.header(header::COOKIE, cookies.join("; "));
        }
        if form.is_some() {
            request = request.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        }
        let response = sender
            .send_request(
                request
                    .body(Full::new(Bytes::from(form.unwrap_or_default())))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let location = response
            .headers()
            .get(header::LOCATION)
            .map(|l| l.to_str().unwrap().to_owned());
        let cookies = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|c| c.to_str().unwrap().split(';').next().unwrap().to_owned())
            .filter(|c| !c.ends_with('='))
            .collect();
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        Answer {
            status,
            location,
            cookies,
            body,
        }
    }
}

/// The instance, stopped when the test ends, even by a failure.
struct Instance(std::process::Child);

impl Drop for Instance {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The path and query of an absolute URL.
fn target(url: &str) -> &str {
    let rest = url.split_once("://").unwrap().1;
    &rest[rest.find('/').unwrap()..]
}

fn query(url: &str) -> Vec<(String, String)> {
    form_urlencoded::parse(url.split_once('?').unwrap().1.as_bytes())
        .into_owned()
        .collect()
}

fn dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kartenn-dev-idp-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The URL of the throwaway database of `pool`.
fn database_url(pool: &PgPool) -> String {
    let base = std::env::var("DATABASE_URL").unwrap();
    let server = &base[..base.rfind('/').unwrap()];
    format!(
        "{server}/{}",
        pool.connect_options().get_database().unwrap()
    )
}

/// Plays a login as `user` up to the callback, and returns its answer.
async fn login(browser: &Browser, kartenn: SocketAddr, idp: SocketAddr, user: &str) -> Answer {
    let start = browser
        .send(kartenn, "GET", "/v1/auth/login?issuer=dev", &[], None)
        .await;
    assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
    let authorize = start.location.unwrap();
    let page = browser
        .send(idp, "GET", target(&authorize), &[], None)
        .await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("Alice Exemple"), "{}", page.body);
    let mut form = form_urlencoded::Serializer::new(String::new());
    form.extend_pairs(query(&authorize));
    form.append_pair("user", user);
    let chosen = browser
        .send(idp, "POST", "/authorize", &[], Some(form.finish()))
        .await;
    assert_eq!(chosen.status, StatusCode::SEE_OTHER, "{}", chosen.body);
    let callback = chosen.location.unwrap();
    assert!(
        query(&callback)
            .iter()
            .any(|(k, v)| k == "iss" && v == &format!("https://{idp}")),
        "RFC 9207: {callback}"
    );
    browser
        .send(kartenn, "GET", target(&callback), &start.cookies, None)
        .await
}

#[test]
fn the_development_configuration_is_valid() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dev/kartenn.toml");
    let config = kartenn_server::config::load(Some(&file)).unwrap();
    assert_eq!(
        config.auth.unwrap().issuers[0].issuer,
        "https://127.0.0.1:8444"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_instance_logs_in_through_the_development_issuer(pool: PgPool) {
    let dir = dir();
    certs::generate(&dir).unwrap();
    let idp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let idp = idp_listener.local_addr().unwrap();
    let kartenn = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let callback = format!("https://{kartenn}/v1/auth/callback");
    let issuer_file = dir.join("idp.toml");
    std::fs::write(
        &issuer_file,
        format!(
            r#"
        issuer = "https://{idp}"
        listen = "{idp}"
        client_id = "kartenn"
        client_secret_file = "{secret}"
        redirect_uris = ["{callback}"]
        cert_file = "{cert}"
        key_file = "{key}"

        [[users]]
        sub = "alice"
        name = "Alice Exemple"
        groups = ["kartenn-admins", "lecteurs"]

        [[users]]
        sub = "refuse"
        name = "Refus d'accès"
        error = "access_denied"
        "#,
            secret = dir.join(certs::SECRET).display(),
            cert = dir.join(certs::CERT).display(),
            key = dir.join(certs::KEY).display(),
        ),
    )
    .unwrap();
    let server = Server::new(Config::load(&issuer_file).unwrap()).unwrap();
    tokio::spawn(server.serve(idp_listener, std::future::pending()));

    let file = dir.join("kartenn.toml");
    std::fs::write(
        &file,
        format!(
            r#"
            [telemetry]
            outputs = [{{ kind = "stdout", filter = "off" }}]
            [http]
            listen = "{kartenn}"
            [http.tls]
            cert_file = "{cert}"
            key_file = "{key}"
            [auth]
            key_file = "{keys}"
            redirect_uri = "{callback}"
            [[auth.issuers]]
            id = "dev"
            issuer = "https://{idp}"
            client_id = "kartenn"
            client_secret_file = "{secret}"
            audience = "kartenn"
            algorithms = ["ES256"]
            "#,
            cert = dir.join(certs::CERT).display(),
            key = dir.join(certs::KEY).display(),
            keys = dir.join(certs::KEYS).display(),
            secret = dir.join(certs::SECRET).display(),
        ),
    )
    .unwrap();
    let mut instance = Instance(
        std::process::Command::new(env!("CARGO_BIN_EXE_kartenn-server"))
            .arg("run")
            .arg(database_url(&pool))
            .arg(&file)
            .env("SSL_CERT_FILE", dir.join(certs::CA))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let mut up = false;
    for _ in 0..300 {
        if TcpStream::connect(kartenn).await.is_ok() {
            up = true;
            break;
        }
        assert!(
            instance.0.try_wait().unwrap().is_none(),
            "the instance stopped"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(up, "the instance listens");

    let browser = Browser::new(&dir.join(certs::CA));
    let done = login(&browser, kartenn, idp, "alice").await;
    assert_eq!(done.status, StatusCode::SEE_OTHER, "{}", done.body);
    assert!(!done.cookies.is_empty(), "a session cookie");
    let me = browser
        .send(kartenn, "GET", "/v1/me", &done.cookies, None)
        .await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.body);
    let me: serde_json::Value = serde_json::from_str(&me.body).unwrap();
    assert_eq!(me["method"], "oidc", "{me}");
    assert_eq!(
        me["groups"],
        serde_json::json!(["kartenn-admins", "lecteurs"]),
        "{me}"
    );

    let refused = login(&browser, kartenn, idp, "refuse").await;
    assert!(refused.status.is_client_error(), "{}", refused.status);
    let mut audited = 0_i64;
    for _ in 0..50 {
        audited = sqlx::query_scalar(
            "SELECT count(*) FROM audit.event
             WHERE name = 'authn.login.failed'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if audited > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(audited, 1, "authn.login.failed");

    drop(instance);
    std::fs::remove_dir_all(dir).unwrap();
}
