//! Imports in batches (spec 004, REQ-004-28 to 30; tasks 004-18, 19), deposited by
//! chunks, sorted and run as background tasks (spec 006, REQ-006-17 to 19; tasks 006-21,
//! 27).

use std::collections::HashMap;

use kartenn_service::authz::Subject;
use kartenn_service::port::{ImportBatch, ImportItem, ImportProgress, ImportRecord, StoreError};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgConnection, PgPool};

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Records an import and its batches.
pub(crate) async fn create(pool: &PgPool, record: &ImportRecord) -> Result<(), StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    sqlx::query(
        "INSERT INTO import (id, author_id, author_subject, source, state, report) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(record.id)
    .bind(record.author)
    .bind(
        record
            .author_subject
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(store_err)?,
    )
    .bind(&record.source)
    .bind(&record.state)
    .bind(&record.report)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?;
    for batch in &record.batches {
        sqlx::query(
            "INSERT INTO import_batch (import_id, idx, operations, state) VALUES ($1, $2, $3, $4)",
        )
        .bind(record.id)
        .bind(batch.index)
        .bind(serde_json::to_value(&batch.items).map_err(store_err)?)
        .bind(&batch.state)
        .execute(&mut *tx)
        .await
        .map_err(store_err)?;
    }
    tx.commit().await.map_err(store_err)
}

/// An import with its batches.
pub(crate) async fn find(pool: &PgPool, id: Uuid) -> Result<Option<ImportRecord>, StoreError> {
    let row: Option<(
        Uuid,
        Option<serde_json::Value>,
        String,
        String,
        serde_json::Value,
    )> = sqlx::query_as(
        "SELECT author_id, author_subject, source, state, report FROM import WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(store_err)?;
    let Some((author, author_subject, source, state, report)) = row else {
        return Ok(None);
    };
    let author_subject = author_subject
        .map(serde_json::from_value)
        .transpose()
        .map_err(store_err)?;
    let rows: Vec<(i32, serde_json::Value, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT idx, operations, state, changeset_id FROM import_batch WHERE import_id = $1 ORDER BY idx",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    let batches = rows
        .into_iter()
        .map(|(index, items, state, changeset)| {
            Ok(ImportBatch {
                index,
                items: serde_json::from_value::<Vec<ImportItem>>(items).map_err(store_err)?,
                state,
                changeset,
            })
        })
        .collect::<Result<_, StoreError>>()?;
    Ok(Some(ImportRecord {
        id,
        author,
        author_subject,
        source,
        state,
        report,
        batches,
    }))
}

/// Records the outcome of a batch.
pub(crate) async fn set_batch(
    pool: &PgPool,
    import: Uuid,
    index: i32,
    state: &str,
    changeset: Option<Uuid>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE import_batch SET state = $3, changeset_id = coalesce($4, changeset_id)
         WHERE import_id = $1 AND idx = $2",
    )
    .bind(import)
    .bind(index)
    .bind(state)
    .bind(changeset)
    .execute(pool)
    .await
    .map_err(store_err)?;
    Ok(())
}

/// Records the state and report of an import whose state is one of `from`.
pub(crate) async fn set_import(
    pool: &PgPool,
    import: Uuid,
    from: &[&str],
    state: &str,
    report: &serde_json::Value,
) -> Result<bool, StoreError> {
    sqlx::query("UPDATE import SET state = $3, report = $4 WHERE id = $1 AND state = ANY($2)")
        .bind(import)
        .bind(from)
        .bind(state)
        .bind(report)
        .execute(pool)
        .await
        .map(|done| done.rows_affected() == 1)
        .map_err(store_err)
}

/// Where an import stands, counted from its batches.
pub(crate) async fn progress(
    pool: &PgPool,
    import: Uuid,
) -> Result<Option<ImportProgress>, StoreError> {
    let row: Option<(String, serde_json::Value, i64, i64)> = sqlx::query_as(
        "SELECT i.state, i.report,
                (SELECT count(*) FROM import_batch b WHERE b.import_id = i.id AND b.state = 'applied'),
                (SELECT count(*) FROM import_batch b WHERE b.import_id = i.id)
           FROM import i WHERE i.id = $1",
    )
    .bind(import)
    .fetch_optional(pool)
    .await
    .map_err(store_err)?;
    Ok(row.map(|(state, report, applied, batches)| ImportProgress {
        state,
        applied: usize::try_from(applied).unwrap_or_default(),
        batches: usize::try_from(batches).unwrap_or_default(),
        report,
    }))
}

/// The name of the background task that runs an import.
#[must_use]
pub fn job_name(import: Uuid) -> String {
    format!("import:{import}")
}

/// The import a background task runs, if it is one.
#[must_use]
pub fn job_import(name: &str) -> Option<Uuid> {
    name.strip_prefix("import:")?.parse().ok()
}

/// Makes the task of `import` due now, creating it if need be.
async fn schedule(conn: &mut PgConnection, import: Uuid) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO job (name, due_at, period) VALUES ($1, now(), interval '10 seconds')
         ON CONFLICT (name) DO UPDATE SET due_at = least(job.due_at, now())",
    )
    .bind(job_name(import))
    .execute(conn)
    .await
    .map(|_| ())
    .map_err(store_err)
}

