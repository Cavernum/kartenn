//! Authentication (spec 005, REQ-005-1 to 15): from a verified proof to a [`Principal`],
//! through ports, so that the cryptography and the identity providers stay in the
//! adapters (`kartenn-auth`) and the identities in the store.
//!
//! A refusal never tells why to the caller (one error, [`AuthnError::Refused`]); the audit
//! records the reason, and the claimed identifier only as a digest (REQ-012-9).

use std::future::Future;
use std::net::IpAddr;

use kartenn_core::audit::{
    AUTHN_LOGIN_FAILED, AUTHN_LOGIN_SUCCEEDED, Actor, ActorKind, AuditEvent, Outcome, Source,
};
use kartenn_core::period::Instant;
use uuid::Uuid;

use crate::aggregate::FailureAggregator;
use crate::audit::{AuditJournal, RequestContext};
use crate::authz::Subject;
use crate::port::StoreError;

/// What kind of identity authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityKind {
    /// A person, through an OIDC issuer.
    Person,
    /// A service account.
    ServiceAccount,
    /// A break-glass account.
    Breakglass,
}

/// How it authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// The session cookie of the BFF.
    OidcSession,
    /// An access token of an issuer.
    Bearer,
    /// Basic, a service account.
    Basic,
    /// A break-glass session.
    Breakglass,
}

impl Method {
    /// As recorded in the audit.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OidcSession => "oidc",
            Self::Bearer => "bearer",
            Self::Basic => "basic",
            Self::Breakglass => "breakglass",
        }
    }
}

/// Who a request comes from, once authenticated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// Internal identifier.
    pub identity: Uuid,
    /// Kind.
    pub kind: IdentityKind,
    /// Issuer, for a person.
    pub issuer: Option<String>,
    /// Groups at that issuer, read from the token.
    pub groups: Vec<String>,
    /// Method.
    pub method: Method,
}

impl Principal {
    /// The subject of its rights.
    #[must_use]
    pub fn subject(&self) -> Subject {
        Subject {
            id: self.identity,
            issuer: self.issuer.clone(),
            groups: self.groups.clone(),
        }
    }

    /// The context of its requests from `source`.
    #[must_use]
    pub fn context(&self, source: Source) -> RequestContext {
        let mut context = RequestContext::of(&self.subject());
        context.actor.auth_method = Some(self.method.as_str().to_owned());
        if self.kind == IdentityKind::ServiceAccount {
            context.actor.kind = ActorKind::ServiceAccount;
        }
        context.source = source;
        context
    }
}

/// A secret of a service account, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSecret {
    /// Identifier.
    pub id: Uuid,
    /// HMAC of the secret under the key `key_id`.
    pub digest: [u8; 32],
    /// The key of the ring that made the digest.
    pub key_id: String,
    /// Expiry.
    pub expires_at: Option<Instant>,
    /// Revoked.
    pub revoked: bool,
}

/// A service account, as authentication needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAccount {
    /// Identifier.
    pub id: Uuid,
    /// Disabled (REQ-005-3).
    pub disabled: bool,
    /// Its secrets.
    pub secrets: Vec<StoredSecret>,
}

/// Whether an identity may act (REQ-005-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityState {
    /// Disabled.
    pub disabled: bool,
    /// Its sessions opened before this instant are revoked.
    pub revoked_after: Option<Instant>,
}

/// A break-glass account, as authentication needs it (REQ-005-14, 15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakglassAccount {
    /// Identifier.
    pub id: Uuid,
    /// Argon2id hash of the passphrase.
    pub password_hash: String,
    /// The TOTP secret, sealed under the key ring.
    pub totp_sealed: Vec<u8>,
    /// The last TOTP step accepted (a code is never accepted twice).
    pub last_step: i64,
    /// Consecutive failures.
    pub failures: i32,
    /// End of its validity.
    pub valid_until: Instant,
    /// Disabled.
    pub disabled: bool,
}

/// The checks of a break-glass login (`kartenn-auth`).
pub trait BreakglassVerifier: Send + Sync {
    /// Whether `passphrase` matches `hash` (Argon2id).
    fn passphrase(&self, hash: &str, passphrase: &str) -> bool;
    /// The same work as [`Self::passphrase`], for an unknown name: the time of an answer
    /// does not tell whether the account exists.
    fn burn(&self, passphrase: &str);
    /// The step at which `code` is accepted at `now` (Unix seconds), after `last_step`.
    fn totp_step(&self, sealed: &[u8], code: &str, now: i64, last_step: i64) -> Option<i64>;
}

