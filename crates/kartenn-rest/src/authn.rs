//! Authentication (REQ-006-26, 27; layer 6): the session cookie, `Authorization: Bearer`
//! or `Basic` → a [`Principal`] in the request extensions, and the [`RequestContext`] of
//! its actor. Two proofs at once → 400, no implicit priority; no proof → 401 without
//! error code; a refused token → 401 `invalid_token` (RFC 6750 §3.1). Without TLS, a
//! `Basic` header or a session cookie is refused (REQ-006-27). The routes outside `/v1`
//! and the login routes ([`PUBLIC`]) answer without authentication.

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use kartenn_auth::keys::KeyRing;
use kartenn_auth::oidc::{SESSION_COOKIE, Session};
use kartenn_auth::secrets::{FailureLimiter, parse_basic};
use kartenn_auth::tokens::{KeySource, TokenError, TokenVerifier};
use kartenn_core::audit::{AuditEvent, Source};
use kartenn_core::period::Instant;
use kartenn_service::aggregate::FailureAggregator;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::{
    self, AuthnError, Failure, Identities, IdentityKind, Method, Principal, StateCache,
};

use crate::problem::{Problem, ProblemKind};
use crate::transport::Client;

/// The routes under `/v1` that answer without authentication: the login routes (design
/// « Ressources et routes »; `logout` needs the session).
pub const PUBLIC: [&str; 3] = ["/v1/auth/login", "/v1/auth/callback", "/v1/auth/breakglass"];

/// The challenge of every 401 (RFC 6750 §3). No `Basic` challenge: a browser would open
/// its own login dialog over the application.
const CHALLENGE: &str = "Bearer realm=\"kartenn\"";
const INVALID_TOKEN: &str = "Bearer realm=\"kartenn\", error=\"invalid_token\"";

/// The proof a request presents.
#[derive(Clone, PartialEq, Eq)]
pub enum Proof {
    /// The chunks of the session cookie, in order.
    Session(Vec<String>),
    /// An access token.
    Bearer(String),
    /// The value of a `Basic` `Authorization` header.
    Basic(String),
}

impl std::fmt::Debug for Proof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Session(_) => "Session(***)",
            Self::Bearer(_) => "Bearer(***)",
            Self::Basic(_) => "Basic(***)",
        })
    }
}

/// Why a proof is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No valid proof: 401 without error code.
    Unauthenticated,
    /// A refused access token: 401 `invalid_token`.
    InvalidToken,
    /// Too many failures from the source (REQ-005-13): 429.
    TooManyFailures,
    /// The identities, the keys of an issuer or the audit cannot be reached: 503.
    Unavailable,
    /// Two proofs at once (a session and `Authorization`, or two `Authorization`): 400.
    TwoProofs,
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        let challenge = |value| [(header::WWW_AUTHENTICATE, HeaderValue::from_static(value))];
        match self {
            Self::Unauthenticated => (
                challenge(CHALLENGE),
                Problem::new(ProblemKind::Unauthenticated),
            )
                .into_response(),
            Self::InvalidToken => (
                challenge(INVALID_TOKEN),
                Problem::new(ProblemKind::Unauthenticated).detail("invalid token"),
            )
                .into_response(),
            Self::TooManyFailures => Problem::new(ProblemKind::RateLimited)
                .detail("too many failed authentications")
                .into_response(),
            Self::Unavailable => Problem::new(ProblemKind::Unavailable).into_response(),
            Self::TwoProofs => Problem::new(ProblemKind::BadRequest)
                .detail("two proofs of authentication")
                .into_response(),
        }
    }
}

/// Turns a proof into a principal (the port of the layer).
pub trait Authenticator: Send + Sync {
    /// The principal of `proof`, presented from `source`.
    fn authenticate<'a>(
        &'a self,
        proof: Proof,
        source: Source,
    ) -> Pin<Box<dyn Future<Output = Result<Principal, Refusal>> + Send + 'a>>;
}

/// No authentication configured (no `[auth]`): every proof is refused.
#[derive(Debug, Clone, Copy)]
pub struct Nobody;

