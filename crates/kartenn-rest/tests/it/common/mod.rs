//! What the tests of the layers share: an authenticator that accepts any access token,
//! for the tests whose subject is not the authentication.

#![allow(dead_code)] // each test binary uses its own part of it

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::response::Response;
use sqlx::PgPool;
use tower::ServiceExt;

use axum::http::{HeaderValue, Request, header};
use kartenn_core::audit::Source;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use uuid::Uuid;

/// Any `Bearer` token is the same person; any other proof is refused.
pub(crate) struct Anyone(pub Uuid);

impl Authenticator for Anyone {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(Principal {
                identity: self.0,
                kind: IdentityKind::Person,
                issuer: Some("https://idp.example.invalid".into()),
                groups: Vec::new(),
                method: Method::Bearer,
            }
            .into()),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }
}

/// Any `Bearer` token is this principal; any other proof is refused.
pub(crate) struct As(pub Principal);

impl Authenticator for As {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(self.0.clone().into()),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }
}

/// [`Anyone`], as the router takes it.
pub(crate) fn anyone() -> Arc<dyn Authenticator> {
    Arc::new(Anyone(Uuid::now_v7()))
}

/// `request` as a test writes: applied by its author when it may (`approval=self`, spec
/// 024, REQ-024-13), unless it names `approval` itself.
pub(crate) fn applied<B>(mut request: Request<B>) -> Request<B> {
    let uri =
        kartenn_testing::applied_by_author(request.method().as_str(), &request.uri().to_string());
    *request.uri_mut() = uri.parse().expect("a URI");
    request
}

/// `request` with an access token.
pub(crate) fn bearer<B>(mut request: Request<B>) -> Request<B> {
    request.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer test"),
    );
    request
}

/// The answer of `app` to `request`, a page or a read by identifier holding the object
/// `id`, with a write of the version of `id` validated between the read of its rows and
/// that of their versions (tasks 021-11c, 11d; [`kartenn_testing::across_a_write`]); the
/// version written.
pub(crate) async fn served_across_a_write(
    pool: &PgPool,
    app: &Router,
    request: Request<Body>,
    id: Uuid,
) -> (Response, Uuid) {
    let served = app.clone().oneshot(request);
    let (response, written) = kartenn_testing::across_a_write(pool, &id.to_string(), served).await;
    (response.unwrap(), written.parse().unwrap())
}
