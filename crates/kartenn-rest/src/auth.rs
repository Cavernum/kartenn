//! The login routes and the session (REQ-006-24, 25): `GET /v1/auth/login?issuer=…` to the
//! issuer with the login cookie, `GET /v1/auth/callback` back to `/` with the session
//! cookie, `POST /v1/auth/breakglass` (a session of at most 1 h), `POST /v1/auth/logout`
//! to the end-session endpoint of the issuer, `GET /v1/me`. The login cookie is removed
//! at the end of every callback, the session cookie at the logout; no cookie is set
//! without TLS (REQ-006-27).

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use kartenn_auth::oidc::{Callback, LOGIN_COOKIE, LOGIN_WITHIN, SESSION_COOKIE, Session};
use kartenn_core::audit::rfc3339;
use kartenn_core::period::Instant;
use kartenn_service::authn::{IdentityKind, Principal};
use serde::Deserialize;

use crate::authn::{self, Authenticator, Refusal, SessionEnds};
use crate::cookies;
use crate::problem::{Problem, ProblemKind};

/// The routes, under `/v1`.
pub fn routes(authenticator: Arc<dyn Authenticator>) -> Router {
    Router::new()
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/breakglass", post(breakglass))
        .route("/auth/logout", post(logout))
        .route("/me", get(me))
        .with_state(authenticator)
}

/// The query parameter `name` of `uri`, decoded.
fn param(uri: &Uri, name: &str) -> Option<String> {
    form_urlencoded::parse(uri.query()?.as_bytes())
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

async fn login(State(authenticator): State<Arc<dyn Authenticator>>, request: Request) -> Response {
    if !authn::client(&request).tls {
        return Refusal::NotTls.into_response();
    }
    let Some(issuer) = param(request.uri(), "issuer") else {
        return Refusal::UnknownIssuer.into_response();
    };
    match authenticator.begin(issuer).await {
        Ok(start) => {
            let mut response = Redirect::to(&start.url).into_response();
            cookies::append(
                response.headers_mut(),
                cookies::set(LOGIN_COOKIE, &start.cookie, Some(LOGIN_WITHIN)),
            );
            response
        }
        Err(refusal) => refusal.into_response(),
    }
}

async fn callback(
    State(authenticator): State<Arc<dyn Authenticator>>,
    request: Request,
) -> Response {
    let client = authn::client(&request);
    if !client.tls {
        return Refusal::NotTls.into_response();
    }
    let uri = request.uri();
    let callback = Callback {
        code: param(uri, "code"),
        state: param(uri, "state"),
        iss: param(uri, "iss"),
        error: param(uri, "error"),
    };
    let login = cookies::read(request.headers(), LOGIN_COOKIE).unwrap_or_default();
    let source = authn::source(client, request.headers());
    let mut response = match authenticator.complete(callback, login, source).await {
        Ok(session) => {
            let mut response = Redirect::to("/").into_response();
            cookies::append(
                response.headers_mut(),
                cookies::set(SESSION_COOKIE, &session, None),
            );
            response
        }
        Err(refusal) => refusal.into_response(),
    };
    // A login is played once: its cookie goes, whatever the outcome.
    cookies::append(response.headers_mut(), cookies::remove(LOGIN_COOKIE));
    response
}

/// The proofs of a break-glass login.
#[derive(Deserialize)]
struct Breakglass {
    name: String,
    passphrase: String,
    code: String,
}

async fn breakglass(
    State(authenticator): State<Arc<dyn Authenticator>>,
    request: Request,
) -> Response {
    let client = authn::client(&request);
    if !client.tls {
        return Refusal::NotTls.into_response();
    }
    let source = authn::source(client, request.headers());
    // The size is bounded by layer 5.
    let Ok(body) = axum::body::to_bytes(request.into_body(), usize::MAX).await else {
        return Problem::new(ProblemKind::BadRequest).into_response();
    };
    let Ok(Breakglass {
        name,
        passphrase,
        code,
    }) = serde_json::from_slice(&body)
    else {
        return Problem::new(ProblemKind::BadRequest)
            .detail("expected name, passphrase and code")
            .into_response();
    };
    match authenticator
        .breakglass(name, passphrase, code, source)
        .await
    {
        Ok(session) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            cookies::append(
                response.headers_mut(),
                cookies::set(SESSION_COOKIE, &session, None),
            );
            response
        }
        Err(refusal) => refusal.into_response(),
    }
}

async fn logout(State(authenticator): State<Arc<dyn Authenticator>>, request: Request) -> Response {
    let source = authn::source(authn::client(&request), request.headers());
    let (Some(principal), Some(session)) = (
        request.extensions().get::<Principal>().cloned(),
        request.extensions().get::<Session>().cloned(),
    ) else {
        return Refusal::NoSession.into_response();
    };
    match authenticator.logout(session, principal, source).await {
        Ok(end) => {
            // REQ-005-8: on to the issuer, which closes its own session.
            let mut response = Redirect::to(end.as_deref().unwrap_or("/")).into_response();
            response.extensions_mut().insert(cookies::Ended);
            cookies::append(response.headers_mut(), cookies::remove(SESSION_COOKIE));
            response
        }
        Err(refusal) => refusal.into_response(),
    }
}

const fn kind(kind: IdentityKind) -> &'static str {
    match kind {
        IdentityKind::Person => "person",
        IdentityKind::ServiceAccount => "service_account",
        IdentityKind::Breakglass => "breakglass",
    }
}

async fn me(State(authenticator): State<Arc<dyn Authenticator>>, request: Request) -> Response {
    let Some(principal) = request.extensions().get::<Principal>() else {
        return Refusal::Unauthenticated.into_response();
    };
    let rights = match authenticator.rights(principal).await {
        Ok(rights) => rights,
        Err(refusal) => return refusal.into_response(),
    };
    let ends = request
        .extensions()
        .get::<SessionEnds>()
        .map(|SessionEnds(at)| rfc3339(Instant(at.saturating_mul(1_000_000))));
    let session = request
        .extensions()
        .get::<Session>()
        .map(|s| serde_json::json!({ "breakglass": s.breakglass, "expires_at": ends }));
    let bindings: Vec<serde_json::Value> = rights
        .bindings
        .iter()
        .map(|b| {
            let scope = match b.scope {
                Some(id) => serde_json::json!({ "kind": b.scope_kind, "id": id }),
                None => serde_json::json!({ "kind": b.scope_kind }),
            };
            serde_json::json!({ "role": b.role, "scope": scope })
        })
        .collect();
    (
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(serde_json::json!({
            "identity": principal.identity,
            "kind": kind(principal.kind),
            "method": principal.method.as_str(),
            "issuer": principal.issuer,
            "groups": principal.groups,
            "name": rights.name,
            "roles": rights.roles,
            "bindings": bindings,
            "global_permissions": rights.global_permissions,
            "session": session,
        })),
    )
        .into_response()
}
