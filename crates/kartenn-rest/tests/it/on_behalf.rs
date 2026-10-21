//! The header `Kartenn-On-Behalf-Of` in the authentication layer (spec 016, REQ-016-12;
//! task 016-12): a service account the authenticator lets act names its requester in the
//! context of the request; on a read, malformed, repeated or from a person: refused.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Extension;
use axum::http::{Request, StatusCode, header};
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use axum::{Json, Router};
use http_body_util::BodyExt;
use kartenn_core::audit::Source;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::{IdentityKind, Method, Principal, Requester};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

/// Any `Bearer` token is a principal of `kind`; the requester `jdoe` is `REQUESTER`.
struct Connector(IdentityKind);

const REQUESTER: Uuid = Uuid::from_u128(0x0199_0000_0000_7000_8000_0000_0000_0042);

impl Authenticator for Connector {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(Principal {
                identity: Uuid::now_v7(),
                kind: self.0,
                issuer: None,
                groups: Vec::new(),
                method: Method::Bearer,
            }
            .into()),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }

    fn on_behalf_of(&self, _: Principal, requester: Requester, _: Source) -> Answer<'_, Uuid> {
        let answer = if requester.sub == "jdoe" {
            Ok(REQUESTER)
        } else {
            Err(Refusal::OnBehalfForbidden)
        };
        Box::pin(std::future::ready(answer))
    }
}

/// A route that answers the requester of its context, behind the authentication layer.
fn app(kind: IdentityKind) -> Router {
    let authenticator: Arc<dyn Authenticator> = Arc::new(Connector(kind));
    let echo = |Extension(context): Extension<RequestContext>| async move {
        Json(json!({ "on_behalf_of": context.on_behalf_of }))
    };
    Router::new()
        .route("/v1/echo", get(echo).post(echo))
        .layer(from_fn_with_state(
            authenticator,
            kartenn_rest::authn::layer,
        ))
}

async fn send(app: Router, method: &str, value: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri("/v1/echo")
        .header(header::AUTHORIZATION, "Bearer test");
    if let Some(value) = value {
        request = request.header("Kartenn-On-Behalf-Of", value);
    }
    let response = app
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn a_service_account_writes_for_its_requester() {
    let (status, body) = send(
        app(IdentityKind::ServiceAccount),
        "POST",
        Some(r#"sub="jdoe", iss="https://tickets.example""#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["on_behalf_of"], REQUESTER.to_string());
    // Without the header, no requester.
    let (status, body) = send(app(IdentityKind::ServiceAccount), "POST", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["on_behalf_of"].is_null());
}

#[tokio::test]
async fn the_header_is_refused_where_it_does_not_apply() {
    // A read: 400.
    let (status, _) = send(app(IdentityKind::ServiceAccount), "GET", Some("sub=jdoe")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Malformed: 400.
    let (status, _) = send(app(IdentityKind::ServiceAccount), "POST", Some("iss=x")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // A person: 403 `core.identity.act_on_behalf`.
    let (status, body) = send(app(IdentityKind::Person), "POST", Some("sub=jdoe")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["detail"], "core.identity.act_on_behalf");
    // A service account the authenticator refuses: 403.
    let (status, _) = send(app(IdentityKind::ServiceAccount), "POST", Some("sub=other")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
