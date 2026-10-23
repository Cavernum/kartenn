//! Authentication (REQ-006-24 to 27; layer 6): the session cookie, `Authorization: Bearer`
//! or `Basic` → a [`Principal`] in the request extensions, and the [`RequestContext`] of
//! its actor. Two proofs at once → 400, no implicit priority; no proof → 401 without
//! error code; a refused token → 401 `invalid_token` (RFC 6750 §3.1). Without TLS, a
//! `Basic` header or a session cookie is refused (REQ-006-27). The routes outside `/v1`
//! and the login routes ([`PUBLIC`]) answer without authentication. A session is sent
//! again at most once a minute, or when its groups were refreshed; its end is audited
//! (`authn.session.closed`) and its cookie removed.

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use kartenn_auth::breakglass::Verifier;
use kartenn_auth::keys::KeyRing;
use kartenn_auth::oidc::{
    Callback, LoginError, LoginStart, Oidc, SESSION_COOKIE, Session, SessionError,
};
use kartenn_auth::secrets::{FailureLimiter, parse_basic};
use kartenn_auth::tokens::{KeySource, TokenError};
use kartenn_core::audit::{AuditEvent, Source};
use kartenn_core::period::Instant;
use kartenn_service::aggregate::FailureAggregator;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::{
    self, AuthnError, Failure, Identities, IdentityKind, Method, Principal, Rights, SessionEnd,
    StateCache,
};

use kartenn_service::authn::Requester;
use uuid::Uuid;

use crate::cookies;
use crate::problem::{Problem, ProblemKind};
use crate::transport::Client;

/// The routes under `/v1` that answer without authentication: the login routes (design
/// « Ressources et routes »; `logout` needs the session).
pub const PUBLIC: [&str; 5] = [
    "/v1/auth/login",
    "/v1/auth/callback",
    "/v1/auth/breakglass",
    "/v1/auth/issuers",
    "/v1/auth/native-clients",
];

/// A public client of the native applications at an issuer, as `GET
/// /v1/auth/native-clients` publishes it (spec 023, T10/A5): nothing secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeClient {
    /// The local identifier of the issuer.
    pub id: String,
    /// Its discovery document (OpenID Connect Discovery 1.0 §4).
    pub discovery_url: String,
    /// The public client at the issuer.
    pub client_id: String,
    /// The audience its access tokens must carry for kartenn.
    pub audience: String,
    /// The scopes to ask for.
    pub scopes: Vec<String>,
    /// Whether the issuer offers it the device flow (RFC 8628).
    pub device: bool,
}

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
    /// A login at an issuer kartenn does not know: 400.
    UnknownIssuer,
    /// A login in clear: no cookie is set without TLS (REQ-006-27): 400.
    NotTls,
    /// A logout without a session: 400.
    NoSession,
    /// `Kartenn-On-Behalf-Of` malformed, repeated or on a read: 400 (REQ-016-12).
    BadOnBehalf(&'static str),
    /// `Kartenn-On-Behalf-Of` from other than a service account holding
    /// `core.identity.act_on_behalf`: 403 (REQ-016-12).
    OnBehalfForbidden,
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        let challenge = |value| [(header::WWW_AUTHENTICATE, HeaderValue::from_static(value))];
        let bad = |detail| Problem::new(ProblemKind::BadRequest).detail(detail);
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
            Self::TwoProofs => bad("two proofs of authentication").into_response(),
            Self::UnknownIssuer => bad("unknown issuer").into_response(),
            Self::NotTls => bad("TLS required").into_response(),
            Self::NoSession => bad("no session").into_response(),
            Self::BadOnBehalf(detail) => bad(detail).into_response(),
            Self::OnBehalfForbidden => Problem::new(ProblemKind::Forbidden)
                .detail(kartenn_core::authz::IDENTITY_ACT_ON_BEHALF.name)
                .into_response(),
        }
    }
}

