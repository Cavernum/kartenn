//! The login routes and the session (REQ-006-24, 25): `GET /v1/auth/login?issuer=…` to the
//! issuer with the login cookie, `GET /v1/auth/callback` back to `/` with the session
//! cookie, `POST /v1/auth/breakglass` (a session of at most 1 h), `POST /v1/auth/logout`
//! to the end-session endpoint of the issuer, `GET /v1/me`. The login cookie is removed
//! at the end of every callback, the session cookie at the logout; no cookie is set
//! without TLS (REQ-006-27).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use kartenn_auth::oidc::{Callback, LOGIN_COOKIE, LOGIN_WITHIN, SESSION_COOKIE, Session};
use kartenn_core::audit::rfc3339;
use kartenn_core::period::Instant;
use kartenn_service::authn::{IdentityKind, Principal};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::authn::{self, Authenticator, Refusal, SessionEnds};
use crate::cookies;
use crate::problem::{Problem, ProblemKind};

/// The routes, under `/v1`.
pub fn routes() -> OpenApiRouter<Arc<dyn Authenticator>> {
    OpenApiRouter::new()
        .routes(routes!(issuers))
        .routes(routes!(login))
        .routes(routes!(callback))
        .routes(routes!(breakglass))
        .routes(routes!(logout))
        .routes(routes!(me))
}

/// The query parameter `name` of `uri`, decoded.
fn param(uri: &Uri, name: &str) -> Option<String> {
    form_urlencoded::parse(uri.query()?.as_bytes())
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

/// An issuer a browser logs in at, as the login page names it.
#[derive(Debug, Serialize, ToSchema)]
struct IssuerDto {
    /// Its local identifier, the `issuer` of `/v1/auth/login`.
    id: String,
}

/// `GET /v1/auth/issuers`, public: the issuers of the login page, in the order of the
/// configuration, by their local identifier alone (REQ-007-41): neither URL nor client.
#[utoipa::path(
    get,
    path = "/auth/issuers",
    operation_id = "login_issuers",
    tag = "authentication",
    responses((status = 200, description = "The issuers of the login.", body = Vec<IssuerDto>))
)]
async fn issuers(State(authenticator): State<Arc<dyn Authenticator>>) -> Json<Vec<IssuerDto>> {
    Json(
        authenticator
            .issuers()
            .into_iter()
            .map(|id| IssuerDto { id })
            .collect(),
    )
}

/// Starts a login at the issuer `issuer` (OIDC, REQ-006-24): a redirection with the
/// login cookie.
#[utoipa::path(
    get,
    path = "/auth/login",
    operation_id = "login",
    tag = "authentication",
    params(("issuer" = String, Query, description = "The issuer of the login.")),
    responses((status = 303, description = "To the authorisation endpoint of the issuer."))
)]
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

/// The return of a login from its issuer: a redirection to `/` with the session cookie.
#[utoipa::path(
    get,
    path = "/auth/callback",
    operation_id = "login_callback",
    tag = "authentication",
    params(
        ("code" = Option<String>, Query, description = "The authorisation code."),
        ("state" = Option<String>, Query, description = "The state of the login."),
        ("iss" = Option<String>, Query, description = "The issuer (RFC 9207)."),
        ("error" = Option<String>, Query, description = "The error of the issuer."),
    ),
    responses((status = 303, description = "To `/`, with the session cookie."))
)]
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
#[derive(Deserialize, ToSchema)]
struct Breakglass {
    name: String,
    passphrase: String,
    code: String,
}

/// A break-glass login (REQ-006-25): a session of at most 1 h.
#[utoipa::path(
    post,
    path = "/auth/breakglass",
    operation_id = "breakglass",
    tag = "authentication",
    request_body = Breakglass,
    responses((status = 204, description = "The session cookie is set."))
)]
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

/// Ends the session, then on to the end-session endpoint of the issuer.
#[utoipa::path(
    post,
    path = "/auth/logout",
    operation_id = "logout",
    tag = "authentication",
    responses((status = 303, description = "To the end-session endpoint of the issuer, or `/`."))
)]
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

/// A binding that serves the caller.
#[derive(Debug, Serialize, ToSchema)]
struct HeldBinding {
    /// Role key.
    role: String,
    /// Its scope.
    scope: Scope,
}

/// The scope of a binding: `global`, or an object.
#[derive(Debug, Serialize, ToSchema)]
struct Scope {
    /// `global`, `entity`, `site`, `domain` or `prefix`.
    kind: String,
    /// The object; absent when global.
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Uuid>,
}

/// The session of the caller.
#[derive(Debug, Serialize, ToSchema)]
struct SessionState {
    /// A break-glass session.
    breakglass: bool,
    /// Its end, RFC 3339.
    expires_at: Option<String>,
}

/// The caller and their effective rights, summed up.
#[derive(Debug, Serialize, ToSchema)]
struct Me {
    /// Identity.
    identity: Uuid,
    /// `person`, `service_account` or `breakglass`.
    kind: &'static str,
    /// How the request was authenticated.
    method: &'static str,
    /// The issuer of the identity.
    issuer: Option<String>,
    /// The groups of the identity.
    groups: Vec<String>,
    /// Display name.
    name: Option<String>,
    /// The keys of the roles held, on any scope.
    roles: Vec<String>,
    /// The bindings that serve the caller.
    bindings: Vec<HeldBinding>,
    /// The entries of the roles of the global bindings.
    global_permissions: Vec<String>,
    /// The session; `null` without one.
    session: Option<SessionState>,
}

/// The caller, their session and their effective rights, summed up.
#[utoipa::path(
    get,
    path = "/me",
    operation_id = "me",
    tag = "authentication",
    responses((status = 200, description = "The caller.", body = Me))
)]
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
    let session = request.extensions().get::<Session>().map(|s| SessionState {
        breakglass: s.breakglass,
        expires_at: ends,
    });
    let bindings = rights
        .bindings
        .iter()
        .map(|b| HeldBinding {
            role: b.role.clone(),
            scope: Scope {
                kind: b.scope_kind.clone(),
                id: b.scope,
            },
        })
        .collect();
    (
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(Me {
            identity: principal.identity,
            kind: kind(principal.kind),
            method: principal.method.as_str(),
            issuer: principal.issuer.clone(),
            groups: principal.groups.clone(),
            name: rights.name,
            roles: rights.roles,
            bindings,
            global_permissions: rights.global_permissions,
            session,
        }),
    )
        .into_response()
}
