//! Audit journal port (spec 012, REQ-012-2, 9, 11; ADR 0019): the context of a request,
//! from which its events take their actor, source and correlation, and the port that
//! records an event outside a write transaction. Inside one, [`WriteTx::audit`]
//! records it with the action.
//!
//! [`WriteTx::audit`]: crate::WriteTx::audit

use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use kartenn_core::audit::{
    APPROVAL_POLICY_CHANGED, AUTHZ_DENIED, CHANGESET_APPLIED, CHANGESET_REFUSED,
    CHANGESET_REVALIDATION_FAILED, CHANGESET_SUBMITTED_PENDING, DECLARATION_CHANGED,
};
pub use kartenn_core::audit::{
    Actor, AuditError, AuditEvent, Correlation, EventKind, Outcome, Source,
};
use kartenn_core::period::Instant;
use kartenn_core::rules::{self, Violation};
use serde_json::json;
use uuid::Uuid;

use crate::pipeline::{Planned, RequestError};
use crate::port::{Journal, StoreError};

/// Who asks, from where, within which trace (REQ-012-6, 9). The authentication (005) and
/// the API (006) fill it; jobs use [`RequestContext::system`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContext {
    /// The actor.
    pub actor: Actor,
    /// The client.
    pub source: Source,
    /// Trace and request identifiers.
    pub correlation: Correlation,
}

impl RequestContext {
    /// kartenn itself, for jobs and migrations.
    #[must_use]
    pub fn system() -> Self {
        Self {
            actor: Actor::system(),
            source: Source::default(),
            correlation: Correlation::default(),
        }
    }

    /// A person, by internal identifier.
    #[must_use]
    pub fn person(id: Uuid) -> Self {
        Self {
            actor: Actor::person(id),
            source: Source::default(),
            correlation: Correlation::default(),
        }
    }

    /// An event of `kind` in this context, now.
    ///
    /// # Errors
    ///
    /// [`AuditError::Unattributed`] when the context has no established actor.
    pub fn event(
        &self,
        kind: &'static EventKind,
        outcome: Outcome,
    ) -> Result<AuditEvent, AuditError> {
        Ok(AuditEvent::new(kind, self.actor.clone(), outcome, now())?
            .source(self.source.clone())
            .correlation(self.correlation.clone()))
    }
}

/// The instance that records events (REQ-012-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// Version of the server.
    pub version: &'static str,
    /// Identifier of this process, drawn at start.
    pub id: Uuid,
}

impl Instance {
    /// This process, under `version`.
    #[must_use]
    pub fn new(version: &'static str) -> Self {
        Self {
            version,
            id: Uuid::now_v7(),
        }
    }
}

/// Records an audit event outside a write transaction, before the response (REQ-012-11).
pub trait AuditJournal: Send + Sync {
    /// Records `event`; an error must fail the action it describes.
    fn record(&self, event: &AuditEvent) -> impl Future<Output = Result<(), StoreError>> + Send;
}