/// A proof accepted.
#[derive(Debug, Clone)]
pub struct Authenticated {
    /// Who.
    pub principal: Principal,
    /// The session, for a session cookie.
    pub session: Option<Session>,
    /// The chunks of the session cookie to send again, if due.
    pub cookie: Option<Vec<String>>,
    /// When the session ends unless seen again (Unix seconds), for a session cookie.
    pub ends_at: Option<i64>,
}

impl From<Principal> for Authenticated {
    fn from(principal: Principal) -> Self {
        Self {
            principal,
            session: None,
            cookie: None,
            ends_at: None,
        }
    }
}

/// When the session of the request ends unless seen again (Unix seconds); a request
/// extension beside its [`Session`] (`GET /v1/me`).
#[derive(Debug, Clone, Copy)]
pub struct SessionEnds(pub i64);

/// What the [`Authenticator`] answers, later.
pub type Answer<'a, T> = Pin<Box<dyn Future<Output = Result<T, Refusal>> + Send + 'a>>;

fn refused<'a, T: Send + 'a>(refusal: Refusal) -> Answer<'a, T> {
    Box::pin(std::future::ready(Err(refusal)))
}

/// Turns a proof into a principal, and opens and closes sessions (the port of the layer
/// and of the login routes). Only [`Self::authenticate`] is required: by default, no
/// login is possible.
pub trait Authenticator: Send + Sync {
    /// The principal of `proof`, presented from `source`.
    fn authenticate(&self, proof: Proof, source: Source) -> Answer<'_, Authenticated>;

    /// Starts a login at the issuer `issuer`.
    fn begin(&self, issuer: String) -> Answer<'_, LoginStart> {
        let _ = issuer;
        refused(Refusal::UnknownIssuer)
    }

    /// Completes a login from its `callback` and the chunks of its login cookie: the
    /// chunks of the session cookie.
    fn complete(
        &self,
        callback: Callback,
        login: Vec<String>,
        source: Source,
    ) -> Answer<'_, Vec<String>> {
        let _ = (callback, login, source);
        refused(Refusal::Unauthenticated)
    }

    /// Opens a break-glass session for `name` by its passphrase and a TOTP code: the
    /// chunks of the session cookie.
    fn breakglass(
        &self,
        name: String,
        passphrase: String,
        code: String,
        source: Source,
    ) -> Answer<'_, Vec<String>> {
        let _ = (name, passphrase, code, source);
        refused(Refusal::Unauthenticated)
    }

    /// Closes `session` of `principal`: where to send the browser at the issuer, if
    /// anywhere (REQ-005-8).
    fn logout(
        &self,
        session: Session,
        principal: Principal,
        source: Source,
    ) -> Answer<'_, Option<String>> {
        let _ = (session, principal, source);
        refused(Refusal::Unavailable)
    }

    /// The local identifiers of the issuers a browser logs in at, in the order of the
    /// configuration (REQ-007-41).
    fn issuers(&self) -> Vec<String> {
        Vec::new()
    }

    /// The public clients of the native applications, one per issuer configured with one,
    /// in the order of the configuration (spec 023, T10/A5).
    fn native_clients(&self) -> Vec<NativeClient> {
        Vec::new()
    }

    /// A summary of the rights `principal` holds now.
    fn rights<'a>(&'a self, principal: &'a Principal) -> Answer<'a, Rights> {
        let _ = principal;
        Box::pin(std::future::ready(Ok(Rights::default())))
    }

    /// The identity of the requester `principal` writes for (REQ-016-12); refused by
    /// default.
    fn on_behalf_of(
        &self,
        principal: Principal,
        requester: Requester,
        source: Source,
    ) -> Answer<'_, Uuid> {
        let _ = (principal, requester, source);
        refused(Refusal::OnBehalfForbidden)
    }
}

/// No authentication configured (no `[auth]`): every proof is refused.
#[derive(Debug, Clone, Copy)]
pub struct Nobody;

impl Authenticator for Nobody {
    fn authenticate(&self, _: Proof, _: Source) -> Answer<'_, Authenticated> {
        refused(Refusal::Unauthenticated)
    }
}

