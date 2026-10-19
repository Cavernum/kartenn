//! Audited reads (spec 012, REQ-012-17, 18): every read records who read which objects,
//! never the values, before the result is returned; reading a sensitive property, or the
//! audit journal itself, is kept longer. If the event cannot be recorded, the read fails
//! (REQ-012-11).

use std::collections::BTreeSet;

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

/// Permission to read the changesets, their operations and differential (006/REQ-006-16),
/// on [`CHANGESETS`]; global only.
pub const CHANGESET_READ_PERMISSION: &str = "core.changeset.read";

/// The object that stands for the journal of the changesets in authorisations.
pub const CHANGESETS: Uuid = Uuid::from_u128(3);

/// Permission to read the administration of the rights (006/REQ-006-4; design « Droits »):
/// roles, bindings, attachments, service accounts and identities, on
/// [`crate::rbac::RBAC_OBJECT`]; global only.
pub const RBAC_READ_PERMISSION: &str = "core.rbac.read";

/// Identifiers a read event records by default: a page of results.
pub const DEFAULT_READ_IDS_MAX: usize = 100;

/// How much a read event records (spec 012, REQ-012-18; 012/L25), the main lever on the
/// volume of the audit: at most `ids_max` identifiers of the objects returned, the others
/// counted in `targets_omitted`; `0` records the criteria alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadAudit {
    /// Most identifiers recorded per read event.
    pub ids_max: usize,
}

impl Default for ReadAudit {
    fn default() -> Self {
        Self {
            ids_max: DEFAULT_READ_IDS_MAX,
        }
    }
}

/// The `data.objects.read` event of a read by `context` that returned `ids` (of
/// `target_kind`) for `criteria`, within the bound of `audit`.
///
/// # Errors
///
/// The catalogue refuses a detail (an error of the code, not of the request).
pub fn objects_read(
    context: &RequestContext,
    target_kind: &str,
    ids: &[Uuid],
    criteria: serde_json::Value,
    audit: ReadAudit,
) -> Result<AuditEvent, kartenn_core::audit::AuditError> {
    let kept = ids.len().min(audit.ids_max);
    let mut event = context
        .event(&OBJECTS_READ, Outcome::Success)?
        .target(target_kind, ids[..kept].iter().copied())
        .detail("criteria", criteria)?;
    if kept < ids.len() {
        event = event.detail("targets_omitted", ids.len() - kept)?;
    }
    Ok(event)
}

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

