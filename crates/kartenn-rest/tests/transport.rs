//! Transport layers (spec 006, REQ-006-10, 27, 31; task 006-3): `Forwarded` believed
//! from the trusted proxies only, bounded body (413 announced and while reading, 415),
//! URL of 8 KiB at most (414), delay (503); on a real port (task 006-24), `QUERY` in
//! HTTP/1.1 and HTTP/2, clear and TLS, and `Forwarded` from the connection to the handler.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Extension};
use axum::http::{Request, Response, StatusCode, header};
use axum::routing::{get, post};
use http_body_util::BodyExt;
use kartenn_rest::filter::{ListQuery, collection};
use kartenn_rest::health::Readiness;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::{Client, ConnectionTls, MAX_URL, Settings};
use tower::ServiceExt;

mod common;
mod wire;

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

fn app(settings: Settings, called: Arc<AtomicBool>) -> Router {
    let api = Router::new()
        .route(
            "/client",
            get(|Extension(c): Extension<Client>| async move { format!("{} {}", c.addr, c.tls) }),
        )
        .route(
            "/echo",
            post(move |body: Bytes| {
                called.store(true, Ordering::SeqCst);
                async move { body.len().to_string() }
            }),
        )
        .route(
            "/list",
            collection(|ListQuery(q): ListQuery| async move { q }),
        )
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                "late"
            }),
        );
    kartenn_rest::router(
        api,
        Arc::new(Ready),
        settings,
        Arc::new(Quotas::new(Rates::default())),
        common::anyone(),
    )
}

fn settings() -> Settings {
    Settings {
        trusted_proxies: vec!["10.0.0.0/8".parse().unwrap()],
        max_body_bytes: 1024,
        request_timeout: Duration::from_millis(200),
        ..Settings::default()
    }
}

fn from(peer: &str, tls: bool, mut request: Request<Body>) -> Request<Body> {
    let peer: SocketAddr = format!("{peer}:40000").parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    request.extensions_mut().insert(ConnectionTls(tls));
    common::bearer(request)
}

async fn text(response: Response<Body>) -> String {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(body.to_vec()).unwrap()
}