/// The authentication of spec 005, put to work.
#[derive(Debug)]
pub struct Authn<I, J, S> {
    /// The identities (the store).
    pub identities: I,
    /// The audit journal.
    pub journal: J,
    /// The key ring: session cookies, secrets of the service accounts, TOTP secrets.
    pub ring: KeyRing,
    /// The OIDC client of the issuers, and the verifier of their access tokens.
    pub oidc: Oidc<S>,
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

/// The principal of a session.
fn principal_of(session: &Session) -> Principal {
    if session.breakglass {
        Principal {
            identity: session.identity,
            kind: IdentityKind::Breakglass,
            issuer: None,
            groups: Vec::new(),
            method: Method::Breakglass,
        }
    } else {
        Principal {
            identity: session.identity,
            kind: IdentityKind::Person,
            issuer: Some(session.issuer.clone()),
            groups: session.groups.clone(),
            method: Method::OidcSession,
        }
    }
}

impl<I: Identities, J: AuditJournal, S: KeySource> Authn<I, J, S> {
    /// The events of the failures aggregated in windows over at `at`, or of every window
    /// if `all`, to record.
    pub fn tick(&self, at: Instant, all: bool) -> Vec<AuditEvent> {
        self.aggregator.flush(at, all)
    }

    /// Forgets the identity states past their time to live and the sources whose window of
    /// failures is over (the local timers of REQ-006-38).
    pub fn purge(&self) {
        self.states.purge();
        self.limiter.purge(std::time::Instant::now());
    }

    /// Reads again the discoveries and the keys of the issuers (every 5 min, REQ-006-38);
    /// an issuer that does not answer keeps what was read, with a warning.
    pub async fn reload(&self) {
        for (issuer, error) in self.oidc.reload().await {
            tracing::warn!(name: "authn.issuer.reload_failed", %issuer, %error, "the issuer could not be read again, its last keys stay");
        }
    }

    /// Audits a failure: [`Refusal::Unauthenticated`], or why it cannot be audited.
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

    /// Audits the end of the session of `principal`: [`Refusal::Unauthenticated`], or why
    /// it cannot be audited.
    async fn closed(&self, principal: &Principal, end: SessionEnd, source: Source) -> Refusal {
        match authn::session_closed(&self.journal, principal, end, source).await {
            Ok(()) => Refusal::Unauthenticated,
            Err(e) => refusal(&e, Refusal::Unauthenticated),
        }
    }

    fn sealed(&self, session: &Session) -> Result<Vec<String>, Refusal> {
        session.seal(&self.ring).map_err(|e| {
            tracing::error!(name: "authn.unavailable", error = %e, "the session cannot be sealed");
            Refusal::Unavailable
        })
    }

    async fn session(&self, chunks: &[String], source: Source) -> Result<Authenticated, Refusal> {
        let at = unix_now();
        let chunks: Vec<&str> = chunks.iter().map(String::as_str).collect();
        let mut session =
            Session::unseal(&self.ring, &chunks).map_err(|_| Refusal::Unauthenticated)?;
        let mut principal = principal_of(&session);
        let last = session.seen;
        if let Err(e) = session.check(at, seconds(self.idle), seconds(self.absolute)) {
            let end = if e == SessionError::Idle {
                SessionEnd::Idle
            } else {
                SessionEnd::Absolute
            };
            return Err(self.closed(&principal, end, source).await);
        }
        let started = Instant(session.started.saturating_mul(1_000_000));
        let state = self
            .states
            .check(
                &self.identities,
                &self.journal,
                &principal,
                Some(started),
                source.clone(),
            )
            .await;
        match state {
            Ok(()) => {}
            Err(AuthnError::Refused) => {
                return Err(self.closed(&principal, SessionEnd::Revoked, source).await);
            }
            Err(e) => return Err(refusal(&e, Refusal::Unauthenticated)),
        }
        let mut reissue = at - last >= cookies::REISSUE_AFTER;
        // REQ-005-7: the groups refreshed at the issuer when its access token expires.
        if !session.breakglass && session.refresh_due(at) {
            match self.oidc.refresh(&mut session).await {
                Ok(()) => {
                    principal.groups.clone_from(&session.groups);
                    reissue = true;
                }
                Err(LoginError::Unavailable(e)) => {
                    tracing::error!(name: "authn.issuer_unavailable", error = %e, "the groups cannot be refreshed");
                    return Err(Refusal::Unavailable);
                }
                Err(e) => {
                    tracing::warn!(name: "authn.refresh_failed", reason = e.reason(), "the issuer refused to refresh a session");
                    return Err(self
                        .closed(&principal, SessionEnd::RefreshFailed, source)
                        .await);
                }
            }
        }
        let cookie = if reissue {
            Some(self.sealed(&session)?)
        } else {
            None
        };
        let ends_at = Some(session.ends_at(seconds(self.idle), seconds(self.absolute)));
        Ok(Authenticated {
            principal,
            session: Some(session),
            cookie,
            ends_at,
        })
    }

