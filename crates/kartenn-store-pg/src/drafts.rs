//! Drafts kept by the server (spec 024, REQ-024-1 to 7a; tasks 024-5, 6): the private
//! operations of a person, which apply nothing until submitted. Each write takes the
//! version `If-Match` named (006/H19) and queues the event `org.kartenn.draft.changed`
//! for the stream of its author (022, REQ-024-6); the creation, the deletion and the
//! actions of an administrator are audited in the same transaction.

use kartenn_core::audit::{AuditEvent, DRAFT_DELETED};
use kartenn_service::audit::{Instance, Outcome, RequestContext};
use kartenn_service::port::{DraftOrigin, StoreError};
use serde_json::Value;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgConnection, PgPool};

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Drafts an identity keeps at most (REQ-024-7, Q-024-3).
pub const MAX_DRAFTS: i64 = 50;
/// Operations a draft holds at most (REQ-024-7, Q-024-3).
pub const MAX_OPERATIONS: usize = 10_000;
/// Days of inactivity after which a draft is deleted (REQ-024-7, Q-024-2).
pub const RETENTION_DAYS: i32 = 90;
/// Days before its deletion from which a draft announces it (`expires_at`, REQ-024-7).
pub const NOTICE_DAYS: i32 = 7;
/// Drafts deleted by one pass of the retention, at most.
const RETENTION_BATCH: i64 = 1_000;

/// The CloudEvents type of the event of a draft, which only its author receives
/// (REQ-024-6): `{draft, author, version}`, or `{draft, author, deleted: true}`.
pub const DRAFT_CHANGED: &str = "org.kartenn.draft.changed";

/// A draft as listed: without its operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftSummary {
    /// Identifier.
    pub id: Uuid,
    /// Its author.
    pub author: Uuid,
    /// Title.
    pub title: String,
    /// The number of its operations.
    pub operations: i64,
    /// Version, its entity tag.
    pub version: i64,
    /// Creation.
    pub created_at: OffsetDateTime,
    /// Last modification.
    pub updated_at: OffsetDateTime,
    /// Its deletion by the retention, from [`NOTICE_DAYS`] before it.
    pub expires_at: Option<OffsetDateTime>,
}

/// A draft, whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// What the list shows.
    pub summary: DraftSummary,
    /// `{summary, description}`, both optional.
    pub reason: Value,
    /// Its operations, `Operation`s of the contract.
    pub operations: Value,
}

/// Why a write of a draft is refused.
#[derive(Debug, thiserror::Error)]
pub enum DraftError {
    /// Absent, or of another author.
    #[error("no such draft")]
    NotFound,
    /// Its version is no longer the one named.
    #[error("the draft has changed")]
    Stale,
    /// Over a limit (REQ-024-7).
    #[error("{0}")]
    Limit(String),
    /// Database failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<sqlx::Error> for DraftError {
    fn from(e: sqlx::Error) -> Self {
        Self::Store(store_err(e))
    }
}

/// The columns of a summary.
const SUMMARY: &str = "id, author_id, title, jsonb_array_length(operations)::bigint AS count, version,
    created_at, updated_at,
    CASE WHEN updated_at <= now() - make_interval(days => 83) THEN updated_at + make_interval(days => 90) END
        AS expires_at";

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    author_id: Uuid,
    title: String,
    count: i64,
    version: i64,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    expires_at: Option<OffsetDateTime>,
}

#[derive(sqlx::FromRow)]
struct FullRow {
    #[sqlx(flatten)]
    row: Row,
    reason: Value,
    operations: Value,
}

impl From<Row> for DraftSummary {
    fn from(r: Row) -> Self {
        Self {
            id: r.id,
            author: r.author_id,
            title: r.title,
            operations: r.count,
            version: r.version,
            created_at: r.created_at,
            updated_at: r.updated_at,
            expires_at: r.expires_at,
        }
    }
}

impl From<FullRow> for Draft {
    fn from(r: FullRow) -> Self {
        Self {
            summary: r.row.into(),
            reason: r.reason,
            operations: r.operations,
        }
    }
}

