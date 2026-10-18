//! Compression (spec 006, REQ-006-32; task 006-5): a list over 1 KiB compressed after
//! `Accept-Encoding`, a small response left as is, a response marked `Secret` never
//! compressed and `no-store`.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use http_body_util::BodyExt;
use kartenn_rest::compression::Secret;
use kartenn_rest::health::Readiness;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use serde_json::{Value, json};
use tower::ServiceExt;

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

fn list() -> Value {
    let items: Vec<Value> = (0..100)
        .map(|n| json!({ "id": n, "name": format!("prefix-{n}") }))
        .collect();
    json!({ "items": items })
}

fn app() -> Router {
    let api = Router::new()
        .route("/list", get(|| async { Json(list()) }))
        .route("/small", get(|| async { Json(json!({ "id": 1 })) }))
        .route("/secret", get(|| async { (Secret, Json(list())) }));
    kartenn_rest::router(
        api,
        Arc::new(Ready),
        Settings::default(),
        Arc::new(Quotas::new(Rates::default())),
    )
}

async fn get_with(uri: &str, accept: &str) -> Response {
    let mut request = Request::get(uri)
        .header(header::ACCEPT_ENCODING, accept)
        .body(Body::empty())
        .unwrap();
    let peer = SocketAddr::from(([192, 0, 2, 1], 40000));
    request.extensions_mut().insert(ConnectInfo(peer));
    app().oneshot(request).await.unwrap()
}

async fn length(response: Response) -> usize {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .len()
}

#[tokio::test]
async fn a_list_is_compressed_after_accept_encoding() {
    let plain = serde_json::to_vec(&list()).unwrap().len();
    assert!(plain > 1024);
    for (accept, coding) in [
        ("zstd", Some("zstd")),
        ("br", Some("br")),
        ("gzip", Some("gzip")),
        ("gzip;q=0.5, br", Some("br")),
        ("identity", None),
        ("deflate", None),
    ] {
        let response = get_with("/v1/list", accept).await;
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers().clone();
        assert_eq!(
            headers
                .get(header::CONTENT_ENCODING)
                .map(|v| v.to_str().unwrap()),
            coding,
            "{accept}"
        );
        if coding.is_some() {
            assert_eq!(headers[header::VARY], "accept-encoding");
            assert!(length(response).await < plain, "{accept}");
        } else {
            assert_eq!(length(response).await, plain, "{accept}");
        }
    }
}

#[tokio::test]
async fn a_response_of_1_kib_or_less_is_not_compressed() {
    let response = get_with("/v1/small", "gzip").await;
    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
}

#[tokio::test]
async fn a_secret_is_never_compressed_and_never_stored() {
    for accept in ["zstd", "br", "gzip"] {
        let response = get_with("/v1/secret", accept).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            !response.headers().contains_key(header::CONTENT_ENCODING),
            "{accept}"
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            length(response).await,
            serde_json::to_vec(&list()).unwrap().len()
        );
    }
}
