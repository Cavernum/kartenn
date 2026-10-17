//! Audit journal port (spec 012, REQ-012-2, 9, 11; ADR 0019): the context of a request,
//! from which its events take their actor, source and correlation, and the port that
//! records an event outside a write transaction. Inside one, [`WriteTx::audit`]
//! records it with the action.
//!
//! [`WriteTx::audit`]: crate::WriteTx::audit

use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

pub use kartenn_core::audit::{
    Actor, AuditError, AuditEvent, Correlation, EventKind, Outcome, Source,
};
use kartenn_core::period::Instant;
use uuid::Uuid;

use crate::port::StoreError;

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