/// Consecutive failures that disable a break-glass account (NIST SP 800-63B §3.2.2).
pub const BREAKGLASS_FAILURES_MAX: i32 = 100;

/// Digests of secrets under the key ring (`kartenn-auth`).
pub trait SecretHasher: Send + Sync {
    /// The digest of `secret` under the key `key_id`, if the ring holds it.
    fn digest(&self, key_id: &str, secret: &str) -> Option<[u8; 32]>;
}

/// The identities, as authentication reads them (the store).
pub trait Identities: Send + Sync {
    /// The service account `name`, read now (no cache: revocation is immediate).
    fn service_account(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<Option<ServiceAccount>, StoreError>> + Send;

    /// The person (`issuer`, `subject`), created at its first login with `name` as display
    /// name, or updated (REQ-005-1, 2; K3): its identifier and whether it is disabled.
    fn upsert_person(
        &self,
        issuer: &str,
        subject: &str,
        name: &str,
    ) -> impl Future<Output = Result<(Uuid, bool), StoreError>> + Send;

    /// The state of an identity (REQ-005-3), if it exists.
    fn identity_state(
        &self,
        identity: Uuid,
    ) -> impl Future<Output = Result<Option<IdentityState>, StoreError>> + Send;

    /// The break-glass account `name`, read now.
    fn breakglass(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<Option<BreakglassAccount>, StoreError>> + Send;

    /// Records a break-glass attempt atomically, against concurrent attempts: a step is
    /// taken only if it is after the last one taken (a code is never accepted twice, even
    /// by two requests at once) and resets the failures; a failure increments them and,
    /// at [`BREAKGLASS_FAILURES_MAX`], disables the account.
    fn record_breakglass(
        &self,
        id: Uuid,
        attempt: BreakglassAttempt,
    ) -> impl Future<Output = Result<BreakglassRecorded, StoreError>> + Send;

    /// The digest of an identifier claimed by an unknown actor (REQ-012-9), in hexadecimal.
    fn claimed_digest(
        &self,
        claimed: &str,
    ) -> impl Future<Output = Result<String, StoreError>> + Send;

    /// A summary of the rights of `subject` now (`GET /v1/me`, 006).
    fn rights(&self, subject: &Subject) -> impl Future<Output = Result<Rights, StoreError>> + Send;

    /// The identity (`issuer`, `subject`) a service account names as its requester, an
    /// identity `requester` created if unknown (REQ-016-12): its identifier and whether it
    /// was created. A person already known keeps its identity and its kind.
    fn requester(
        &self,
        issuer: &str,
        subject: &str,
        name: Option<&str>,
    ) -> impl Future<Output = Result<(Uuid, bool), StoreError>> + Send;
}

/// The requester a service account names in `Kartenn-On-Behalf-Of` (REQ-016-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requester {
    /// Its subject at its issuer.
    pub sub: String,
    /// Its issuer; the namespace of the service account when absent.
    pub iss: Option<String>,
    /// Its display name.
    pub name: Option<String>,
}

/// The identity of the requester `principal` acts for (REQ-016-12, 13; G10): only a
/// service account that holds `core.identity.act_on_behalf` on the global scope names one;
/// unknown, it becomes an identity `requester`, recorded by `authn.requester.created`. The rights
/// of the request stay those of the service account alone.
///
/// # Errors
///
/// [`AuthnError::Refused`] when `principal` may not act on behalf of another;
/// [`AuthnError::Store`] or [`AuthnError::Audit`] on infrastructure failure.
pub async fn act_on_behalf<I: Identities, J: AuditJournal>(
    identities: &I,
    journal: &J,
    principal: &Principal,
    requester: &Requester,
    source: kartenn_core::audit::Source,
) -> Result<Uuid, AuthnError> {
    let permission = kartenn_core::authz::IDENTITY_ACT_ON_BEHALF.name;
    // A refused attempt to act for another is a signal: recorded before the refusal
    // (INV-60).
    let refusal = async |source: kartenn_core::audit::Source| {
        let event = principal
            .context(source)
            .event(
                &kartenn_core::audit::AUTHZ_DENIED,
                crate::audit::Outcome::Denied,
            )
            .and_then(|event| event.detail("permission", permission))
            .map_err(|e| AuthnError::Audit(StoreError(e.to_string())))?
            .target("person", [principal.identity]);
        journal.record(&event).await.map_err(AuthnError::Audit)?;
        Err(AuthnError::Refused)
    };
    if principal.kind != IdentityKind::ServiceAccount {
        return refusal(source).await;
    }
    let rights = identities
        .rights(&principal.subject())
        .await
        .map_err(AuthnError::Store)?;
    if !rights
        .global_permissions
        .iter()
        .any(|entry| kartenn_core::authz::grants(entry, permission, false))
    {
        return refusal(source).await;
    }
    // Without an issuer, the subject is the service account's own: its namespace.
    let issuer = requester
        .iss
        .clone()
        .unwrap_or_else(|| format!("urn:kartenn:service-account:{}", principal.identity));
    let (id, created) = identities
        .requester(&issuer, &requester.sub, requester.name.as_deref())
        .await
        .map_err(AuthnError::Store)?;
    if created {
        let event = principal
            .context(source)
            .event(
                &kartenn_core::audit::AUTHN_ON_BEHALF,
                crate::audit::Outcome::Success,
            )
            .map_err(|e| AuthnError::Audit(StoreError(e.to_string())))?
            .target("person", [id])
            .on_behalf_of(id);
        journal.record(&event).await.map_err(AuthnError::Audit)?;
    }
    Ok(id)
}

/// A summary of the rights of a subject now (`GET /v1/me`; 006, design « Droits »).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rights {
    /// Its display name.
    pub name: Option<String>,
    /// The keys of the roles it holds, on any scope.
    pub roles: Vec<String>,
    /// The bindings that serve it: its own, those of its groups and of its issuer.
    pub bindings: Vec<HeldBinding>,
    /// The entries of the roles of its global bindings.
    pub global_permissions: Vec<String>,
}