fn full() -> String {
    // The notice of [`NOTICE_DAYS`] before [`RETENTION_DAYS`], written out in `SUMMARY`.
    const _: () = assert!(RETENTION_DAYS - NOTICE_DAYS == 83 && RETENTION_DAYS == 90);
    format!("SELECT {SUMMARY}, reason, operations FROM draft")
}

/// The drafts of `author`, the most recently modified first.
///
/// # Errors
///
/// Database errors.
pub async fn list(pool: &PgPool, author: Uuid) -> Result<Vec<DraftSummary>, StoreError> {
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {SUMMARY} FROM draft WHERE author_id = $1 ORDER BY updated_at DESC, id"
    )))
    .bind(author)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Draft `id`, whoever its author.
///
/// # Errors
///
/// Database errors.
pub async fn find(pool: &PgPool, id: Uuid) -> Result<Option<Draft>, StoreError> {
    let row: Option<FullRow> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!("{} WHERE id = $1", full())))
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(store_err)?;
    Ok(row.map(Into::into))
}

/// Queues the event of draft `id` of `author` at `version`, or deleted when `None`, in
/// the transaction of `conn` (REQ-024-6): not tied to a changeset, its author alone
/// receives it.
async fn event(
    conn: &mut PgConnection,
    id: Uuid,
    author: Uuid,
    version: Option<i64>,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO outbox (changeset_id, payload) VALUES (NULL, jsonb_build_object(
            'specversion', '1.0', 'id', $1::text, 'source', $2::text, 'type', $3::text,
            'time', now(), 'datacontenttype', 'application/json',
            'data', jsonb_strip_nulls(jsonb_build_object(
                'draft', $4, 'author', $5, 'version', $6,
                'deleted', CASE WHEN $6::bigint IS NULL THEN true END))))",
    )
    .bind(Uuid::now_v7())
    .bind(crate::write::EVENT_SOURCE)
    .bind(DRAFT_CHANGED)
    .bind(id)
    .bind(author)
    .bind(version)
    .execute(&mut *conn)
    .await
    .map_err(store_err)?;
    crate::outbox::notify(conn).await
}

/// Serialises the writes that count the drafts of `author`, in the transaction of `conn`.
async fn lock_author(conn: &mut PgConnection, author: Uuid) -> Result<(), StoreError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('draft/' || $1::text, 0))")
        .bind(author)
        .execute(&mut *conn)
        .await
        .map_err(store_err)?;
    Ok(())
}

/// Refuses a 51st draft of `author` (REQ-024-7).
async fn room(conn: &mut PgConnection, author: Uuid) -> Result<(), DraftError> {
    lock_author(conn, author).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM draft WHERE author_id = $1")
        .bind(author)
        .fetch_one(&mut *conn)
        .await?;
    if count >= MAX_DRAFTS {
        return Err(DraftError::Limit(format!(
            "an identity keeps at most {MAX_DRAFTS} drafts"
        )));
    }
    Ok(())
}

/// Refuses more than [`MAX_OPERATIONS`] operations.
fn bounded(count: usize) -> Result<(), DraftError> {
    if count > MAX_OPERATIONS {
        return Err(DraftError::Limit(format!(
            "a draft holds at most {MAX_OPERATIONS} operations, not {count}"
        )));
    }
    Ok(())
}

/// What a new draft holds.
#[derive(Debug, Clone)]
pub struct NewDraft {
    /// Its identifier.
    pub id: Uuid,
    /// Its author.
    pub author: Uuid,
    /// Title.
    pub title: String,
    /// `{summary, description}`.
    pub reason: Value,
    /// Its operations, checked against the contract.
    pub operations: Vec<Value>,
}