impl Authenticator for Nobody {
    fn authenticate<'a>(
        &'a self,
        _: Proof,
        _: Source,
    ) -> Pin<Box<dyn Future<Output = Result<Principal, Refusal>> + Send + 'a>> {
        Box::pin(std::future::ready(Err(Refusal::Unauthenticated)))
    }
}

/// The authentication of spec 005, put to work.
#[derive(Debug)]
pub struct Authn<I, J, S> {
    /// The identities (the store).
    pub identities: I,
    /// The audit journal.
    pub journal: J,
    /// The key ring: session cookies, secrets of the service accounts.
    pub ring: KeyRing,
    /// The access tokens of the issuers.
    pub tokens: TokenVerifier<S>,
    /// The state of the identities (K10).
    pub states: StateCache,
    /// The failures per source (REQ-005-13).
    pub limiter: FailureLimiter,
    /// The failures aggregated in the audit (REQ-012-19).
    pub aggregator: FailureAggregator,
    /// Inactivity that closes a session.
    pub idle: Duration,
    /// Lifetime of a session.
    pub absolute: Duration,
}

/// Unix seconds now.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn seconds(duration: Duration) -> i64 {
    i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
}

/// A refusal of the service: refused → `refused`, otherwise unavailable.
fn refusal(error: &AuthnError, refused: Refusal) -> Refusal {
    match error {
        AuthnError::Refused => refused,
        AuthnError::Store(e) | AuthnError::Audit(e) => {
            tracing::error!(name: "authn.unavailable", error = %e, "authentication cannot complete");
            Refusal::Unavailable
        }
    }
}

impl<I: Identities, J: AuditJournal, S: KeySource> Authn<I, J, S> {
    /// The events of the failures aggregated in windows over at `at`, or of every window
    /// if `all`, to record.
    pub fn tick(&self, at: Instant, all: bool) -> Vec<AuditEvent> {
        self.aggregator.flush(at, all)
    }

    async fn failed(&self, method: Method, reason: &str, source: Source) -> Refusal {
        let failure = Failure {
            claimed: "",
            method,
            issuer: None,
            reason,
        };
        let at = kartenn_service::audit::now();
        match authn::failed(
            &self.identities,
            &self.journal,
            &self.aggregator,
            failure,
            source,
            at,
        )
        .await
        {
            Ok(()) => Refusal::Unauthenticated,
            Err(e) => refusal(&e, Refusal::Unauthenticated),
        }
    }

    async fn session(&self, chunks: &[String], source: Source) -> Result<Principal, Refusal> {
        let chunks: Vec<&str> = chunks.iter().map(String::as_str).collect();
        let session = Session::open(
            &self.ring,
            &chunks,
            unix_now(),
            seconds(self.idle),
            seconds(self.absolute),
        )
        .map_err(|_| Refusal::Unauthenticated)?;
        let principal = Principal {
            identity: session.identity,
            kind: IdentityKind::Person,
            issuer: Some(session.issuer),
            groups: session.groups,
            method: Method::OidcSession,
        };
        let started = Instant(session.started.saturating_mul(1_000_000));
        self.states
            .check(
                &self.identities,
                &self.journal,
                &principal,
                Some(started),
                source,
            )
            .await
            .map_err(|e| refusal(&e, Refusal::Unauthenticated))?;
        Ok(principal)
    }

    async fn bearer(&self, token: &str, source: Source) -> Result<Principal, Refusal> {
        let verified = match self.tokens.verify(token).await {
            Ok(verified) => verified,
            Err(TokenError::Keys(e)) => {
                tracing::error!(name: "authn.issuer_unavailable", error = %e, "the keys of an issuer cannot be fetched");
                return Err(Refusal::Unavailable);
            }
            Err(e) => {
                return match self.failed(Method::Bearer, e.reason(), source).await {
                    Refusal::Unauthenticated => Err(Refusal::InvalidToken),
                    other => Err(other),
                };
            }
        };
        let principal = authn::seen(
            &self.identities,
            &self.journal,
            &self.aggregator,
            (
                &verified.issuer,
                &verified.subject,
                verified.name.as_deref(),
            ),
            verified.groups,
            Method::Bearer,
            source.clone(),
            kartenn_service::audit::now(),
        )
        .await
        .map_err(|e| refusal(&e, Refusal::InvalidToken))?;
        self.states
            .check(&self.identities, &self.journal, &principal, None, source)
            .await
            .map_err(|e| refusal(&e, Refusal::InvalidToken))?;
        Ok(principal)
    }

