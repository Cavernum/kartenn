//! The audit of the event stream (spec 022, REQ-022-24; amends REQ-012-18 and INV-50,
//! A4): one event at the opening, one at the end with the objects notified, bounded by
//! `audit.read_ids_max`; beyond the bound, each delivery goes to the technical logs, at
//! DEBUG, as `events.delivery`, outside the sealed journal (R24).

use kartenn_core::audit::{AuditEvent, STREAM_CLOSED, STREAM_OPENED};
use kartenn_service::audit::{AuditJournal, Outcome, RequestContext};
use kartenn_service::events::Kind;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use uuid::Uuid;

use super::hub::Cause;
use super::stream::{Report, Tally};

/// The target kind of the objects notified: of any type.
const TARGET: &str = "object";

/// `events.stream.opened`.
///
/// # Errors
///
/// When the event cannot be built.
pub fn opened(
    context: &RequestContext,
    criteria: serde_json::Value,
    resumed: bool,
) -> Result<AuditEvent, kartenn_core::audit::AuditError> {
    context
        .event(&STREAM_OPENED, Outcome::Success)?
        .detail("criteria", criteria)?
        .detail("resumed", resumed)
}

/// `events.stream.closed`.
///
/// # Errors
///
/// When the event cannot be built.
pub fn closed(
    context: &RequestContext,
    cause: Cause,
    duration_ms: u64,
    tally: &Tally,
) -> Result<AuditEvent, kartenn_core::audit::AuditError> {
    let mut event = context
        .event(&STREAM_CLOSED, Outcome::Success)?
        .target(TARGET, tally.objects.iter().copied())
        .detail("cause", cause.as_str())?
        .detail("duration_ms", duration_ms)?
        .detail("delivered", tally.delivered)?
        .detail("resets", tally.resets)?;
    if tally.omitted > 0 {
        event = event.detail("targets_omitted", tally.omitted)?;
    }
    Ok(event)
}

/// The report of a connection to the audit journal.
#[derive(Debug, Clone)]
pub struct Audited {
    /// The journal.
    pub journal: PgAuditJournal,
    /// The context of the request that opened the stream: its actor.
    pub context: RequestContext,
    /// `audit.read_ids_max`.
    pub ids_max: usize,
}

impl Report for Audited {
    fn ids_max(&self) -> usize {
        self.ids_max
    }

    fn beyond(&self, conn: u64, identity: Uuid, kind: Kind, objects: &[Uuid]) {
        tracing::debug!(
            name: "events.delivery",
            connection = conn,
            actor = %identity,
            kind = kind.as_str(),
            objects = ?objects,
            "an event delivered beyond the bound of the audit"
        );
    }

    fn closed(&self, cause: Cause, duration_ms: u64, tally: Tally) {
        let event = match closed(&self.context, cause, duration_ms, &tally) {
            Ok(event) => event,
            Err(e) => {
                tracing::error!(name: "audit.write.failed", error = %e, "the end of an event stream could not be recorded");
                return;
            }
        };
        let journal = self.journal.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if let Err(e) = journal.record(&event).await {
                    tracing::error!(name: "audit.write.failed", error = %e.0, "the end of an event stream could not be recorded");
                }
            });
        }
    }
}