/// Creates `draft`, its creation audited by `audit` (REQ-024-6).
///
/// # Errors
///
/// [`DraftError::Limit`] beyond 50 drafts or 10 000 operations; database errors.
pub async fn create(
    pool: &PgPool,
    draft: NewDraft,
    audit: &AuditEvent,
    instance: &Instance,
) -> Result<Draft, DraftError> {
    bounded(draft.operations.len())?;
    let mut tx = pool.begin().await?;
    room(&mut tx, draft.author).await?;
    sqlx::query(
        "INSERT INTO draft (id, author_id, title, reason, operations) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(draft.id)
    .bind(draft.author)
    .bind(&draft.title)
    .bind(&draft.reason)
    .bind(Value::Array(draft.operations))
    .execute(&mut *tx)
    .await?;
    crate::audit_journal::insert(&mut *tx, audit, instance).await?;
    event(&mut tx, draft.id, draft.author, Some(1)).await?;
    let found: FullRow = sqlx::query_as(sqlx::AssertSqlSafe(format!("{} WHERE id = $1", full())))
        .bind(draft.id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(found.into())
}

/// A modification of a draft (`PATCH`, or operations added).
#[derive(Debug, Clone, Default)]
pub struct Change {
    /// A new title.
    pub title: Option<String>,
    /// A new reason.
    pub reason: Option<Value>,
    /// Operations that replace its own.
    pub operations: Option<Vec<Value>>,
    /// Operations added after its own.
    pub append: Vec<Value>,
}

/// Locks draft `id` of `author` at `version`, and returns the number of its operations.
async fn locked(
    conn: &mut PgConnection,
    id: Uuid,
    author: Uuid,
    version: i64,
) -> Result<i64, DraftError> {
    let found: Option<(Uuid, i64, i64)> = sqlx::query_as(
        "SELECT author_id, version, jsonb_array_length(operations)::bigint FROM draft
         WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    match found {
        Some((owner, _, _)) if owner != author => Err(DraftError::NotFound),
        None => Err(DraftError::NotFound),
        Some((_, current, _)) if current != version => Err(DraftError::Stale),
        Some((_, _, count)) => Ok(count),
    }
}

/// Applies `change` to draft `id` of `author` at `version` (REQ-024-3): not audited
/// (REQ-024-6).
///
/// # Errors
///
/// [`DraftError::NotFound`] for a draft absent or of another, [`DraftError::Stale`] when
/// its version changed, [`DraftError::Limit`] beyond 10 000 operations; database errors.
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    author: Uuid,
    version: i64,
    change: Change,
) -> Result<Draft, DraftError> {
    let mut tx = pool.begin().await?;
    let count = locked(&mut tx, id, author, version).await?;
    let kept = change
        .operations
        .as_ref()
        .map_or(usize::try_from(count).unwrap_or(usize::MAX), Vec::len);
    bounded(kept.saturating_add(change.append.len()))?;
    let found: FullRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH changed AS (
             UPDATE draft SET title = coalesce($2, title), reason = coalesce($3, reason),
                    operations = coalesce($4, operations) || $5, version = version + 1,
                    updated_at = now()
             WHERE id = $1 RETURNING *)
         {} ",
        full().replace("FROM draft", "FROM changed")
    )))
    .bind(id)
    .bind(change.title)
    .bind(change.reason)
    .bind(change.operations.map(Value::Array))
    .bind(Value::Array(change.append))
    .fetch_one(&mut *tx)
    .await?;
    event(&mut tx, id, author, Some(found.row.version)).await?;
    tx.commit().await?;
    Ok(found.into())
}