    async fn basic(&self, header: &str, source: Source) -> Result<Principal, Refusal> {
        let Some((name, secret)) = parse_basic(header) else {
            return Err(self.failed(Method::Basic, "malformed", source).await);
        };
        authn::authenticate_service(
            &self.identities,
            &self.ring,
            &self.journal,
            &self.aggregator,
            &name,
            &secret,
            source.ip,
            kartenn_service::audit::now(),
        )
        .await
        .map_err(|e| refusal(&e, Refusal::Unauthenticated))
    }

    async fn run(&self, proof: Proof, source: Source) -> Result<Principal, Refusal> {
        let ip = source.ip.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        if !self.limiter.allows(ip, std::time::Instant::now()) {
            return Err(Refusal::TooManyFailures);
        }
        let result = match &proof {
            Proof::Session(chunks) => self.session(chunks, source).await,
            Proof::Bearer(token) => self.bearer(token, source).await,
            Proof::Basic(header) => self.basic(header, source).await,
        };
        if matches!(
            result,
            Err(Refusal::Unauthenticated | Refusal::InvalidToken)
        ) {
            self.limiter.failed(ip, std::time::Instant::now());
        }
        result
    }
}

impl<I, J, S> Authenticator for Authn<I, J, S>
where
    I: Identities,
    J: AuditJournal,
    S: KeySource,
{
    fn authenticate<'a>(
        &'a self,
        proof: Proof,
        source: Source,
    ) -> Pin<Box<dyn Future<Output = Result<Principal, Refusal>> + Send + 'a>> {
        Box::pin(self.run(proof, source))
    }
}

/// The chunks of the session cookie: `__Host-kartenn` alone, or `__Host-kartenn-1`… in
/// order; `None` without any.
fn session_cookie(headers: &HeaderMap) -> Option<Vec<String>> {
    let mut whole = None;
    let mut chunks: Vec<(u8, String)> = Vec::new();
    let pairs = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='));
    for (name, value) in pairs {
        if name == SESSION_COOKIE {
            whole = Some(value.to_owned());
        } else if let Some(n) = name
            .strip_prefix(SESSION_COOKIE)
            .and_then(|rest| rest.strip_prefix('-'))
            .and_then(|n| n.parse::<u8>().ok())
        {
            chunks.push((n, value.to_owned()));
        }
    }
    if let Some(whole) = whole {
        return Some(vec![whole]);
    }
    if chunks.is_empty() {
        return None;
    }
    chunks.sort_by_key(|(n, _)| *n);
    Some(chunks.into_iter().map(|(_, value)| value).collect())
}

/// The proof of a request: `Err` with the answer when there are two, a malformed one, or
/// none.
fn proof(headers: &HeaderMap, client: Client) -> Result<Proof, Refusal> {
    let mut authorizations = headers.get_all(header::AUTHORIZATION).iter();
    let authorization = authorizations.next();
    if authorizations.next().is_some() {
        return Err(Refusal::TwoProofs);
    }
    let proof = match (authorization, session_cookie(headers)) {
        (Some(_), Some(_)) => return Err(Refusal::TwoProofs),
        (None, None) => return Err(Refusal::Unauthenticated),
        (None, Some(chunks)) => Proof::Session(chunks),
        (Some(value), None) => {
            let value = value.to_str().map_err(|_| Refusal::Unauthenticated)?;
            match value.trim().split_once(' ') {
                Some((scheme, token)) if scheme.eq_ignore_ascii_case("bearer") => {
                    Proof::Bearer(token.trim().to_owned())
                }
                Some((scheme, _)) if scheme.eq_ignore_ascii_case("basic") => {
                    Proof::Basic(value.to_owned())
                }
                _ => return Err(Refusal::Unauthenticated),
            }
        }
    };
    // REQ-006-27: a password or a session cookie never travels in clear.
    if !client.tls && matches!(proof, Proof::Basic(_) | Proof::Session(_)) {
        return Err(Refusal::Unauthenticated);
    }
    Ok(proof)
}