async fn problem_type(response: Response<Body>) -> String {
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let body: serde_json::Value = serde_json::from_str(&text(response).await).unwrap();
    body["type"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn forwarded_is_believed_from_a_trusted_proxy_only() {
    let app = app(settings(), Arc::default());
    let client = |peer: &str, tls: bool, forwarded: &str| {
        from(
            peer,
            tls,
            Request::get("/v1/client")
                .header(header::FORWARDED, forwarded)
                .body(Body::empty())
                .unwrap(),
        )
    };
    let seen = |request| {
        let app = app.clone();
        async move { text(app.oneshot(request).await.unwrap()).await }
    };
    assert_eq!(
        seen(client("192.0.2.9", false, "for=198.51.100.1;proto=https")).await,
        "192.0.2.9 false",
        "an address not configured: its `Forwarded` is ignored, no TLS"
    );
    assert_eq!(
        seen(client("10.0.0.5", false, "for=198.51.100.1;proto=https")).await,
        "198.51.100.1 true"
    );
    assert_eq!(
        seen(client("10.0.0.5", true, "for=198.51.100.1;proto=http")).await,
        "198.51.100.1 false",
        "TLS to the proxy only: the client spoke clear"
    );
    assert_eq!(
        seen(client("192.0.2.9", true, "for=198.51.100.1;proto=http")).await,
        "192.0.2.9 true"
    );
}

#[tokio::test]
async fn a_body_over_the_limit_is_refused_announced_or_while_reading() {
    let called = Arc::new(AtomicBool::new(false));
    let app = app(settings(), called.clone());
    let post = |body: Body, length: Option<usize>| {
        let mut request = Request::post("/v1/echo");
        if let Some(length) = length {
            request = request.header(header::CONTENT_LENGTH, length);
        }
        from("192.0.2.9", true, request.body(body).unwrap())
    };

    let response = app
        .clone()
        .oneshot(post(Body::from(vec![0u8; 1025]), Some(1025)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(response.headers().contains_key("request-id"));
    assert_eq!(
        problem_type(response).await,
        "urn:kartenn:problem:payload-too-large"
    );
    assert!(
        !called.load(Ordering::SeqCst),
        "announced: refused before the route"
    );

    let chunks = || {
        Body::from_stream(futures_util::stream::iter(
            (0..3).map(|_| Ok::<_, std::io::Error>(Bytes::from(vec![0u8; 400]))),
        ))
    };
    let response = app.clone().oneshot(post(chunks(), None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        problem_type(response).await,
        "urn:kartenn:problem:payload-too-large"
    );

    let response = app
        .clone()
        .oneshot(post(Body::from(vec![0u8; 1024]), Some(1024)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(text(response).await, "1024", "at the limit, accepted");
}

#[tokio::test]
async fn a_content_coded_body_is_refused() {
    let app = app(settings(), Arc::default());
    let response = app
        .oneshot(from(
            "192.0.2.9",
            true,
            Request::post("/v1/echo")
                .header(header::CONTENT_ENCODING, "gzip")
                .body(Body::from("x"))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(response.headers()[header::ACCEPT_ENCODING], "identity");
    assert_eq!(
        problem_type(response).await,
        "urn:kartenn:problem:unsupported-media-type"
    );
}

#[tokio::test]
async fn a_url_over_8_kib_is_refused() {
    let app = app(settings(), Arc::default());
    let url = |len: usize| format!("/v1/client?q={}", "a".repeat(len - "/v1/client?q=".len()));
    let response = app
        .clone()
        .oneshot(from(
            "192.0.2.9",
            true,
            Request::get(url(MAX_URL + 1)).body(Body::empty()).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::URI_TOO_LONG);
    assert!(text(response).await.is_empty(), "minimal body");
    let response = app
        .oneshot(from(
            "192.0.2.9",
            true,
            Request::get(url(MAX_URL)).body(Body::empty()).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_request_over_the_delay_answers_503() {
    let app = app(settings(), Arc::default());
    let response = app
        .oneshot(from(
            "192.0.2.9",
            true,
            Request::get("/v1/slow").body(Body::empty()).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        problem_type(response).await,
        "urn:kartenn:problem:unavailable"
    );
}

/// REQ-006-10, 32: the same search by `GET` and by `QUERY`, over each transport; the
/// `QUERY` without `Content-Type` → 415, played again → the same answer.
#[tokio::test]
async fn query_crosses_a_local_port_in_http1_and_h2() {
    for tls in [false, true] {
        let server = wire::start(app(settings(), Arc::default()), tls).await;
        for version in wire::VERSIONS {
            let mut client = server.connect(version).await;
            let what = format!("{version:?}, TLS {tls}");
            let get = client
                .send(
                    client
                        .request("GET", "/v1/list?page_size=7")
                        .header(header::AUTHORIZATION, "Bearer test")
                        .body(wire::empty())
                        .unwrap(),
                )
                .await;
            assert_eq!(get.status(), StatusCode::OK, "{what}");
            assert!(get.headers().contains_key("accept-query"), "{what}");
            assert_eq!(get.body(), "page_size=7", "{what}");
            for _ in 0..2 {
                let query = client
                    .send(
                        client
                            .request("QUERY", "/v1/list")
                            .header(header::AUTHORIZATION, "Bearer test")
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(wire::body(r#"{"page_size":7}"#))
                            .unwrap(),
                    )
                    .await;
                assert_eq!(query.status(), StatusCode::OK, "{what}");
                assert!(query.headers().contains_key("accept-query"), "{what}");
                assert_eq!(query.body(), get.body(), "{what}");
            }
            let untyped = client
                .send(
                    client
                        .request("QUERY", "/v1/list")
                        .header(header::AUTHORIZATION, "Bearer test")
                        .body(wire::body(r#"{"page_size":7}"#))
                        .unwrap(),
                )
                .await;
            assert_eq!(
                untyped.status(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "{what}"
            );
        }
    }
}

/// REQ-006-27: the peer and the TLS of the connection reach the handler; `Forwarded` is
/// believed from the loopback once it is a trusted proxy, ignored otherwise.
#[tokio::test]
async fn forwarded_crosses_a_local_port() {
    let trusted = Settings {
        trusted_proxies: vec!["127.0.0.0/8".parse().unwrap()],
        ..settings()
    };
    for (settings, tls, forwarded, seen) in [
        (settings(), false, None, "127.0.0.1 false"),
        (settings(), true, None, "127.0.0.1 true"),
        (
            settings(),
            false,
            Some("for=198.51.100.1;proto=https"),
            "127.0.0.1 false",
        ),
        (
            trusted.clone(),
            false,
            Some("for=198.51.100.1;proto=https"),
            "198.51.100.1 true",
        ),
        (
            trusted.clone(),
            true,
            Some("for=198.51.100.1;proto=http"),
            "198.51.100.1 false",
        ),
        (trusted.clone(), false, None, "127.0.0.1 false"),
    ] {
        let server = wire::start(app(settings, Arc::default()), tls).await;
        for version in wire::VERSIONS {
            let mut client = server.connect(version).await;
            let mut request = client
                .request("GET", "/v1/client")
                .header(header::AUTHORIZATION, "Bearer test");
            if let Some(forwarded) = forwarded {
                request = request.header(header::FORWARDED, forwarded);
            }
            let response = client.send(request.body(wire::empty()).unwrap()).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.body(),
                seen,
                "{version:?}, TLS {tls}, {forwarded:?}"
            );
        }
    }
}
