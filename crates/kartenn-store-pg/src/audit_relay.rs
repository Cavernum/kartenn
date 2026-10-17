//! Exporting the audit journal to its outputs (spec 012, REQ-012-15; ADR 0019): each
//! output receives the committed events it has not received yet, in sequence order, at
//! least once; its deliveries are recorded after it took them, as for the outbox (ADR
//! 0018), so that a transaction committed late leaves no gap.
//!
//! The exported form is the stored row, the one the seals fingerprint: a receiver can
//! verify an event against its seal.

use std::time::Duration;

use kartenn_service::StoreError;
use kartenn_service::audit::{AuditSink, SinkError};
use sqlx::PgPool;
use sqlx::types::time::OffsetDateTime;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Why a relay pass failed.
#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// The database.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The output refused or could not take the batch; nothing is marked delivered.
    #[error("output {sink}: {error}")]
    Sink {
        /// Output.
        sink: String,
        /// Its error.
        error: SinkError,
    },
}

/// A row to export: sequence, criticality, recording time, row as JSON.
type Row = (i64, String, OffsetDateTime, serde_json::Value);

/// Delivers to `sink` at most `batch` events it has not received, and records their
/// delivery; the number delivered. One relay per output at a time (advisory lock); `0`
/// when another instance holds it.
///
/// # Errors
///
/// [`RelayError`]; after a [`RelayError::Sink`], the same events are offered again.
pub async fn relay_once<S: AuditSink>(
    pool: &PgPool,
    sink: &S,
    batch: i64,
) -> Result<usize, RelayError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    let locked: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtext('kartenn.audit.relay.' || $1))",
    )
    .bind(sink.name())
    .fetch_one(&mut *tx)
    .await
    .map_err(store_err)?;
    if !locked {
        return Ok(0);
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT e.seq, e.criticality, e.recorded_at, to_jsonb(e) FROM audit.event e
         WHERE NOT EXISTS (SELECT 1 FROM audit.delivery d WHERE d.sink = $1 AND d.event_seq = e.seq)
         ORDER BY e.seq LIMIT $2",
    )
    .bind(sink.name())
    .bind(batch)
    .fetch_all(&mut *tx)
    .await
    .map_err(store_err)?;
    if rows.is_empty() {
        return Ok(0);
    }
    let events: Vec<serde_json::Value> = rows.iter().map(|r| r.3.clone()).collect();
    sink.deliver(&events)
        .await
        .map_err(|error| RelayError::Sink {
            sink: sink.name().to_owned(),
            error,
        })?;
    let seqs: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let criticalities: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    let times: Vec<OffsetDateTime> = rows.iter().map(|r| r.2).collect();
    sqlx::query(
        "INSERT INTO audit.delivery (sink, event_seq, criticality, recorded_at)
         SELECT $1, * FROM unnest($2::bigint[], $3::text[], $4::timestamptz[])",
    )
    .bind(sink.name())
    .bind(&seqs)
    .bind(&criticalities)
    .bind(&times)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?;
    tx.commit().await.map_err(store_err)?;
    Ok(rows.len())
}

/// Age of the oldest event `sink` has not received (REQ-012-25); `None` when it is up to
/// date.
///
/// # Errors
///
/// Database errors.
pub async fn export_lag(pool: &PgPool, sink: &str) -> Result<Option<Duration>, StoreError> {
    let micros: Option<i64> = sqlx::query_scalar(
        "SELECT (extract(epoch FROM now() - min(e.recorded_at)) * 1000000)::bigint
         FROM audit.event e
         WHERE NOT EXISTS (SELECT 1 FROM audit.delivery d WHERE d.sink = $1 AND d.event_seq = e.seq)",
    )
    .bind(sink)
    .fetch_one(pool)
    .await
    .map_err(store_err)?;
    Ok(micros.map(|m| Duration::from_micros(u64::try_from(m).unwrap_or(0))))
}
