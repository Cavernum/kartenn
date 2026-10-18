//! Audit journal port (spec 012, REQ-012-2, 9, 11; ADR 0019): the context of a request,
//! from which its events take their actor, source and correlation, and the port that
//! records an event outside a write transaction. Inside one, [`WriteTx::audit`]
//! records it with the action.
//!
//! [`WriteTx::audit`]: crate::WriteTx::audit

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// Groups of the actor at its issuer, read from its token (005/K4); empty otherwise.
    pub groups: Vec<String>,
}

impl RequestContext {
    /// The subject whose rights the request uses (005): the actor, its issuer and groups;
    /// `None` for kartenn itself.
    #[must_use]
    pub fn subject(&self) -> Option<crate::authz::Subject> {
        self.actor.id.map(|id| crate::authz::Subject {
            id,
            issuer: self.actor.issuer.clone(),
            groups: self.groups.clone(),
        })
    }

    /// The context of a subject: a person with its issuer and groups.
    #[must_use]
    pub fn of(subject: &crate::authz::Subject) -> Self {
        let mut context = Self::person(subject.id);
        context.actor.issuer.clone_from(&subject.issuer);
        context.groups.clone_from(&subject.groups);
        context
    }

    /// kartenn itself, for jobs and migrations.
    #[must_use]
    pub fn system() -> Self {
        Self {
            actor: Actor::system(),
            source: Source::default(),
            correlation: Correlation::default(),
            groups: Vec::new(),
        }
    }

