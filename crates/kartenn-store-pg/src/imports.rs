//! Imports in batches (spec 004, REQ-004-28 to 30; tasks 004-18, 19).

use kartenn_service::port::{ImportBatch, ImportItem, ImportRecord, StoreError};
use sqlx::PgPool;
use sqlx::types::Uuid;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Records an import and its batches.
pub(crate) async fn create(pool: &PgPool, record: &ImportRecord) -> Result<(), StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    sqlx::query(
        "INSERT INTO import (id, author_id, source, state, report) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(record.id)
    .bind(record.author)
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
    let row: Option<(Uuid, String, String, serde_json::Value)> =
        sqlx::query_as("SELECT author_id, source, state, report FROM import WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(store_err)?;
    let Some((author, source, state, report)) = row else {
        return Ok(None);
    };
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

/// Records the state and report of an import.
pub(crate) async fn set_import(
    pool: &PgPool,
    import: Uuid,
    state: &str,
    report: &serde_json::Value,
) -> Result<(), StoreError> {
    sqlx::query("UPDATE import SET state = $2, report = $3 WHERE id = $1")
        .bind(import)
        .bind(state)
        .bind(report)
        .execute(pool)
        .await
        .map_err(store_err)?;
    Ok(())
}

/// The object known to a source under an identifier.
pub(crate) async fn external(
    pool: &PgPool,
    source: &str,
    external_id: &str,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar("SELECT object_id FROM external_id WHERE source = $1 AND external_id = $2")
        .bind(source)
        .bind(external_id)
        .fetch_optional(pool)
        .await
        .map_err(store_err)
}
