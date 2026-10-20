//! Quotas and load shedding (spec 006, REQ-006-29, 30; task 006-4): 429 and its headers
//! past a burst, 503 at the ceiling of requests in flight without reaching the route, a
//! table of keys bounded under varied keys.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::response::Response;
use axum::routing::{get, post};
use kartenn_rest::health::Readiness;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::{Client, Settings};
use kartenn_service::audit::RequestContext;
use tokio::sync::Notify;
use tower::ServiceExt;

mod common;
use uuid::Uuid;

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

fn from(peer: &str, method: Method, uri: &str) -> Request {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let peer = SocketAddr::new(peer.parse().unwrap(), 40000);
    request.extensions_mut().insert(ConnectInfo(peer));
    common::bearer(request)
}

/// The quota layer alone, behind an identity as the authentication (task 6) will set it.
fn behind(identity: Uuid, quotas: Arc<Quotas>) -> Router {
    let authenticated = move |mut request: Request, next: Next| async move {
        request
            .extensions_mut()
            .insert(RequestContext::person(identity));
        request.extensions_mut().insert(Client {
            addr: "192.0.2.1".parse().unwrap(),
            tls: true,
        });
        next.run(request).await
    };
    Router::new()
        .route("/v1/w", post(|| async { "ok" }).get(|| async { "ok" }))
        .layer(from_fn_with_state(quotas, kartenn_rest::quota::layer))
        .layer(from_fn(authenticated))
}

async fn status(app: &Router, request: Request) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

#[tokio::test]
async fn the_21st_write_of_a_burst_is_refused_with_its_headers() {
    let app = behind(Uuid::now_v7(), Arc::new(Quotas::new(Rates::default())));
    for n in 1..=20 {
        let response = status(&app, from("192.0.2.1", Method::POST, "/v1/w")).await;
        assert_eq!(response.status(), StatusCode::OK, "write {n}");
    }
    let response = status(&app, from("192.0.2.1", Method::POST, "/v1/w")).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let headers = response.headers();
    assert_eq!(
        headers[header::RETRY_AFTER],
        "1",
        "10/s: next token within 100 ms"
    );
    assert_eq!(headers["ratelimit-policy"], "\"w\";q=20;w=1");
    assert_eq!(headers["ratelimit"], "\"w\";r=0;t=1");
    assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
    assert_eq!(
        status(&app, from("192.0.2.1", Method::GET, "/v1/w"))
            .await
            .status(),
        StatusCode::OK,
        "reads are counted apart"
    );
}

#[tokio::test]
async fn without_identity_the_client_address_is_counted() {
    let app = kartenn_rest::router(
        Router::new(),
        Arc::new(Ready),
        Settings::default(),
        Arc::new(Quotas::new(Rates::default())),
        common::anyone(),
    );
    // The login route, anonymous (in clear, it answers 400: the quota is what counts).
    for _ in 0..20 {
        let response = status(&app, from("192.0.2.1", Method::GET, "/v1/auth/login")).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = status(&app, from("192.0.2.1", Method::GET, "/v1/auth/login")).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["ratelimit-policy"], "\"a\";q=20;w=1");
    assert!(response.headers().contains_key("request-id"));
    let other = status(&app, from("192.0.2.2", Method::GET, "/v1/auth/login")).await;
    assert_eq!(other.status(), StatusCode::BAD_REQUEST, "another address");
}

#[tokio::test]
async fn at_the_ceiling_the_request_is_shed_without_reaching_the_route() {
    let release = Arc::new(Notify::new());
    let entered = Arc::new(Notify::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let api = Router::new().route(
        "/slow",
        get({
            let (release, entered, calls) = (release.clone(), entered.clone(), calls.clone());
            move || async move {
                calls.fetch_add(1, Ordering::SeqCst);
                entered.notify_one();
                release.notified().await;
                "done"
            }
        }),
    );
    let app = kartenn_rest::router(
        api,
        Arc::new(Ready),
        Settings {
            max_in_flight: 1,
            ..Settings::default()
        },
        Arc::new(Quotas::new(Rates::default())),
        common::anyone(),
    );
    let first = tokio::spawn(
        app.clone()
            .oneshot(from("192.0.2.1", Method::GET, "/v1/slow")),
    );
    entered.notified().await;

    let response = status(&app, from("192.0.2.2", Method::GET, "/v1/slow")).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()[header::RETRY_AFTER], "1");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the route (and its SQL) not reached"
    );

    release.notify_one();
    assert_eq!(first.await.unwrap().unwrap().status(), StatusCode::OK);
    release.notify_one();
    let response = status(&app, from("192.0.2.2", Method::GET, "/v1/slow")).await;
    assert_eq!(response.status(), StatusCode::OK, "free again");
}

#[tokio::test]
async fn the_table_of_keys_stays_under_its_ceiling() {
    let quotas = Arc::new(Quotas::with_max_keys(Rates::default(), 100));
    let app = kartenn_rest::router(
        Router::new(),
        Arc::new(Ready),
        Settings::default(),
        quotas.clone(),
        common::anyone(),
    );
    for n in 0..1000u32 {
        let [_, _, c, d] = (0x0A00_0000 | n).to_be_bytes();
        let peer = format!("2001:db8:{c:x}:{d:x}::1");
        let response = status(&app, from(&peer, Method::GET, "/v1/auth/login")).await;
        assert_ne!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let [_, _, anonymous] = quotas.keys();
        assert!(anonymous <= 100, "{anonymous} keys after {n}");
    }
    assert_eq!(quotas.keys()[2], 100, "the overflow key holds the rest");
}
