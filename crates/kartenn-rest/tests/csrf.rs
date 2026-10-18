//! The CSRF check (spec 006, REQ-006-28; task 006-8): through the router, an unsafe method
//! of a cookie session from another origin → 403; the safe methods, `QUERY`, and the
//! requests by `Bearer` pass whatever their headers.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use axum::routing::any;
use http_body_util::BodyExt;
use kartenn_core::audit::Source;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::health::Readiness;
use kartenn_rest::quota::{Quotas, Rate, Rates};
use kartenn_rest::transport::{ConnectionTls, Settings};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

const PUBLIC: &str = "https://kartenn.example.invalid";

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

/// Any session cookie is an OIDC session, any `Bearer` token an access token.
struct Sessions;

impl Authenticator for Sessions {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let method = match proof {
            Proof::Session(_) => Ok(Method::OidcSession),
            Proof::Bearer(_) => Ok(Method::Bearer),
            Proof::Basic(_) => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(method.map(|method| {
            Principal {
                identity: Uuid::nil(),
                kind: IdentityKind::Person,
                issuer: None,
                groups: Vec::new(),
                method,
            }
            .into()
        })))
    }
}

fn app() -> Router {
    kartenn_rest::router(
        Router::new().route("/thing", any(|| async { StatusCode::NO_CONTENT })),
        Arc::new(Ready),
        Settings {
            public_origin: Some(PUBLIC.into()),
            ..Settings::default()
        },
        // The 60 writes of the matrix come at once.
        Arc::new(Quotas::new(Rates {
            write: Rate::new(1000, 1000).unwrap(),
            ..Rates::default()
        })),
        Arc::new(Sessions),
    )
}

enum By {
    Cookie,
    Bearer,
}

fn request(method: &str, by: &By, site: Option<&str>, origin: Option<&str>) -> Request<Body> {
    let mut request = Request::builder().method(method).uri("/v1/thing");
    request = match by {
        By::Cookie => request.header(header::COOKIE, "__Host-kartenn=s"),
        By::Bearer => request.header(header::AUTHORIZATION, "Bearer t"),
    };
    if let Some(site) = site {
        request = request.header("sec-fetch-site", site);
    }
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    let mut request = request.body(Body::empty()).unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo::<std::net::SocketAddr>(
            "192.0.2.1:40000".parse().unwrap(),
        ));
    request.extensions_mut().insert(ConnectionTls(true));
    request
}

const SITES: [Option<&str>; 5] = [
    Some("same-origin"),
    Some("same-site"),
    Some("cross-site"),
    Some("none"),
    None,
];
const ORIGINS: [Option<&str>; 3] = [Some(PUBLIC), Some("https://other.example.invalid"), None];

/// REQ-006-28: for each unsafe method, only `same-origin`, and an equal `Origin` without
/// `Sec-Fetch-Site`, pass; the refusal is the problem `cross-site-request`.
#[tokio::test]
async fn unsafe_methods_by_cookie() {
    let app = app();
    for method in ["POST", "PUT", "PATCH", "DELETE"] {
        for site in SITES {
            for origin in ORIGINS {
                let response = app
                    .clone()
                    .oneshot(request(method, &By::Cookie, site, origin))
                    .await
                    .unwrap();
                let passes =
                    site == Some("same-origin") || (site.is_none() && origin == Some(PUBLIC));
                let case = format!("{method} {site:?} × {origin:?}");
                if passes {
                    assert_eq!(response.status(), StatusCode::NO_CONTENT, "{case}");
                } else {
                    assert_eq!(response.status(), StatusCode::FORBIDDEN, "{case}");
                    let body = response.into_body().collect().await.unwrap().to_bytes();
                    let problem: Value = serde_json::from_slice(&body).unwrap();
                    assert_eq!(
                        problem["type"], "urn:kartenn:problem:cross-site-request",
                        "{case}"
                    );
                }
            }
        }
    }
}

/// The safe methods and `QUERY` by cookie, and every method by `Bearer`, are not
/// concerned.
#[tokio::test]
async fn safe_methods_and_bearer_not_concerned() {
    let app = app();
    let cases = ["GET", "HEAD", "OPTIONS", "QUERY"]
        .map(|m| (m, By::Cookie))
        .into_iter()
        .chain(["POST", "PUT", "PATCH", "DELETE", "QUERY"].map(|m| (m, By::Bearer)));
    for (method, by) in cases {
        let response = app
            .clone()
            .oneshot(request(
                method,
                &by,
                Some("cross-site"),
                Some("https://other.example.invalid"),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT, "{method}");
    }
}
