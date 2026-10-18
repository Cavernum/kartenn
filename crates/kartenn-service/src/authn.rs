//! Authentication (spec 005, REQ-005-1 to 15): from a verified proof to a [`Principal`],
//! through ports, so that the cryptography and the identity providers stay in the
//! adapters (`kartenn-auth`) and the identities in the store.
//!
//! A refusal never tells why to the caller (one error, [`AuthnError::Refused`]); the audit
//! records the reason, and the claimed identifier only as a digest (REQ-012-9).

use std::future::Future;
use std::net::IpAddr;

use kartenn_core::audit::{AUTHN_LOGIN_FAILED, Actor, ActorKind, AuditEvent, Outcome, Source};
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

    /// The digest of an identifier claimed by an unknown actor (REQ-012-9), in hexadecimal.
    fn claimed_digest(
        &self,
        claimed: &str,
    ) -> impl Future<Output = Result<String, StoreError>> + Send;
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

/// Records a failed authentication of `claimed` from `source` for `reason`, through the
/// aggregator of repeated failures (REQ-012-19).
///
/// # Errors
///
/// [`AuthnError`] when the identities or the audit fail.
pub async fn failed<I: Identities, J: AuditJournal>(
    identities: &I,
    journal: &J,
    aggregator: &FailureAggregator,
    (claimed, method, reason): (&str, Method, &str),
    source: Source,
    at: Instant,
) -> Result<(), AuthnError> {
    let actor = Actor {
        kind: ActorKind::Unknown,
        id: None,
        auth_method: Some(method.as_str().to_owned()),
        issuer: None,
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
            (name, Method::Basic, reason),
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