/// A binding that serves a subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldBinding {
    /// Role key.
    pub role: String,
    /// `global`, `entity`, `site`, `domain` or `prefix`.
    pub scope_kind: String,
    /// The scope; `None` for global.
    pub scope: Option<Uuid>,
}

/// Why an authentication fails.
#[derive(Debug, thiserror::Error)]
pub enum AuthnError {
    /// Refused: no reason given to the caller (the audit has it).
    #[error("authentication refused")]
    Refused,
    /// The identities cannot be read.
    #[error(transparent)]
    Store(StoreError),
    /// The refusal cannot be audited.
    #[error("audit: {0}")]
    Audit(StoreError),
}

/// Equality of two digests in constant time.
fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A failed authentication, as the audit records it (spec 005, design « Audit »).
#[derive(Debug, Clone, Copy)]
pub struct Failure<'a> {
    /// The identifier claimed, recorded as a digest only (REQ-012-9).
    pub claimed: &'a str,
    /// How the caller tried.
    pub method: Method,
    /// The issuer, for an OIDC login or an access token.
    pub issuer: Option<&'a str>,
    /// Coded reason: `secret`, `totp`, `state`, `iss`, `nonce`, `signature`, `expired`…
    pub reason: &'a str,
}

/// Records `failure` from `source`, through the aggregator of repeated failures
/// (REQ-012-19).
///
/// # Errors
///
/// [`AuthnError`] when the identities or the audit fail.
pub async fn failed<I: Identities, J: AuditJournal>(
    identities: &I,
    journal: &J,
    aggregator: &FailureAggregator,
    failure: Failure<'_>,
    source: Source,
    at: Instant,
) -> Result<(), AuthnError> {
    let Failure {
        claimed,
        method,
        issuer,
        reason,
    } = failure;
    let actor = Actor {
        kind: ActorKind::Unknown,
        id: None,
        auth_method: Some(method.as_str().to_owned()),
        issuer: issuer.map(str::to_owned),
        claimed: Some(
            identities
                .claimed_digest(claimed)
                .await
                .map_err(AuthnError::Store)?,
        ),
    };
    let event = AuditEvent::new(&AUTHN_LOGIN_FAILED, actor, Outcome::Failure, at)
        .and_then(|e| e.detail("method", method.as_str()))
        .map_err(|e| AuthnError::Audit(StoreError(e.to_string())))?
        .source(source)
        .reason(reason);
    for event in aggregator.failure(event, at) {
        journal.record(&event).await.map_err(AuthnError::Audit)?;
    }
    Ok(())
}