/// The object `id` valid at `valid_at`, as asserted at `as_of`, read by `context`, under
/// the rights of its subject (005): `core.<kind>.read` on the object, or the object is
/// answered as absent and `authz.access.denied` recorded (its existence is not
/// disclosed, REQ-005-19); the value of a sensitive property needs `prop.<key>.read`, or it
/// is masked (REQ-005-21). Recorded: `data.objects.read` (within `audit`), plus
/// `data.sensitive.read` for each sensitive value read.
///
/// # Errors
///
/// [`RequestError::Audit`] when the read cannot be audited (nothing is returned);
/// [`RequestError::Store`] on infrastructure failure.
#[allow(clippy::too_many_arguments)] // a read, its reader, its rights and its audit
pub async fn read_object<R: Reader, A: Authorizer, J: AuditJournal>(
    reader: &R,
    authorizer: &A,
    journal: &J,
    context: &RequestContext,
    id: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
    audit: ReadAudit,
) -> Result<Option<ObjectRead>, RequestError> {
    let mut read = reader.object(id, valid_at, as_of).await?;
    if let (Some(found), Some(subject)) = (read.as_mut(), context.subject()) {
        let permission = format!("core.{}.read", found.kind);
        if !authorizer
            .authorize_many(&subject, &permission, &[id])
            .await?
            .is_empty()
        {
            let denied = context
                .event(&AUTHZ_DENIED, Outcome::Denied)
                .map_err(audit_err)?
                .target("object", [id])
                .reason(&permission)
                .detail("permission", permission.as_str())
                .map_err(audit_err)?;
            journal.record(&denied).await.map_err(RequestError::Audit)?;
            return Ok(None);
        }
        for key in found.sensitive.clone() {
            if !authorizer
                .authorize_many(&subject, &format!("prop.{key}.read"), &[id])
                .await?
                .is_empty()
            {
                found.properties.remove(&key);
                found.sensitive.remove(&key);
                found.masked.insert(key);
            }
        }
    }
    let criteria = json!({
        "object": id,
        "valid_at": rfc3339(valid_at),
        "as_of": as_of.map(rfc3339),
    });
    let found: Vec<Uuid> = read.iter().map(|_| id).collect();
    let mut events =
        vec![objects_read(context, "object", &found, criteria, audit).map_err(audit_err)?];
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

/// The objects among `objects` on which the subject of `context` may see the value of the
/// sensitive property `key` (`prop.<key>.read`, REQ-005-21), each disclosure recorded as
/// `data.sensitive.read` (REQ-012-18); all of them for a read without a subject.
///
/// # Errors
///
/// [`RequestError::Audit`] when a disclosure cannot be audited; [`RequestError::Store`]
/// on infrastructure failure.
pub async fn sensitive_disclosed<A: Authorizer, J: AuditJournal>(
    authorizer: &A,
    journal: &J,
    context: &RequestContext,
    key: &str,
    objects: &[Uuid],
) -> Result<BTreeSet<Uuid>, RequestError> {
    let mut disclosed: BTreeSet<Uuid> = objects.iter().copied().collect();
    if let Some(subject) = context.subject() {
        let denied = authorizer
            .authorize_many(&subject, &format!("prop.{key}.read"), objects)
            .await?;
        for id in denied {
            disclosed.remove(&id);
        }
    }
    let mut events = Vec::with_capacity(disclosed.len());
    for id in &disclosed {
        events.push(
            context
                .event(&SENSITIVE_READ, Outcome::Success)
                .map_err(audit_err)?
                .target("object", [*id])
                .detail("property", key)
                .map_err(audit_err)?,
        );
    }
    record_all(journal, &events).await?;
    Ok(disclosed)
}

/// Whether the actor of `context` holds `permission` on the sentinel `object` (a global
/// permission); a refusal is audited (`authz.access.denied`, target `target`).
async fn globally<A: Authorizer, J: AuditJournal>(
    authorizer: &A,
    journal: &J,
    context: &RequestContext,
    permission: &str,
    (target, object): (&str, Uuid),
) -> Result<(), RequestError> {
    let Some(actor) = context.subject() else {
        return Ok(());
    };
    let denied = authorizer
        .authorize_many(&actor, permission, &[object])
        .await?;
    if denied.is_empty() {
        return Ok(());
    }
    let event = context
        .event(&AUTHZ_DENIED, Outcome::Denied)
        .map_err(audit_err)?
        .target(target, [object])
        .reason(permission)
        .detail("permission", permission)
        .map_err(audit_err)?;
    record_all(journal, &[event]).await?;
    Err(RequestError::Forbidden(permission.into()))
}

/// Whether the actor of `context` may read the changesets ([`CHANGESET_READ_PERMISSION`]);
/// a refusal is audited (`authz.access.denied`).
///
/// # Errors
///
/// [`RequestError::Forbidden`] without the permission; [`RequestError::Audit`] when the
/// refusal cannot be audited; [`RequestError::Store`] on infrastructure failure.
pub async fn changesets_readable<A: Authorizer, J: AuditJournal>(
    authorizer: &A,
    journal: &J,
    context: &RequestContext,
) -> Result<(), RequestError> {
    globally(
        authorizer,
        journal,
        context,
        CHANGESET_READ_PERMISSION,
        ("changeset", CHANGESETS),
    )
    .await
}

/// Whether the actor of `context` may read the administration of the rights
/// ([`RBAC_READ_PERMISSION`]); a refusal is audited (`authz.access.denied`).
///
/// # Errors
///
/// As [`changesets_readable`].
pub async fn rbac_readable<A: Authorizer, J: AuditJournal>(
    authorizer: &A,
    journal: &J,
    context: &RequestContext,
) -> Result<(), RequestError> {
    globally(
        authorizer,
        journal,
        context,
        RBAC_READ_PERMISSION,
        ("rbac", crate::rbac::RBAC_OBJECT),
    )
    .await
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
    let allowed = match context.subject() {
        // kartenn itself (an export job) reads its own journal.
        None => true,
        Some(actor) => authorizer
            .authorize_many(&actor, AUDIT_READ_PERMISSION, &[AUDIT_JOURNAL])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_event_records_at_most_its_bound() {
        let ids: Vec<Uuid> = (0..250).map(|_| Uuid::now_v7()).collect();
        let context = RequestContext::system();
        let event =
            objects_read(&context, "address", &ids, json!({}), ReadAudit::default()).unwrap();
        assert_eq!(event.target.as_ref().unwrap().ids, ids[..100]);
        assert_eq!(event.details["targets_omitted"], 150);
        let event = objects_read(
            &context,
            "address",
            &ids[..3],
            json!({}),
            ReadAudit::default(),
        )
        .unwrap();
        assert_eq!(event.target.as_ref().unwrap().ids.len(), 3);
        assert!(
            !event.details.contains_key("targets_omitted"),
            "nothing left out"
        );
        // The criteria alone.
        let event = objects_read(
            &context,
            "address",
            &ids,
            json!({ "page": 1 }),
            ReadAudit { ids_max: 0 },
        )
        .unwrap();
        assert!(event.target.as_ref().unwrap().ids.is_empty());
        assert_eq!(event.details["targets_omitted"], 250);
        assert_eq!(event.details["criteria"]["page"], 1);
    }
}
