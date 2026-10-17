//! Sealing the audit events in chained batches (spec 012, REQ-012-13; ADR 0019).
//!
//! A batch takes the committed events that belong to no seal yet, in sequence order: an
//! event of a long transaction, committed after a later one was sealed, enters the next
//! batch, so the membership table leaves no gap. The fingerprint of an event is the
//! SHA-256 of the canonical JSON (RFC 8785) of its stored row.

use kartenn_core::audit::jcs::{batch_hash, event_hash};
use kartenn_core::audit::seal::{SealKey, hex};
use kartenn_core::audit::{AUDIT_SEALED, Outcome};
use kartenn_service::StoreError;
use kartenn_service::audit::{Instance, RequestContext};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgConnection, PgPool};

/// Most events in one batch.
pub const BATCH_MAX: i64 = 10_000;

/// Advisory lock of the sealing: one instance at a time.
const SEAL_LOCK: &str = "kartenn.audit.seal";

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// A seal written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    /// Sequence of the seal.
    pub seq: i64,
    /// Identifier.
    pub id: Uuid,
    /// Events sealed.
    pub count: usize,
    /// Fingerprint of the batch.
    pub hash: [u8; 32],
}

/// A row of an event: sequence, criticality, recording time and the whole row as JSON.
type EventRow = (i64, String, OffsetDateTime, serde_json::Value);

/// The canonical fingerprint of each stored event row.
pub(crate) fn hashes(rows: &[EventRow]) -> Result<Vec<[u8; 32]>, StoreError> {
    rows.iter()
        .map(|(_, _, _, json)| event_hash(json).map_err(store_err))
        .collect()
}

/// The fingerprint of the last seal, or 32 zero octets.
pub(crate) async fn last_hash(conn: &mut PgConnection) -> Result<[u8; 32], StoreError> {
    let last: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT hash FROM audit.seal ORDER BY seq DESC LIMIT 1")
            .fetch_optional(conn)
            .await
            .map_err(store_err)?;
    last.map_or(Ok([0; 32]), |h| {
        <[u8; 32]>::try_from(h.as_slice()).map_err(store_err)
    })
}

/// Seals the committed events that are not sealed yet, at most [`BATCH_MAX`]; records
/// `audit.seal.created`, which the next batch seals, unless the batch holds only such
/// events. `None` when there is nothing to seal or another instance is sealing.
///
/// # Errors
///
/// Database errors; an event that has no canonical form.
pub async fn seal_once(
    pool: &PgPool,
    key: Option<&SealKey>,
    instance: &Instance,
) -> Result<Option<Sealed>, StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtext($1))")
        .bind(SEAL_LOCK)
        .fetch_one(&mut *tx)
        .await
        .map_err(store_err)?;
    if !locked {
        return Ok(None);
    }
    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT e.seq, e.criticality, e.recorded_at, to_jsonb(e) FROM audit.event e
         WHERE NOT EXISTS (SELECT 1 FROM audit.sealed s WHERE s.event_seq = e.seq)
         ORDER BY e.seq LIMIT $1",
    )
    .bind(BATCH_MAX)
    .fetch_all(&mut *tx)
    .await
    .map_err(store_err)?;
    if rows.is_empty() {
        return Ok(None);
    }
    let previous = last_hash(&mut tx).await?;
    let hash = batch_hash(&previous, &hashes(&rows)?);
    let id = Uuid::now_v7();
    let signature = key.map(|k| k.sign(&hash).to_vec());
    let seq: i64 = sqlx::query_scalar(
        "INSERT INTO audit.seal (id, prev_hash, hash, count, signature, key_id)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING seq",
    )
    .bind(id)
    .bind(previous.as_slice())
    .bind(hash.as_slice())
    .bind(i32::try_from(rows.len()).map_err(store_err)?)
    .bind(signature)
    .bind(key.map(SealKey::id))
    .fetch_one(&mut *tx)
    .await
    .map_err(store_err)?;
    let seqs: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let criticalities: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    let times: Vec<OffsetDateTime> = rows.iter().map(|r| r.2).collect();
    sqlx::query(
        "INSERT INTO audit.sealed (seal_seq, event_seq, criticality, recorded_at)
         SELECT $1, * FROM unnest($2::bigint[], $3::text[], $4::timestamptz[])",
    )
    .bind(seq)
    .bind(&seqs)
    .bind(&criticalities)
    .bind(&times)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?;
    // A batch of seal events alone records no new one: an idle journal stops sealing.
    let only_seals = rows
        .iter()
        .all(|(_, _, _, json)| json["name"] == AUDIT_SEALED.name);
    if only_seals {
        tx.commit().await.map_err(store_err)?;
        return Ok(Some(Sealed {
            seq,
            id,
            count: rows.len(),
            hash,
        }));
    }
    let mut event = RequestContext::system()
        .event(&AUDIT_SEALED, Outcome::Success)
        .map_err(store_err)?
        .target("audit_seal", [id]);
    for (k, v) in [
        ("seal", serde_json::json!(id)),
        ("count", serde_json::json!(rows.len())),
        ("hash", serde_json::json!(hex(&hash))),
        ("key_id", serde_json::json!(key.map(SealKey::id))),
    ] {
        event = event.detail(k, v).map_err(store_err)?;
    }
    crate::audit_journal::insert(&mut *tx, &event, instance).await?;
    tx.commit().await.map_err(store_err)?;
    Ok(Some(Sealed {
        seq,
        id,
        count: rows.len(),
        hash,
    }))
}