/// Authenticates the service account `name` by `secret` (REQ-005-11, 12): an active
/// secret (neither revoked nor expired) whose digest matches, on an enabled account. A
/// success is not audited on its own (the request is, with its actor); a failure is.
///
/// # Errors
///
/// [`AuthnError::Refused`] for any failure; [`AuthnError::Store`], [`AuthnError::Audit`].
#[allow(clippy::too_many_arguments)] // a proof, where it comes from and how it is recorded
pub async fn authenticate_service<I: Identities, H: SecretHasher, J: AuditJournal>(
    identities: &I,
    hasher: &H,
    journal: &J,
    aggregator: &FailureAggregator,
    name: &str,
    secret: &str,
    ip: Option<IpAddr>,
    at: Instant,
) -> Result<Principal, AuthnError> {
    let source = Source {
        ip,
        user_agent: None,
    };
    let account = identities
        .service_account(name)
        .await
        .map_err(AuthnError::Store)?;
    let reason = match &account {
        None => "unknown",
        Some(a) if a.disabled => "disabled",
        Some(a) => {
            let matched = a.secrets.iter().find(|s| {
                hasher
                    .digest(&s.key_id, secret)
                    .is_some_and(|d| same(&d, &s.digest))
            });
            match matched {
                None => "secret",
                Some(s) if s.revoked => "revoked",
                Some(s) if s.expires_at.is_some_and(|e| e <= at) => "expired",
                Some(_) => "",
            }
        }
    };
    if !reason.is_empty() {
        failed(
            identities,
            journal,
            aggregator,
            Failure {
                claimed: name,
                method: Method::Basic,
                issuer: None,
                reason,
            },
            source,
            at,
        )
        .await?;
        return Err(AuthnError::Refused);
    }
    let Some(account) = account else {
        return Err(AuthnError::Refused);
    };
    Ok(Principal {
        identity: account.id,
        kind: IdentityKind::ServiceAccount,
        issuer: None,
        groups: Vec::new(),
        method: Method::Basic,
    })
}

/// A person who completed a login at `issuer` as `subject` (REQ-005-1, 2): its identity,
/// created at the first login, and `authn.login.succeeded` recorded; a disabled identity
/// is refused, the refusal audited.
///
/// # Errors
///
/// [`AuthnError::Refused`] for a disabled identity; [`AuthnError::Store`],
/// [`AuthnError::Audit`].
#[allow(clippy::too_many_arguments)] // who, from where, and how it is recorded
pub async fn logged_in<I: Identities, J: AuditJournal>(
    identities: &I,
    journal: &J,
    aggregator: &FailureAggregator,
    who: (&str, &str, Option<&str>),
    groups: Vec<String>,
    method: Method,
    source: Source,
    at: Instant,
) -> Result<Principal, AuthnError> {
    let principal = seen(
        identities,
        journal,
        aggregator,
        who,
        groups,
        method,
        source.clone(),
        at,
    )
    .await?;
    let event = principal
        .context(source)
        .event(&AUTHN_LOGIN_SUCCEEDED, Outcome::Success)
        .and_then(|e| e.detail("method", method.as_str()))
        .map_err(|e| AuthnError::Audit(StoreError(e.to_string())))?;
    journal.record(&event).await.map_err(AuthnError::Audit)?;
    Ok(principal)
}

/// A person who presents an access token of `issuer` as `subject` (REQ-006-26): as
/// [`logged_in`], created just in time, a disabled identity refused and the refusal
/// audited, but no `authn.login.succeeded`: each request of a token is not a login (the
/// request itself is audited with its actor).
///
/// # Errors
///
/// [`AuthnError::Refused`] for a disabled identity; [`AuthnError::Store`],
/// [`AuthnError::Audit`].
#[allow(clippy::too_many_arguments)] // who, from where, and how it is recorded
pub async fn seen<I: Identities, J: AuditJournal>(
    identities: &I,
    journal: &J,
    aggregator: &FailureAggregator,
    (issuer, subject, name): (&str, &str, Option<&str>),
    groups: Vec<String>,
    method: Method,
    source: Source,
    at: Instant,
) -> Result<Principal, AuthnError> {
    let (identity, disabled) = identities
        .upsert_person(issuer, subject, name.unwrap_or(subject))
        .await
        .map_err(AuthnError::Store)?;
    if disabled {
        failed(
            identities,
            journal,
            aggregator,
            Failure {
                claimed: &format!("{issuer} {subject}"),
                method,
                issuer: Some(issuer),
                reason: "disabled",
            },
            source,
            at,
        )
        .await?;
        return Err(AuthnError::Refused);
    }
    Ok(Principal {
        identity,
        kind: IdentityKind::Person,
        issuer: Some(issuer.to_owned()),
        groups,
        method,
    })
}