    async fn bearer(&self, token: &str, source: Source) -> Result<Authenticated, Refusal> {
        let verified = match self.oidc.tokens().verify(token).await {
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
        Ok(principal.into())
    }

    async fn basic(&self, header: &str, source: Source) -> Result<Authenticated, Refusal> {
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
        .map(Authenticated::from)
        .map_err(|e| refusal(&e, Refusal::Unauthenticated))
    }

    /// `attempt`, refused at once after too many failures from `source`, its own failure
    /// counted (REQ-005-13).
    async fn limited<T>(
        &self,
        source: &Source,
        attempt: impl Future<Output = Result<T, Refusal>>,
    ) -> Result<T, Refusal> {
        let ip = source.ip.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        if !self.limiter.allows(ip, std::time::Instant::now()) {
            return Err(Refusal::TooManyFailures);
        }
        let result = attempt.await;
        if matches!(
            result,
            Err(Refusal::Unauthenticated | Refusal::InvalidToken)
        ) {
            self.limiter.failed(ip, std::time::Instant::now());
        }
        result
    }

    async fn run(&self, proof: Proof, source: Source) -> Result<Authenticated, Refusal> {
        let attempt = async {
            match &proof {
                Proof::Session(chunks) => self.session(chunks, source.clone()).await,
                Proof::Bearer(token) => self.bearer(token, source.clone()).await,
                Proof::Basic(header) => self.basic(header, source.clone()).await,
            }
        };
        self.limited(&source, attempt).await
    }

    async fn completed(
        &self,
        callback: Callback,
        login: Vec<String>,
        source: Source,
    ) -> Result<Vec<String>, Refusal> {
        let chunks: Vec<&str> = login.iter().map(String::as_str).collect();
        let login = match self.oidc.complete(&callback, &chunks).await {
            Ok(login) => login,
            Err(e) => {
                let audited = self.failed(Method::OidcSession, e.reason(), source).await;
                return Err(match e {
                    LoginError::Unavailable(_) if audited == Refusal::Unauthenticated => {
                        Refusal::Unavailable
                    }
                    _ => audited,
                });
            }
        };
        let principal = authn::logged_in(
            &self.identities,
            &self.journal,
            &self.aggregator,
            (&login.issuer, &login.subject, login.name.as_deref()),
            login.groups.clone(),
            Method::OidcSession,
            source,
            kartenn_service::audit::now(),
        )
        .await
        .map_err(|e| refusal(&e, Refusal::Unauthenticated))?;
        self.sealed(&Session::new(principal.identity, &login))
    }

    async fn broken(
        &self,
        (name, passphrase, code): (String, String, String),
        source: Source,
    ) -> Result<Vec<String>, Refusal> {
        let principal = authn::authenticate_breakglass(
            &self.identities,
            &Verifier(self.ring.clone()),
            &self.journal,
            &self.aggregator,
            (&name, &passphrase, &code),
            source.ip,
            kartenn_service::audit::now(),
        )
        .await
        .map_err(|e| refusal(&e, Refusal::Unauthenticated))?;
        self.sealed(&Session::breakglass(principal.identity))
    }

    async fn closing(
        &self,
        session: Session,
        principal: Principal,
        source: Source,
    ) -> Result<Option<String>, Refusal> {
        authn::session_closed(&self.journal, &principal, SessionEnd::Logout, source)
            .await
            .map_err(|e| refusal(&e, Refusal::Unavailable))?;
        if session.breakglass {
            return Ok(None);
        }
        // The session is closed here whatever the issuer answers.
        Ok(self
            .oidc
            .logout_url(&session, None)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(name: "authn.issuer_unavailable", error = %e, "no logout at the issuer");
                None
            }))
    }
}