/// Journal in memory, for tests.
#[derive(Debug, Clone, Default)]
pub struct MemoryJournal {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl MemoryJournal {
    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The events recorded so far, in order.
    #[must_use]
    pub fn events(&self) -> Vec<AuditEvent> {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl AuditJournal for MemoryJournal {
    fn record(&self, event: &AuditEvent) -> impl Future<Output = Result<(), StoreError>> + Send {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(event.clone());
        std::future::ready(Ok(()))
    }
}

/// Rules whose violation is a refusal of rights, audited as `authz.access.denied`.
const RIGHTS_RULES: [&str; 5] = [
    rules::PROP_FORBIDDEN_EFFECT.id,
    rules::POLICY_FORBIDDEN.id,
    rules::APPROVAL_FORBIDDEN.id,
    rules::APPROVAL_SELF.id,
    rules::CHANGESET_NOT_AUTHOR.id,
];

fn audit_err(e: &AuditError) -> RequestError {
    RequestError::Audit(StoreError(e.to_string()))
}

/// The `authz.access.denied` event of a refusal of rights, if `violation` is one.
///
/// # Errors
///
/// [`RequestError::Audit`] when the context has no actor.
pub(crate) fn denial(
    context: &RequestContext,
    violation: &Violation,
    changeset: Option<Uuid>,
) -> Result<Option<AuditEvent>, RequestError> {
    if !RIGHTS_RULES.contains(&violation.rule.id) {
        return Ok(None);
    }
    let mut event = context
        .event(&AUTHZ_DENIED, Outcome::Denied)
        .map_err(|e| audit_err(&e))?
        .target("object", violation.subjects.iter().copied())
        .reason(violation.rule.id);
    if let Some(changeset) = changeset {
        event = event.changeset(changeset);
    }
    Ok(Some(event))
}

/// The audit events of a journaled changeset (REQ-012-8): its state, the refusals of
/// rights among its violations, and, once applied, its approval policies and declarations.
///
/// # Errors
///
/// [`RequestError::Audit`] when the context has no actor.
pub(crate) fn changeset_events(
    context: &RequestContext,
    journal: &Journal<'_>,
    planned: &[Planned],
) -> Result<Vec<AuditEvent>, RequestError> {
    let errors: Vec<&str> = journal
        .violations
        .iter()
        .filter(|v| v.is_error())
        .map(|v| v.rule.id)
        .collect();
    let (kind, outcome, detail) = match journal.state {
        "applied" => (
            &CHANGESET_APPLIED,
            Outcome::Success,
            ("operations", json!(journal.request.operations.len())),
        ),
        "pending" => (
            &CHANGESET_SUBMITTED_PENDING,
            Outcome::Success,
            ("kind", json!(journal.kind)),
        ),
        "failed_revalidation" => (
            &CHANGESET_REVALIDATION_FAILED,
            Outcome::Failure,
            ("rules", json!(errors)),
        ),
        _ => (
            &CHANGESET_REFUSED,
            Outcome::Failure,
            ("rules", json!(errors)),
        ),
    };
    let event = |kind, outcome| -> Result<AuditEvent, RequestError> {
        Ok(context
            .event(kind, outcome)
            .map_err(|e| audit_err(&e))?
            .changeset(journal.id)
            .motive(journal.request.motive.clone()))
    };
    let mut events = vec![
        event(kind, outcome)?
            .target("changeset", [journal.id])
            .detail(detail.0, detail.1)
            .map_err(|e| audit_err(&e))?,
    ];
    for v in journal.violations {
        events.extend(denial(context, v, Some(journal.id))?);
    }
    for p in planned {
        match p {
            Planned::Policy(policy) => events.push(
                event(&APPROVAL_POLICY_CHANGED, Outcome::Success)?
                    .target("approval_policy", [policy.id])
                    .detail("enabled", policy.enabled)
                    .map_err(|e| audit_err(&e))?,
            ),
            Planned::Declare { id, declaration }
            | Planned::UpdateDeclaration { id, declaration } => events.push(
                event(&DECLARATION_CHANGED, Outcome::Success)?
                    .target("property", [*id])
                    .detail("key", declaration.key.to_string())
                    .map_err(|e| audit_err(&e))?,
            ),
            _ => {}
        }
    }
    Ok(events)
}

/// The current time as an [`Instant`].
#[must_use]
pub fn now() -> Instant {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_micros()).unwrap_or(i64::MAX));
    Instant(micros)
}

#[cfg(test)]
mod tests {
    use kartenn_core::audit::{AUTHZ_DENIED, SYS_STARTED};

    use super::*;

    #[tokio::test]
    async fn events_take_the_context_and_land_in_the_journal() {
        let journal = MemoryJournal::new();
        let person = Uuid::now_v7();
        let mut context = RequestContext::person(person);
        context.correlation.request_id = Some("req-1".into());
        let event = context.event(&AUTHZ_DENIED, Outcome::Denied).unwrap();
        journal.record(&event).await.unwrap();
        journal
            .record(
                &RequestContext::system()
                    .event(&SYS_STARTED, Outcome::Success)
                    .unwrap(),
            )
            .await
            .unwrap();
        let events = journal.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].actor.id, Some(person));
        assert_eq!(events[0].correlation.request_id.as_deref(), Some("req-1"));
        assert_eq!(events[1].kind.name, "sys.instance.started");
    }
}