/// The state of the identities, read at each request and kept a few seconds per instance
/// (K10): a change applied on this instance takes effect at once ([`StateCache::forget`]),
/// on the others within the time to live.
#[derive(Debug)]
pub struct StateCache {
    ttl: std::time::Duration,
    entries: std::sync::Mutex<std::collections::HashMap<Uuid, (IdentityState, std::time::Instant)>>,
}

impl StateCache {
    /// Entries kept `ttl` (5 s in production).
    #[must_use]
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            ttl,
            entries: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Forgets `identity`, after a change of its state on this instance.
    pub fn forget(&self, identity: Uuid) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&identity);
    }

    /// Forgets the entries past their time to live (the local timer of REQ-006-38).
    pub fn purge(&self) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|_, (_, at)| at.elapsed() < self.ttl);
    }

    async fn state<I: Identities>(
        &self,
        identities: &I,
        identity: Uuid,
    ) -> Result<Option<IdentityState>, StoreError> {
        let cached = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&identity)
            .filter(|(_, at)| at.elapsed() < self.ttl)
            .map(|(state, _)| *state);
        if let Some(state) = cached {
            return Ok(Some(state));
        }
        let state = identities.identity_state(identity).await?;
        if let Some(state) = state {
            self.entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(identity, (state, std::time::Instant::now()));
        }
        Ok(state)
    }

    /// Whether `principal`, in a session opened at `session_started` if any, may act now:
    /// its identity exists, is enabled, and its sessions were not revoked since; a refusal
    /// is audited (`authz.access.denied`, reason `identity.disabled` or `session.revoked`).
    ///
    /// # Errors
    ///
    /// [`AuthnError::Refused`]; [`AuthnError::Store`], [`AuthnError::Audit`].
    pub async fn check<I: Identities, J: AuditJournal>(
        &self,
        identities: &I,
        journal: &J,
        principal: &Principal,
        session_started: Option<Instant>,
        source: Source,
    ) -> Result<(), AuthnError> {
        let state = self
            .state(identities, principal.identity)
            .await
            .map_err(AuthnError::Store)?;
        let reason = match state {
            None | Some(IdentityState { disabled: true, .. }) => "identity.disabled",
            Some(IdentityState {
                revoked_after: Some(revoked),
                ..
            }) if session_started.is_some_and(|started| started <= revoked) => "session.revoked",
            Some(_) => return Ok(()),
        };
        let event = principal
            .context(source)
            .event(&kartenn_core::audit::AUTHZ_DENIED, Outcome::Denied)
            .map_err(|e| AuthnError::Audit(StoreError(e.to_string())))?
            .reason(reason);
        journal.record(&event).await.map_err(AuthnError::Audit)?;
        Err(AuthnError::Refused)
    }
}

/// Why a session ended (REQ-005-7, 8), as `authn.session.closed` records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEnd {
    /// The person logged out.
    Logout,
    /// No request within the inactivity limit.
    Idle,
    /// The absolute lifetime ran out.
    Absolute,
    /// The issuer refused to refresh it (groups, REQ-005-7).
    RefreshFailed,
    /// Its identity was disabled or its sessions revoked (REQ-005-3).
    Revoked,
}

impl SessionEnd {
    /// As recorded in the audit.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Logout => "logout",
            Self::Idle => "idle",
            Self::Absolute => "absolute",
            Self::RefreshFailed => "refresh_failed",
            Self::Revoked => "revoked",
        }
    }
}

/// Records the end of a session of `principal` (REQ-005-28), called by the HTTP layer
/// (006) where it sees the session end.
///
/// # Errors
///
/// [`AuthnError::Audit`] when the audit fails.
pub async fn session_closed<J: AuditJournal>(
    journal: &J,
    principal: &Principal,
    end: SessionEnd,
    source: Source,
) -> Result<(), AuthnError> {
    let event = principal
        .context(source)
        .event(&kartenn_core::audit::AUTHN_SESSION_CLOSED, Outcome::Success)
        .and_then(|e| e.detail("cause", end.as_str()))
        .map_err(|e| AuthnError::Audit(StoreError(e.to_string())))?;
    journal.record(&event).await.map_err(AuthnError::Audit)
}

