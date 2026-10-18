//! What the tests of the layers share: an authenticator that accepts any access token,
//! for the tests whose subject is not the authentication.

#![allow(dead_code)] // each test binary uses its own part of it

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::http::{HeaderValue, Request, header};
use kartenn_core::audit::Source;
use kartenn_rest::authn::{Authenticator, Proof, Refusal};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use uuid::Uuid;

/// Any `Bearer` token is the same person; any other proof is refused.
pub(crate) struct Anyone(pub Uuid);

impl Authenticator for Anyone {
    fn authenticate<'a>(
        &'a self,
        proof: Proof,
        _: Source,
    ) -> Pin<Box<dyn Future<Output = Result<Principal, Refusal>> + Send + 'a>> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(Principal {
                identity: self.0,
                kind: IdentityKind::Person,
                issuer: Some("https://idp.example.invalid".into()),
                groups: Vec::new(),
                method: Method::Bearer,
            }),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }
}

/// [`Anyone`], as the router takes it.
pub(crate) fn anyone() -> Arc<dyn Authenticator> {
    Arc::new(Anyone(Uuid::now_v7()))
}

/// `request` with an access token.
pub(crate) fn bearer<B>(mut request: Request<B>) -> Request<B> {
    request.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer test"),
    );
    request
}