/// Deletes draft `id` of `author` at `version`, the deletion audited by `audit`
/// (REQ-024-6; an administrator's, REQ-024-7a).
///
/// # Errors
///
/// As [`update`].
pub async fn delete(
    pool: &PgPool,
    id: Uuid,
    author: Uuid,
    version: i64,
    audit: &AuditEvent,
    instance: &Instance,
) -> Result<(), DraftError> {
    let mut tx = pool.begin().await?;
    locked(&mut tx, id, author, version).await?;
    sqlx::query("DELETE FROM draft WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    crate::audit_journal::insert(&mut *tx, audit, instance).await?;
    event(&mut tx, id, author, None).await?;
    tx.commit().await?;
    Ok(())
}

/// Gives draft `id` of `author` at `version` to `taker` (REQ-024-7a): it leaves the
/// drafts of its former author, who receives its deletion; audited by `audit`.
///
/// # Errors
///
/// As [`update`]; [`DraftError::Limit`] when `taker` has 50 drafts already.
pub async fn take_over(
    pool: &PgPool,
    id: Uuid,
    (author, version): (Uuid, i64),
    taker: Uuid,
    audit: &AuditEvent,
    instance: &Instance,
) -> Result<Draft, DraftError> {
    let mut tx = pool.begin().await?;
    room(&mut tx, taker).await?;
    locked(&mut tx, id, author, version).await?;
    let found: FullRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH changed AS (
             UPDATE draft SET author_id = $2, version = version + 1, updated_at = now()
             WHERE id = $1 RETURNING *)
         {}",
        full().replace("FROM draft", "FROM changed")
    )))
    .bind(id)
    .bind(taker)
    .fetch_one(&mut *tx)
    .await?;
    crate::audit_journal::insert(&mut *tx, audit, instance).await?;
    event(&mut tx, id, author, None).await?;
    event(&mut tx, id, taker, Some(found.row.version)).await?;
    tx.commit().await?;
    Ok(found.into())
}

/// Deletes the draft a changeset was just created from, in the transaction of its journal
/// (REQ-024-5), when still at the version submitted: one modified since stays.
pub(crate) async fn submitted(
    conn: &mut PgConnection,
    draft: DraftOrigin,
) -> Result<(), StoreError> {
    let author: Option<Uuid> =
        sqlx::query_scalar("DELETE FROM draft WHERE id = $1 AND version = $2 RETURNING author_id")
            .bind(draft.id)
            .bind(draft.version)
            .fetch_optional(&mut *conn)
            .await
            .map_err(store_err)?;
    match author {
        Some(author) => event(conn, draft.id, author, None).await,
        None => Ok(()),
    }
}

/// The changeset that idempotency key `key` of `author` created from draft `id`, within
/// its 24 hours (REQ-024-5): a submission retried once the draft is gone answers it again.
///
/// # Errors
///
/// Database errors.
pub async fn submission_of(
    pool: &PgPool,
    author: Uuid,
    key: &str,
    id: Uuid,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT k.changeset_id FROM idempotency_key k JOIN changeset c ON c.id = k.changeset_id
         WHERE k.identity_id = $1 AND k.key = $2 AND k.expires_at > now() AND c.draft_id = $3",
    )
    .bind(author)
    .bind(key)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(store_err)
}

/// Deletes the drafts inactive for [`RETENTION_DAYS`] days (REQ-024-7), by batches, each
/// deletion audited for kartenn itself with the author, and told to the author. Returns
/// how many.
///
/// # Errors
///
/// Database errors.
pub async fn expire(pool: &PgPool, instance: &Instance) -> Result<u64, StoreError> {
    let mut deleted = 0;
    loop {
        let mut tx = pool.begin().await.map_err(store_err)?;
        let gone: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "DELETE FROM draft WHERE id IN (
                 SELECT id FROM draft WHERE updated_at < now() - make_interval(days => $1)
                 ORDER BY updated_at LIMIT $2 FOR UPDATE SKIP LOCKED)
             RETURNING id, author_id",
        )
        .bind(RETENTION_DAYS)
        .bind(RETENTION_BATCH)
        .fetch_all(&mut *tx)
        .await
        .map_err(store_err)?;
        for (id, author) in &gone {
            let audit = RequestContext::system()
                .event(&DRAFT_DELETED, Outcome::Success)
                .map_err(store_err)?
                .target("draft", [*id])
                .detail("cause", "expired")
                .map_err(store_err)?
                .detail("author", author.to_string())
                .map_err(store_err)?;
            crate::audit_journal::insert(&mut *tx, &audit, instance).await?;
            event(&mut tx, *id, *author, None).await?;
        }
        tx.commit().await.map_err(store_err)?;
        deleted += u64::try_from(gone.len()).unwrap_or(u64::MAX);
        if i64::try_from(gone.len()).unwrap_or(i64::MAX) < RETENTION_BATCH {
            return Ok(deleted);
        }
    }
}