/// Removes the task of `import` (its import is over or paused).
///
/// # Errors
///
/// Database errors.
pub async fn unschedule(pool: &PgPool, import: Uuid) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM job WHERE name = $1")
        .bind(job_name(import))
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(store_err)
}

/// An import without its elements (REQ-006-19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportHeader {
    /// Its author: the only caller who sees it.
    pub author: Uuid,
    /// The source.
    pub source: String,
    /// Its state.
    pub state: String,
    /// Chunks deposited.
    pub chunks: i64,
    /// Elements deposited.
    pub elements: i64,
    /// Why its last sort failed (H18), until the next start.
    pub error: Option<serde_json::Value>,
}

/// An import without its elements, with the count of its chunks.
///
/// # Errors
///
/// Database errors.
pub async fn header(pool: &PgPool, import: Uuid) -> Result<Option<ImportHeader>, StoreError> {
    let row: Option<(Uuid, String, String, i64, i64, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT i.author_id, i.source, i.state, count(c.n), coalesce(sum(c.size), 0)::bigint,
                i.error
           FROM import i LEFT JOIN import_chunk c ON c.import_id = i.id
          WHERE i.id = $1 GROUP BY i.id",
    )
    .bind(import)
    .fetch_optional(pool)
    .await
    .map_err(store_err)?;
    Ok(row.map(
        |(author, source, state, chunks, elements, error)| ImportHeader {
            author,
            source,
            state,
            chunks,
            elements,
            error,
        },
    ))
}

/// The imports of `author`, the most recent first, after the key `after` (opening date,
/// identifier), at most `limit` of them (REQ-007-40).
///
/// # Errors
///
/// Database errors.
pub async fn of_author(
    pool: &PgPool,
    author: Uuid,
    after: Option<(OffsetDateTime, Uuid)>,
    limit: i64,
) -> Result<Vec<(OffsetDateTime, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT created_at, id FROM import
          WHERE author_id = $1
            AND ($2::timestamptz IS NULL OR (created_at, id) < ($2::timestamptz, $3::uuid))
          ORDER BY created_at DESC, id DESC
          LIMIT $4",
    )
    .bind(author)
    .bind(after.map(|(at, _)| at))
    .bind(after.map(|(_, id)| id))
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(store_err)
}

/// What became of a deposited chunk (REQ-006-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deposit {
    /// New under its number.
    Created,
    /// Resent identical: nothing changes.
    Same,
    /// Another content under a number already received.
    Different,
    /// The import no longer takes chunks (or is unknown).
    Closed,
}

/// Deposits chunk `n` of `import`, of digest `digest`, while the import is `uploading`;
/// the start waits for the deposits in progress and the other way around.
///
/// # Errors
///
/// Database errors.
pub async fn deposit(
    pool: &PgPool,
    import: Uuid,
    n: i32,
    digest: &[u8],
    elements: &[ImportItem],
) -> Result<Deposit, StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    let state: Option<String> =
        sqlx::query_scalar("SELECT state FROM import WHERE id = $1 FOR SHARE")
            .bind(import)
            .fetch_optional(&mut *tx)
            .await
            .map_err(store_err)?;
    if state.as_deref() != Some("uploading") {
        return Ok(Deposit::Closed);
    }
    let created: Option<i32> = sqlx::query_scalar(
        "INSERT INTO import_chunk (import_id, n, digest, size, elements)
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING RETURNING n",
    )
    .bind(import)
    .bind(n)
    .bind(digest)
    .bind(i32::try_from(elements.len()).unwrap_or(i32::MAX))
    .bind(sqlx::types::Json(elements))
    .fetch_optional(&mut *tx)
    .await
    .map_err(store_err)?;
    let deposit = if created.is_some() {
        Deposit::Created
    } else {
        let known: Vec<u8> =
            sqlx::query_scalar("SELECT digest FROM import_chunk WHERE import_id = $1 AND n = $2")
                .bind(import)
                .bind(n)
                .fetch_one(&mut *tx)
                .await
                .map_err(store_err)?;
        if known == digest {
            Deposit::Same
        } else {
            Deposit::Different
        }
    };
    tx.commit().await.map_err(store_err)?;
    Ok(deposit)
}

/// Starts `import` (REQ-006-17, H18): an `uploading` import becomes `sorting`, loses the
/// error of its previous sort and its task is due; the sort is left to the task. False if
/// the import is not `uploading` (or is unknown). The start waits for the deposits in
/// progress. The import keeps `author`, the author's issuer and groups when starting it,
/// for its batches (REQ-005-22).
///
/// # Errors
///
/// Database errors.
pub async fn start(pool: &PgPool, import: Uuid, author: &Subject) -> Result<bool, StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    let started = sqlx::query(
        "UPDATE import SET state = 'sorting', error = NULL, author_subject = $2 \
         WHERE id = $1 AND state = 'uploading'",
    )
    .bind(import)
    .bind(serde_json::to_value(author).map_err(store_err)?)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?
    .rows_affected()
        == 1;
    if started {
        schedule(&mut tx, import).await?;
    }
    tx.commit().await.map_err(store_err)?;
    Ok(started)
}