/// Whether `path` answers without authentication.
fn public(path: &str) -> bool {
    !(path == "/v1" || path.starts_with("/v1/")) || PUBLIC.contains(&path)
}

/// Layer 6: the [`Principal`] and the [`RequestContext`] of its actor in the request
/// extensions, or the refusal.
pub async fn layer(
    State(authenticator): State<Arc<dyn Authenticator>>,
    mut request: Request,
    next: Next,
) -> Response {
    if public(request.uri().path()) {
        return next.run(request).await;
    }
    let client = request
        .extensions()
        .get::<Client>()
        .copied()
        .unwrap_or(Client {
            addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            tls: false,
        });
    let proof = match proof(request.headers(), client) {
        Ok(proof) => proof,
        Err(refusal) => return refusal.into_response(),
    };
    let source = Source {
        ip: Some(client.addr),
        user_agent: request
            .headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
    };
    let principal = match authenticator.authenticate(proof, source.clone()).await {
        Ok(principal) => principal,
        Err(refusal) => return refusal.into_response(),
    };
    let mut context = principal.context(source);
    if let Some(before) = request.extensions().get::<RequestContext>() {
        context.correlation = before.correlation.clone();
    }
    request.extensions_mut().insert(context);
    request.extensions_mut().insert(principal);
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(
                header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        map
    }

    const TLS: Client = Client {
        addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        tls: true,
    };

    #[test]
    fn the_session_cookie_is_read_whole_or_by_chunks() {
        assert_eq!(
            session_cookie(&headers(&[("cookie", "a=1; __Host-kartenn=xyz")])),
            Some(vec!["xyz".into()])
        );
        assert_eq!(
            session_cookie(&headers(&[
                ("cookie", "__Host-kartenn-2=b"),
                ("cookie", "x=y; __Host-kartenn-1=a")
            ])),
            Some(vec!["a".into(), "b".into()])
        );
        assert_eq!(
            session_cookie(&headers(&[("cookie", "__Host-kartenn-login=l")])),
            None,
            "the login cookie is no session"
        );
    }

    #[test]
    fn a_request_presents_one_proof() {
        let status = |h: &[(&str, &str)], client| {
            proof(&headers(h), client).map_err(|r| r.into_response().status())
        };
        assert_eq!(
            status(&[("authorization", "Bearer t")], TLS),
            Ok(Proof::Bearer("t".into()))
        );
        assert_eq!(
            status(&[("authorization", "basic YTpi")], TLS),
            Ok(Proof::Basic("basic YTpi".into()))
        );
        assert_eq!(status(&[], TLS), Err(axum::http::StatusCode::UNAUTHORIZED));
        assert_eq!(
            status(&[("authorization", "Digest x")], TLS),
            Err(axum::http::StatusCode::UNAUTHORIZED)
        );
        assert_eq!(
            status(
                &[
                    ("authorization", "Bearer t"),
                    ("cookie", "__Host-kartenn=s")
                ],
                TLS
            ),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            status(
                &[("authorization", "Bearer t"), ("authorization", "Bearer u")],
                TLS
            ),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn only_the_login_routes_and_the_probes_are_public() {
        for path in [
            "/health/live",
            "/v1/auth/login",
            "/v1/auth/callback",
            "/other",
        ] {
            assert!(public(path), "{path}");
        }
        for path in ["/v1", "/v1/prefixes", "/v1/auth/logout", "/v1/me", "/v1x"] {
            assert_eq!(public(path), path == "/v1x", "{path}");
        }
    }
}