impl<I, J, S> Authenticator for Authn<I, J, S>
where
    I: Identities,
    J: AuditJournal,
    S: KeySource,
{
    fn authenticate(&self, proof: Proof, source: Source) -> Answer<'_, Authenticated> {
        Box::pin(self.run(proof, source))
    }

    fn issuers(&self) -> Vec<String> {
        self.oidc.tokens().issuer_ids().map(str::to_owned).collect()
    }

    fn native_clients(&self) -> Vec<NativeClient> {
        self.oidc
            .tokens()
            .issuers()
            .iter()
            .filter_map(|issuer| {
                let native = issuer.native.as_ref()?;
                Some(NativeClient {
                    id: issuer.id.clone(),
                    discovery_url: issuer.discovery_url(),
                    client_id: native.client_id.clone(),
                    audience: issuer.audience.clone(),
                    scopes: native.scopes.clone(),
                    device: native.device,
                })
            })
            .collect()
    }

    fn begin(&self, issuer: String) -> Answer<'_, LoginStart> {
        Box::pin(async move {
            self.oidc.begin(&issuer).await.map_err(|e| match e {
                LoginError::UnknownIssuer => Refusal::UnknownIssuer,
                e => {
                    tracing::error!(name: "authn.issuer_unavailable", error = %e, "no login at the issuer");
                    Refusal::Unavailable
                }
            })
        })
    }

    fn complete(
        &self,
        callback: Callback,
        login: Vec<String>,
        source: Source,
    ) -> Answer<'_, Vec<String>> {
        Box::pin(async move {
            self.limited(&source, self.completed(callback, login, source.clone()))
                .await
        })
    }

    fn breakglass(
        &self,
        name: String,
        passphrase: String,
        code: String,
        source: Source,
    ) -> Answer<'_, Vec<String>> {
        Box::pin(async move {
            self.limited(
                &source,
                self.broken((name, passphrase, code), source.clone()),
            )
            .await
        })
    }

    fn logout(
        &self,
        session: Session,
        principal: Principal,
        source: Source,
    ) -> Answer<'_, Option<String>> {
        Box::pin(self.closing(session, principal, source))
    }

    fn rights<'a>(&'a self, principal: &'a Principal) -> Answer<'a, Rights> {
        Box::pin(async move {
            self.identities
                .rights(&principal.subject())
                .await
                .map_err(|e| refusal(&AuthnError::Store(e), Refusal::Unavailable))
        })
    }

    fn on_behalf_of(
        &self,
        principal: Principal,
        requester: Requester,
        source: Source,
    ) -> Answer<'_, Uuid> {
        Box::pin(async move {
            authn::act_on_behalf(
                &self.identities,
                &self.journal,
                &principal,
                &requester,
                source,
            )
            .await
            .map_err(|e| refusal(&e, Refusal::OnBehalfForbidden))
        })
    }
}

