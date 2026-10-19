//! Reading the journal of the changesets (spec 006, REQ-006-16; task 006-17): one
//! changeset with its operations and result, a page of the list filtered by state and
//! author, newest first, and the summaries of some changesets.

use std::collections::BTreeMap;

use kartenn_core::rules::Violation;
use kartenn_service::port::StoreError;
use kartenn_service::{ObjectKind, Operation};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::idempotency::{closures, violations};

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// The states of a changeset, as journaled.
pub const STATES: [&str; 7] = [
    "applied",
    "refused",
    "pending",
    "rejected",
    "withdrawn",
    "expired",
    "failed_revalidation",
];

/// A changeset as listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangesetSummary {
    /// Identifier.
    pub id: Uuid,
    /// State.
    pub state: String,
    /// `direct`, `approval` or `proposal`.
    pub kind: String,
    /// Author.
    pub author: Uuid,
    /// Motive.
    pub motive: String,
    /// When it was submitted (system time).
    pub submitted_at: OffsetDateTime,
}

/// A changeset as journaled.
#[derive(Debug, Clone)]
pub struct ChangesetRead {
    /// What the list shows.
    pub summary: ChangesetSummary,
    /// Who validated it.
    pub validator: Option<Uuid>,
    /// Start of its period.
    pub valid_from: OffsetDateTime,
    /// End of its period; `None`: infinity.
    pub valid_to: Option<OffsetDateTime>,
    /// When it was applied.
    pub applied_at: Option<OffsetDateTime>,
    /// When it expires, pending.
    pub expires_at: Option<OffsetDateTime>,
    /// The approval policy that held it.
    pub policy: Option<Uuid>,
    /// The motive of its rejection.
    pub rejection: Option<String>,
    /// Its operations, in order.
    pub operations: Vec<Operation>,
    /// Errors, then warnings.
    pub violations: Vec<Violation>,
    /// The differential, counted.
    pub diff: serde_json::Value,
    /// Closures added by a cascade.
    pub cascaded: Vec<(ObjectKind, Uuid)>,
}

/// The filter of the list.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangesetFilter<'a> {
    /// One of [`STATES`].
    pub state: Option<&'a str>,
    /// Author.
    pub author: Option<Uuid>,
}

type SummaryRow = (Uuid, String, String, Uuid, String, OffsetDateTime);

fn summary((id, state, kind, author, motive, submitted_at): SummaryRow) -> ChangesetSummary {
    ChangesetSummary {
        id,
        state,
        kind,
        author,
        motive,
        submitted_at,
    }
}

const SUMMARY: &str = "id, state, kind, author_id, motive, submitted_at";

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    state: String,
    kind: String,
    author_id: Uuid,
    motive: String,
    submitted_at: OffsetDateTime,
    validator_id: Option<Uuid>,
    valid_from: OffsetDateTime,
    valid_to: Option<OffsetDateTime>,
    applied_at: Option<OffsetDateTime>,
    expires_at: Option<OffsetDateTime>,
    policy_id: Option<Uuid>,
    rejection: Option<String>,
    violations: serde_json::Value,
    anomalies: serde_json::Value,
    diff_summary: serde_json::Value,
    cascaded: serde_json::Value,
}

/// Changeset `id`, if journaled.
///
/// # Errors
///
/// Database or decoding errors.
pub async fn find(pool: &PgPool, id: Uuid) -> Result<Option<ChangesetRead>, StoreError> {
    let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {SUMMARY}, validator_id, lower(default_period) AS valid_from,
                upper(default_period) AS valid_to, applied_at, expires_at, policy_id,
                rejection, violations, anomalies, diff_summary, cascaded
         FROM changeset WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(store_err)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let mut found = violations(&row.violations)?;
    found.extend(violations(&row.anomalies)?);
    Ok(Some(ChangesetRead {
        summary: summary((
            row.id,
            row.state,
            row.kind,
            row.author_id,
            row.motive,
            row.submitted_at,
        )),
        validator: row.validator_id,
        valid_from: row.valid_from,
        valid_to: row.valid_to,
        applied_at: row.applied_at,
        expires_at: row.expires_at,
        policy: row.policy_id,
        rejection: row.rejection,
        operations: crate::write::operations_of(pool, id).await?,
        violations: found,
        diff: row.diff_summary,
        cascaded: closures(&row.cascaded)?,
    }))
}

/// At most `limit` changesets matching `filter`, newest first (system time, then
/// identifier), after `after`, the key of the last one of the previous page.
///
/// # Errors
///
/// Database errors.
pub async fn page(
    pool: &PgPool,
    filter: ChangesetFilter<'_>,
    after: Option<(OffsetDateTime, Uuid)>,
    limit: i64,
) -> Result<Vec<ChangesetSummary>, StoreError> {
    let rows: Vec<SummaryRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {SUMMARY} FROM changeset
         WHERE ($1::text IS NULL OR state = $1) AND ($2::uuid IS NULL OR author_id = $2)
           AND ($3::timestamptz IS NULL OR (submitted_at, id) < ($3, $4))
         ORDER BY submitted_at DESC, id DESC LIMIT $5"
    )))
    .bind(filter.state)
    .bind(filter.author)
    .bind(after.map(|(at, _)| at))
    .bind(after.map(|(_, id)| id))
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    Ok(rows.into_iter().map(summary).collect())
}

/// The objects of the differential detail of changeset `id` for each of `keys`.
///
/// # Errors
///
/// Database errors.
pub async fn diff_objects(
    pool: &PgPool,
    id: Uuid,
    keys: &[String],
) -> Result<BTreeMap<String, Vec<Uuid>>, StoreError> {
    let rows: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT DISTINCT property_key, object_id FROM changeset_diff
         WHERE changeset_id = $1 AND property_key = ANY($2)",
    )
    .bind(id)
    .bind(keys)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    let mut out: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    for (key, object) in rows {
        out.entry(key).or_default().push(object);
    }
    Ok(out)
}

/// The summaries of the changesets `ids` that are journaled, in no given order.
///
/// # Errors
///
/// Database errors.
pub async fn summaries(pool: &PgPool, ids: &[Uuid]) -> Result<Vec<ChangesetSummary>, StoreError> {
    let rows: Vec<SummaryRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {SUMMARY} FROM changeset WHERE id = ANY($1)"
    )))
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    Ok(rows.into_iter().map(summary).collect())
}