/// The elements of a `sorting` import, its chunks read by increasing number; `None` if the
/// import is not `sorting`. Its chunks no longer change: they are deposited only while it
/// is `uploading`.
///
/// # Errors
///
/// Database errors.
pub async fn sorting(pool: &PgPool, import: Uuid) -> Result<Option<Vec<ImportItem>>, StoreError> {
    let state: Option<String> = sqlx::query_scalar("SELECT state FROM import WHERE id = $1")
        .bind(import)
        .fetch_optional(pool)
        .await
        .map_err(store_err)?;
    if state.as_deref() != Some("sorting") {
        return Ok(None);
    }
    let chunks: Vec<sqlx::types::Json<Vec<ImportItem>>> =
        sqlx::query_scalar("SELECT elements FROM import_chunk WHERE import_id = $1 ORDER BY n")
            .bind(import)
            .fetch_all(pool)
            .await
            .map_err(store_err)?;
    Ok(Some(chunks.into_iter().flat_map(|chunk| chunk.0).collect()))
}

/// Records the batches of a sorted import and makes it `ready`, in one transaction; false
/// (and nothing recorded) if it is no longer `sorting`, cancelled meanwhile.
///
/// # Errors
///
/// Database errors.
pub async fn sorted(
    pool: &PgPool,
    import: Uuid,
    batches: &[Vec<ImportItem>],
) -> Result<bool, StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    for (index, items) in batches.iter().enumerate() {
        sqlx::query(
            "INSERT INTO import_batch (import_id, idx, operations, state)
             VALUES ($1, $2, $3, 'queued')",
        )
        .bind(import)
        .bind(i32::try_from(index).unwrap_or(i32::MAX))
        .bind(sqlx::types::Json(items))
        .execute(&mut *tx)
        .await
        .map_err(store_err)?;
    }
    // Last, so that a cancel waits for the row only the time of the commit.
    let ready =
        sqlx::query("UPDATE import SET state = 'ready' WHERE id = $1 AND state = 'sorting'")
            .bind(import)
            .execute(&mut *tx)
            .await
            .map_err(store_err)?
            .rows_affected()
            == 1;
    if ready {
        tx.commit().await.map_err(store_err)?;
    }
    Ok(ready)
}

/// Brings a `sorting` import whose elements cannot be sorted back to `uploading` with
/// `error`, and removes its task; false if it is no longer `sorting`.
///
/// # Errors
///
/// Database errors.
pub async fn unsorted(
    pool: &PgPool,
    import: Uuid,
    error: &serde_json::Value,
) -> Result<bool, StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    let back = sqlx::query(
        "UPDATE import SET state = 'uploading', error = $2 WHERE id = $1 AND state = 'sorting'",
    )
    .bind(import)
    .bind(error)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?
    .rows_affected()
        == 1;
    if back {
        sqlx::query("DELETE FROM job WHERE name = $1")
            .bind(job_name(import))
            .execute(&mut *tx)
            .await
            .map_err(store_err)?;
    }
    tx.commit().await.map_err(store_err)?;
    Ok(back)
}

/// Moves `import` from one of `from` to `to`; with `run`, the author resuming it, makes
/// its task due and keeps the author's issuer and groups for its batches (REQ-005-22).
/// False if the import is in none of `from`.
///
/// # Errors
///
/// Database errors.
pub async fn transition(
    pool: &PgPool,
    import: Uuid,
    from: &[&str],
    to: &str,
    run: Option<&Subject>,
) -> Result<bool, StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    let author = run
        .map(serde_json::to_value)
        .transpose()
        .map_err(store_err)?;
    let moved = sqlx::query(
        "UPDATE import SET state = $3, author_subject = coalesce($4, author_subject) \
         WHERE id = $1 AND state = ANY($2)",
    )
    .bind(import)
    .bind(from)
    .bind(to)
    .bind(author)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?
    .rows_affected()
        == 1;
    if moved && run.is_some() {
        schedule(&mut tx, import).await?;
    }
    tx.commit().await.map_err(store_err)?;
    Ok(moved)
}

/// The objects known to `source` by some of `external_ids`, by identifier.
pub(crate) async fn externals(
    pool: &PgPool,
    source: &str,
    external_ids: &[String],
) -> Result<HashMap<String, Uuid>, StoreError> {
    let rows: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT external_id, object_id FROM external_id
         WHERE source = $1 AND external_id = ANY($2)",
    )
    .bind(source)
    .bind(external_ids)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    Ok(rows.into_iter().collect())
}