/// The proof of a request: `Err` with the answer when there are two, a malformed one, or
/// none.
fn proof(headers: &HeaderMap, client: Client) -> Result<Proof, Refusal> {
    let mut authorizations = headers.get_all(header::AUTHORIZATION).iter();
    let authorization = authorizations.next();
    if authorizations.next().is_some() {
        return Err(Refusal::TwoProofs);
    }
    let proof = match (authorization, cookies::read(headers, SESSION_COOKIE)) {
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

/// The requester `Kartenn-On-Behalf-Of` names, if any (REQ-016-12): at most one header,
/// on a write, well formed, from a service account the authenticator lets act for it.
async fn on_behalf(
    authenticator: &dyn Authenticator,
    headers: &HeaderMap,
    method: &axum::http::Method,
    principal: &Principal,
    source: Source,
) -> Result<Option<Uuid>, Refusal> {
    let mut values = headers.get_all(crate::on_behalf::ON_BEHALF_OF).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(Refusal::BadOnBehalf(
            "Kartenn-On-Behalf-Of is given more than once",
        ));
    }
    if matches!(method.as_str(), "GET" | "HEAD" | "OPTIONS" | "QUERY") {
        return Err(Refusal::BadOnBehalf(
            "Kartenn-On-Behalf-Of is only for a write",
        ));
    }
    let value = value
        .to_str()
        .map_err(|_| Refusal::BadOnBehalf("Kartenn-On-Behalf-Of is not ASCII"))?;
    let requester = crate::on_behalf::parse(value).map_err(Refusal::BadOnBehalf)?;
    if principal.kind != IdentityKind::ServiceAccount {
        // Refused whatever the authenticator answers; asked all the same, so that it
        // records the attempt (INV-60: a refused attempt to act for another is audited).
        let _ = authenticator
            .on_behalf_of(principal.clone(), requester, source)
            .await;
        return Err(Refusal::OnBehalfForbidden);
    }
    authenticator
        .on_behalf_of(principal.clone(), requester, source)
        .await
        .map(Some)
}

/// Whether `path` answers without authentication.
fn public(path: &str) -> bool {
    !(path == "/v1" || path.starts_with("/v1/")) || PUBLIC.contains(&path)
}

/// The client of a request, as layer 4 found it.
pub(crate) fn client(request: &Request) -> Client {
    request
        .extensions()
        .get::<Client>()
        .copied()
        .unwrap_or(Client {
            addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            tls: false,
        })
}

/// Where a request comes from, for the audit.
pub(crate) fn source(client: Client, headers: &HeaderMap) -> Source {
    Source {
        ip: Some(client.addr),
        user_agent: headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
    }
}

/// Layer 6: the [`Principal`], its [`Session`] if any, and the [`RequestContext`] of its
/// actor in the request extensions, or the refusal; the session cookie sent again when
/// due, removed when the session ended.
pub async fn layer(
    State(authenticator): State<Arc<dyn Authenticator>>,
    mut request: Request,
    next: Next,
) -> Response {
    if public(request.uri().path()) {
        return next.run(request).await;
    }
    let client = client(&request);
    let proof = match proof(request.headers(), client) {
        Ok(proof) => proof,
        Err(refusal) => return refusal.into_response(),
    };
    let by_session = matches!(proof, Proof::Session(_));
    let source = source(client, request.headers());
    let Authenticated {
        principal,
        session,
        cookie,
        ends_at,
    } = match authenticator.authenticate(proof, source.clone()).await {
        Ok(authenticated) => authenticated,
        Err(refusal) => {
            let mut response = refusal.into_response();
            if by_session && refusal == Refusal::Unauthenticated {
                cookies::append(response.headers_mut(), cookies::remove(SESSION_COOKIE));
            }
            return response;
        }
    };
    let mut context = principal.context(source.clone());
    if let Some(before) = request.extensions().get::<RequestContext>() {
        context.correlation = before.correlation.clone();
    }
    let method = request.method().clone();
    match on_behalf(
        authenticator.as_ref(),
        request.headers(),
        &method,
        &principal,
        source,
    )
    .await
    {
        Ok(requester) => context.on_behalf_of = requester,
        Err(refusal) => return refusal.into_response(),
    }
    request.extensions_mut().insert(context);
    request.extensions_mut().insert(principal);
    if let Some(session) = session {
        request.extensions_mut().insert(session);
    }
    if let Some(ends_at) = ends_at {
        request.extensions_mut().insert(SessionEnds(ends_at));
    }
    let mut response = next.run(request).await;
    if let Some(cookie) = cookie
        && response.extensions().get::<cookies::Ended>().is_none()
    {
        cookies::append(
            response.headers_mut(),
            cookies::set(SESSION_COOKIE, &cookie, None),
        );
    }
    response
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