    /// A person, by internal identifier.
    #[must_use]
    pub fn person(id: Uuid) -> Self {
        Self {
            actor: Actor::person(id),
            source: Source::default(),
            correlation: Correlation::default(),
            groups: Vec::new(),
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

/// Why an output of the audit did not take a batch.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SinkError(pub String);

/// An output of the audit journal (REQ-012-15): it receives the stored rows, as JSON, in
/// sequence order, and returns once they are written; an error leaves them for the next
/// attempt (at least once).
pub trait AuditSink: Send + Sync {
    /// Stable name, the key of its deliveries.
    fn name(&self) -> &str;

    /// Writes `events`.
    fn deliver(
        &self,
        events: &[serde_json::Value],
    ) -> impl Future<Output = Result<(), SinkError>> + Send;
}

/// Availability of an output (REQ-012-26): which event to record after a delivery.
#[derive(Debug, Clone, Default)]
pub struct SinkHealth {
    failing_since: Option<Instant>,
    reported: bool,
}

/// How long an output fails before it is reported unavailable.
pub const UNAVAILABLE_AFTER_MICROS: i64 = 60_000_000;

impl SinkHealth {
    /// The outcome of a delivery at `at`: `sys.output.unavailable` once an output has
    /// failed for a minute, `sys.output.available` when it comes back after that.
    pub fn observe(&mut self, ok: bool, at: Instant) -> Option<&'static EventKind> {
        if ok {
            self.failing_since = None;
            if std::mem::take(&mut self.reported) {
                return Some(&kartenn_core::audit::SYS_OUTPUT_AVAILABLE);
            }
            return None;
        }
        let since = *self.failing_since.get_or_insert(at);
        if !self.reported && at.0 - since.0 >= UNAVAILABLE_AFTER_MICROS {
            self.reported = true;
            return Some(&kartenn_core::audit::SYS_OUTPUT_UNAVAILABLE);
        }
        None
    }

    /// Whether the output is reported unavailable (failing for a minute or more).
    #[must_use]
    pub const fn unavailable(&self) -> bool {
        self.reported
    }
}

/// The number of audit outputs reported unavailable, shared between the relays that
/// observe them and the readiness probe (006, REQ-006-36).
#[derive(Debug, Clone, Default)]
pub struct AuditStatus(Arc<AtomicUsize>);

impl AuditStatus {
    /// Records the state of every output after a relay pass.
    pub fn update<'a>(&self, outputs: impl IntoIterator<Item = &'a SinkHealth>) {
        let down = outputs.into_iter().filter(|h| h.unavailable()).count();
        self.0.store(down, Ordering::Relaxed);
    }

    /// Whether every output is available.
    #[must_use]
    pub fn available(&self) -> bool {
        self.0.load(Ordering::Relaxed) == 0
    }
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
            // The administration of the RBAC (005, REQ-005-28): roles, bindings and
            // service accounts each have their event; attachments and identity states are
            // audited by the changeset.
            Planned::Rbac(op) => events.extend(rbac_event(&event, op)?),
            Planned::Modify {
                kind: crate::ObjectKind::Property,
                id,
                change: crate::port::Change::Sensitive(sensitive),
                ..
            } => events.push(
                event(&DECLARATION_CHANGED, Outcome::Success)?
                    .target("property", [*id])
                    .detail("sensitive", *sensitive)
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

/// The event of an operation of RBAC administration, if it has its own.
fn rbac_event(
    event: &impl Fn(&'static EventKind, Outcome) -> Result<AuditEvent, RequestError>,
    op: &crate::rbac::RbacOp,
) -> Result<Option<AuditEvent>, RequestError> {
    use crate::rbac::RbacOp;
    use kartenn_core::audit::{RBAC_BINDING_CHANGED, RBAC_ROLE_CHANGED, SERVICE_ACCOUNT_CHANGED};
    let account = |id: Uuid, change: &str| -> Result<AuditEvent, RequestError> {
        event(&SERVICE_ACCOUNT_CHANGED, Outcome::Success)?
            .target("service_account", [id])
            .detail("change", change)
            .map_err(|e| audit_err(&e))
    };
    Ok(Some(match op {
        RbacOp::SetRole { key, .. } | RbacOp::CloseRole { key } => {
            event(&RBAC_ROLE_CHANGED, Outcome::Success)?
                .target("role", [crate::rbac::RBAC_OBJECT])
                .detail("role", key.as_str())
                .map_err(|e| audit_err(&e))?
        }
        RbacOp::Bind {
            id,
            role,
            scope_kind,
            scope,
            ..
        } => event(&RBAC_BINDING_CHANGED, Outcome::Success)?
            .target("binding", [*id])
            .detail("role", role.as_str())
            .and_then(|e| {
                e.detail(
                    "scope",
                    scope.map_or_else(|| (*scope_kind).to_owned(), |s| format!("{scope_kind}:{s}")),
                )
            })
            .map_err(|e| audit_err(&e))?,
        // Its role and scope are those of the binding it ends, in the changeset.
        RbacOp::Unbind { id } => {
            event(&RBAC_BINDING_CHANGED, Outcome::Success)?.target("binding", [*id])
        }
        RbacOp::CreateAccount { id, .. } => account(*id, "created")?,
        RbacOp::AddSecret { account: id, .. } => account(*id, "secret_added")?,
        RbacOp::RevokeSecret { account: id, .. } => account(*id, "secret_revoked")?,
        RbacOp::Attach { .. } | RbacOp::Detach { .. } | RbacOp::IdentityState { .. } => {
            return Ok(None);
        }
    }))
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

    #[test]
    fn an_output_is_reported_unavailable_after_a_minute_then_available() {
        let mut health = SinkHealth::default();
        let s = |secs: i64| Instant(secs * 1_000_000);
        assert_eq!(health.observe(false, s(0)), None);
        assert_eq!(health.observe(false, s(59)), None);
        assert_eq!(
            health.observe(false, s(60)).map(|k| k.name),
            Some("sys.output.unavailable")
        );
        assert_eq!(health.observe(false, s(120)), None, "reported once");
        assert_eq!(
            health.observe(true, s(130)).map(|k| k.name),
            Some("sys.output.available")
        );
        assert_eq!(health.observe(true, s(131)), None);
        // A short failure is not reported.
        assert_eq!(health.observe(false, s(200)), None);
        assert_eq!(health.observe(true, s(210)), None);
    }

    #[test]
    fn the_status_counts_the_outputs_reported_unavailable() {
        let s = |secs: i64| Instant(secs * 1_000_000);
        let status = AuditStatus::default();
        let (mut up, mut down) = (SinkHealth::default(), SinkHealth::default());
        up.observe(true, s(0));
        down.observe(false, s(0));
        status.update([&up, &down]);
        assert!(
            status.available(),
            "a failure under a minute is not reported"
        );
        down.observe(false, s(60));
        status.clone().update([&up, &down]);
        assert!(!status.available(), "clones share the state");
        down.observe(true, s(61));
        status.update([&up, &down]);
        assert!(status.available());
    }

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