/// A break-glass attempt, as recorded on its account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakglassAttempt {
    /// Both proofs hold, with the TOTP code of this step.
    Accepted(i64),
    /// A wrong passphrase or code.
    Failed,
}

/// What recording an attempt did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakglassRecorded {
    /// The step is taken: the login goes on.
    Taken,
    /// Another request took this step or a later one first: a replay.
    Replayed,
    /// The consecutive failures, this one included.
    Failures(i32),
}

/// Authenticates the break-glass account `name` by its passphrase and a TOTP code
/// (REQ-005-15): valid, enabled, the passphrase and a code never used; each use audited
/// at high criticality and logged as a WARN; 100 consecutive failures disable it.
///
/// # Errors
///
/// [`AuthnError::Refused`] for any failure; [`AuthnError::Store`], [`AuthnError::Audit`].
#[allow(clippy::too_many_arguments)] // a proof, where it comes from and how it is recorded
pub async fn authenticate_breakglass<I: Identities, V: BreakglassVerifier, J: AuditJournal>(
    identities: &I,
    verifier: &V,
    journal: &J,
    aggregator: &FailureAggregator,
    (name, passphrase, code): (&str, &str, &str),
    ip: Option<IpAddr>,
    at: Instant,
) -> Result<Principal, AuthnError> {
    let source = Source {
        ip,
        user_agent: None,
    };
    let account = identities
        .breakglass(name)
        .await
        .map_err(AuthnError::Store)?;
    let unix = at.0.div_euclid(1_000_000);
    let (mut reason, step) = match &account {
        None => {
            verifier.burn(passphrase);
            ("unknown", None)
        }
        Some(a) if a.disabled => ("disabled", None),
        Some(a) if a.valid_until <= at => ("expired", None),
        Some(a) if !verifier.passphrase(&a.password_hash, passphrase) => ("secret", None),
        Some(a) => match verifier.totp_step(&a.totp_sealed, code, unix, a.last_step) {
            None => ("totp", None),
            Some(step) => ("", Some(step)),
        },
    };
    if let Some(account) = &account
        && matches!(reason, "secret" | "totp" | "")
    {
        let mut recorded = identities
            .record_breakglass(
                account.id,
                step.map_or(BreakglassAttempt::Failed, BreakglassAttempt::Accepted),
            )
            .await
            .map_err(AuthnError::Store)?;
        if recorded == BreakglassRecorded::Replayed {
            // The same code twice at once: the second is a failure (RFC 6238 §5.2).
            reason = "totp";
            recorded = identities
                .record_breakglass(account.id, BreakglassAttempt::Failed)
                .await
                .map_err(AuthnError::Store)?;
        }
        if recorded == BreakglassRecorded::Failures(BREAKGLASS_FAILURES_MAX) {
            let locked = AuditEvent::new(
                &kartenn_core::audit::AUTHN_ACCOUNT_LOCKED,
                Actor::person(account.id),
                Outcome::Success,
                at,
            )
            .map_err(|e| AuthnError::Audit(StoreError(e.to_string())))?;
            journal.record(&locked).await.map_err(AuthnError::Audit)?;
        }
    }
    let Some(account) = account.filter(|_| reason.is_empty()) else {
        failed(
            identities,
            journal,
            aggregator,
            Failure {
                claimed: name,
                method: Method::Breakglass,
                issuer: None,
                reason,
            },
            source,
            at,
        )
        .await?;
        return Err(AuthnError::Refused);
    };
    let principal = Principal {
        identity: account.id,
        kind: IdentityKind::Breakglass,
        issuer: None,
        groups: Vec::new(),
        method: Method::Breakglass,
    };
    let event = principal
        .context(source)
        .event(
            &kartenn_core::audit::AUTHN_BREAKGLASS_USED,
            Outcome::Success,
        )
        .and_then(|e| e.detail("method", Method::Breakglass.as_str()))
        .map_err(|e| AuthnError::Audit(StoreError(e.to_string())))?;
    journal.record(&event).await.map_err(AuthnError::Audit)?;
    tracing::warn!(name: "authn.breakglass.used", identity = %account.id, "a break-glass account opened a session");
    Ok(principal)
}
