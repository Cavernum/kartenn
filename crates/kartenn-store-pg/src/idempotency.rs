//! Idempotency keys (004/REQ-004-10, 006/REQ-006-15): the journal of the first
//! changeset of a key, from which its response is rebuilt, and the purge of the expired
//! keys.

use kartenn_core::rules::{self, Violation};
use kartenn_service::ObjectKind;
use kartenn_service::port::StoreError;
use sqlx::PgPool;
use sqlx::types::Uuid;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// What the journal of a changeset says of its submission.
#[derive(Debug, Clone)]
pub struct Journaled {
    /// `applied`, `refused`, `pending`…
    pub state: String,
    /// `direct`, `approval` or `proposal`.
    pub kind: String,
    /// The approval policy that held it.
    pub policy: Option<Uuid>,
    /// Errors, then warnings, each in the order of the journal.
    pub violations: Vec<Violation>,
    /// The differential, counted, as journaled (REQ-003-19).
    pub diff: serde_json::Value,
    /// Closures added by a cascade, in the order of the expansion (REQ-004-26).
    pub cascaded: Vec<(ObjectKind, Uuid)>,
    /// Why its author could not apply it when asking to (spec 024, REQ-024-13).
    pub reason: Option<Violation>,
}

fn field<'a>(
    entry: &'a serde_json::Value,
    name: &str,
) -> Result<&'a serde_json::Value, StoreError> {
    entry
        .get(name)
        .ok_or_else(|| store_err(format!("the journal lacks {name}")))
}

fn uuid(value: &serde_json::Value) -> Result<Uuid, StoreError> {
    value
        .as_str()
        .ok_or_else(|| store_err("an identifier of the journal is not a string"))?
        .parse()
        .map_err(store_err)
}

fn entries(stored: &serde_json::Value) -> Result<&Vec<serde_json::Value>, StoreError> {
    stored
        .as_array()
        .ok_or_else(|| store_err("a list of the journal is not an array"))
}

/// The violations as journaled by the store (`rule`, `message`, `subjects`, `operation`).
pub(crate) fn violations(stored: &serde_json::Value) -> Result<Vec<Violation>, StoreError> {
    entries(stored)?
        .iter()
        .map(|entry| {
            let id = field(entry, "rule")?.as_str().unwrap_or_default();
            let rule = rules::rule(id)
                .ok_or_else(|| store_err(format!("unknown rule {id} in the journal")))?;
            Ok(Violation {
                rule,
                subjects: entries(field(entry, "subjects")?)?
                    .iter()
                    .map(uuid)
                    .collect::<Result<_, _>>()?,
                message: field(entry, "message")?
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                operation: field(entry, "operation")?
                    .as_u64()
                    .map(usize::try_from)
                    .transpose()
                    .map_err(store_err)?,
            })
        })
        .collect()
}

/// The closures of a cascade as journaled: `[{kind, id}]`.
pub(crate) fn closures(stored: &serde_json::Value) -> Result<Vec<(ObjectKind, Uuid)>, StoreError> {
    entries(stored)?
        .iter()
        .map(|entry| {
            let kind = serde_json::from_value(field(entry, "kind")?.clone()).map_err(store_err)?;
            Ok((kind, uuid(field(entry, "id")?)?))
        })
        .collect()
}

/// The journal of changeset `id`, if it exists.
///
/// # Errors
///
/// Database or decoding errors.
pub async fn journaled(pool: &PgPool, id: Uuid) -> Result<Option<Journaled>, StoreError> {
    type Row = (
        String,
        String,
        Option<Uuid>,
        serde_json::Value,
        serde_json::Value,
        serde_json::Value,
        serde_json::Value,
        Option<serde_json::Value>,
    );
    let row: Option<Row> = sqlx::query_as(
        "SELECT state, kind, policy_id, violations, anomalies, diff_summary, cascaded,
                required_rights -> 'reason'
         FROM changeset WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(store_err)?;
    let Some((state, kind, policy, errors, warnings, diff, cascaded, reason)) = row else {
        return Ok(None);
    };
    let mut found = violations(&errors)?;
    found.extend(violations(&warnings)?);
    let reason = reason
        .filter(|r| !r.is_null())
        .map(serde_json::from_value::<(String, Vec<Uuid>)>)
        .transpose()
        .map_err(store_err)?
        .map(|(message, objects)| {
            Violation::new(&rules::APPROVAL_SELF, message).with_subjects(objects)
        });
    Ok(Some(Journaled {
        state,
        kind,
        policy,
        violations: found,
        diff,
        cascaded: closures(&cascaded)?,
        reason,
    }))
}

/// Deletes the keys past their 24 h (REQ-004-10), for the background task (REQ-006-38).
/// Returns how many.
///
/// # Errors
///
/// Database errors.
pub async fn purge_expired(pool: &PgPool) -> Result<u64, StoreError> {
    sqlx::query("DELETE FROM idempotency_key WHERE expires_at <= now()")
        .execute(pool)
        .await
        .map(|done| done.rows_affected())
        .map_err(store_err)
}
