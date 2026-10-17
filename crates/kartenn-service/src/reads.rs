//! Audited reads (spec 012, REQ-012-17, 18): every read records who read which objects,
//! never the values, before the result is returned; reading a sensitive property, or the
//! audit journal itself, is kept longer. If the event cannot be recorded, the read fails
//! (REQ-012-11).

use kartenn_core::audit::{
    AUDIT_READ, AUTHZ_DENIED, AuditEvent, OBJECTS_READ, SENSITIVE_READ, rfc3339,
};
use kartenn_core::period::Instant;
use serde_json::json;
use uuid::Uuid;

use crate::audit::{AuditJournal, Outcome, RequestContext};
use crate::authz::Authorizer;
use crate::pipeline::RequestError;
use crate::port::{AuditQuery, ObjectRead, Reader, StoreError};

/// Permission to consult or export the audit journal (REQ-012-17), on [`AUDIT_JOURNAL`];
/// until the catalogue of permissions of spec 005.
pub const AUDIT_READ_PERMISSION: &str = "core.audit.read";

/// The object that stands for the audit journal in authorisations.
pub const AUDIT_JOURNAL: Uuid = Uuid::nil();

fn audit_err(e: impl std::fmt::Display) -> RequestError {
    RequestError::Audit(StoreError(e.to_string()))
}

async fn record_all<J: AuditJournal>(
    journal: &J,
    events: &[AuditEvent],
) -> Result<(), RequestError> {
    for event in events {
        journal.record(event).await.map_err(RequestError::Audit)?;
    }
    Ok(())
}

/// The object `id` valid at `valid_at`, as asserted at `as_of`, read by `context`:
/// `data.objects.read`, plus `data.sensitive.read` for each sensitive property in it.
///
/// # Errors
///
/// [`RequestError::Audit`] when the read cannot be audited (nothing is returned);
/// [`RequestError::Store`] on infrastructure failure.
pub async fn read_object<R: Reader, J: AuditJournal>(
    reader: &R,
    journal: &J,
    context: &RequestContext,
    id: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Option<ObjectRead>, RequestError> {
    let read = reader.object(id, valid_at, as_of).await?;
    let criteria = json!({
        "object": id,
        "valid_at": rfc3339(valid_at),
        "as_of": as_of.map(rfc3339),
    });
    let found: Vec<Uuid> = read.iter().map(|_| id).collect();
    let mut events = vec![
        context
            .event(&OBJECTS_READ, Outcome::Success)
            .map_err(audit_err)?
            .target("object", found.iter().copied())
            .detail("criteria", criteria)
            .map_err(audit_err)?,
    ];
    for key in read.iter().flat_map(|r| &r.sensitive) {
        events.push(
            context
                .event(&SENSITIVE_READ, Outcome::Success)
                .map_err(audit_err)?
                .target("object", [id])
                .detail("property", key.as_str())
                .map_err(audit_err)?,
        );
    }
    record_all(journal, &events).await?;
    Ok(read)
}

/// Audit events matching `query`, for an actor holding [`AUDIT_READ_PERMISSION`]; the
/// consultation is itself audited (`audit.events.read`), and so is a refusal.
///
/// # Errors
///
/// [`RequestError::Forbidden`] without the permission; [`RequestError::Audit`] when the
/// consultation cannot be audited; [`RequestError::Store`] on infrastructure failure.
pub async fn read_audit<R: Reader, A: Authorizer, J: AuditJournal>(
    reader: &R,
    authorizer: &A,
    journal: &J,
    context: &RequestContext,
    query: &AuditQuery,
) -> Result<Vec<serde_json::Value>, RequestError> {
    let allowed = match context.actor.id {
        // kartenn itself (an export job) reads its own journal.
        None => true,
        Some(actor) => authorizer
            .authorize_many(actor, AUDIT_READ_PERMISSION, &[AUDIT_JOURNAL])
            .await?
            .is_empty(),
    };
    if !allowed {
        let denied = context
            .event(&AUTHZ_DENIED, Outcome::Denied)
            .map_err(audit_err)?
            .target("audit", [AUDIT_JOURNAL])
            .reason(AUDIT_READ_PERMISSION)
            .detail("permission", AUDIT_READ_PERMISSION)
            .map_err(audit_err)?;
        record_all(journal, &[denied]).await?;
        return Err(RequestError::Forbidden(AUDIT_READ_PERMISSION.into()));
    }
    let events = reader.audit_events(query).await?;
    let read = context
        .event(&AUDIT_READ, Outcome::Success)
        .map_err(audit_err)?
        .target("audit", [AUDIT_JOURNAL])
        .detail(
            "criteria",
            json!({
                "from": query.from.map(rfc3339),
                "to": query.to.map(rfc3339),
                "name": query.name,
                "actor": query.actor,
                "object": query.object,
                "limit": query.limit,
            }),
        )
        .map_err(audit_err)?
        .detail("count", events.len())
        .map_err(audit_err)?;
    record_all(journal, &[read]).await?;
    Ok(events)
}
