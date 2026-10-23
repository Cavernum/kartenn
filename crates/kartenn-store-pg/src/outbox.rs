//! Outbox relay (REQ-004-36, 37; ADR 0018 amended).
//!
//! Events are written in the changeset transaction. The relay reads the committed events
//! never delivered to this consumer, in sequence order, and records each delivery after
//! the sink accepted it: delivery is at least once, without gaps, ordered within an
//! ordering key: an addressing domain, or a lock key of module operations (REQ-016-14,
//! ADR 0018 amendment 4). An event of a transaction still running is invisible and
//! delivered on a later run; it can only be overtaken by events without a key in common,
//! since changesets that share a key are serialised by its lock (ADR 0015, ADR 0026
//! clause 14) and so number their events in commit order. There is no barrier on the
//! oldest running transaction: it is cluster-wide, and any long transaction (a backup, a
//! report) would stall every consumer.

use std::future::Future;

use kartenn_service::StoreError;
use sqlx::types::Uuid;
use sqlx::{PgConnection, PgPool};

/// The channel the shared reader of the event stream listens on (spec 022, E1; ADR
/// 0034): notified, with an empty payload, by every transaction that writes the outbox.
pub const CHANNEL: &str = "kartenn_outbox";

/// Notifies [`CHANNEL`] in the transaction of `conn`: delivered at its commit only, and
/// once per transaction however many times it is called (PostgreSQL merges identical
/// notifications of a transaction).
///
/// # Errors
///
/// Database errors.
pub(crate) async fn notify(conn: &mut PgConnection) -> Result<(), StoreError> {
    sqlx::query("SELECT pg_notify($1, '')")
        .bind(CHANNEL)
        .execute(conn)
        .await
        .map_err(|e| StoreError(e.to_string()))?;
    Ok(())
}

/// An event to deliver, in CloudEvents 1.0 JSON form.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Sequence number (delivery order).
    pub seq: i64,
    /// Changeset that produced the event (the CloudEvents `id` of its own event); none
    /// for an event of a background task of a module (REQ-016-15).
    pub changeset: Option<Uuid>,
    /// Addressing domains written by the changeset.
    pub domains: Vec<Uuid>,
    /// Ordering keys: `domain:<uuid>` for each domain, then the lock keys of its module
    /// operations (REQ-016-14).
    pub order_keys: Vec<String>,
    /// CloudEvents envelope and data.
    pub payload: serde_json::Value,
}

/// Errors of the relay.
#[derive(Debug, thiserror::Error)]
pub enum RelayError<E> {
    /// Database failure.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// The sink refused an event; it will be delivered again.
    #[error("the sink refused event {seq}")]
    Sink {
        /// Sequence number of the refused event.
        seq: i64,
        /// Error of the sink.
        source: E,
    },
}

/// A row of the outbox: sequence, changeset, domains, ordering keys, payload.
type Row = (i64, Option<Uuid>, Vec<Uuid>, Vec<String>, serde_json::Value);

/// The events a consumer has not received (from the floor of its relay, migration 0027).
const UNDELIVERED: crate::floor::Rows = crate::floor::Rows {
    table: "outbox",
    seq: "seq",
    unhandled: "NOT EXISTS (SELECT 1 FROM outbox_delivery d WHERE d.consumer = $1 AND d.seq = e.seq)",
};

/// Delivers up to `batch` pending events to `sink`, in order, and records each delivery.
/// Returns the number of events delivered. A single relay per consumer runs at a time
/// (transaction-level advisory lock on the consumer name).
///
/// # Errors
///
/// [`RelayError::Database`], or [`RelayError::Sink`] when the sink refuses an event (the
/// events delivered before it stay recorded).
pub async fn relay<F, Fut, E>(
    pool: &PgPool,
    consumer: &str,
    batch: i64,
    mut sink: F,
) -> Result<usize, RelayError<E>>
where
    F: FnMut(Event) -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('outbox/' || $1, 0))")
        .bind(consumer)
        .execute(&mut *tx)
        .await?;
    let floor = crate::floor::floor(
        &mut tx,
        &format!("outbox/{consumer}"),
        consumer,
        &UNDELIVERED,
    )
    .await?;
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT o.seq, o.changeset_id, o.domain_ids, o.order_keys, o.payload FROM outbox o
         WHERE o.seq >= $3
           AND NOT EXISTS (SELECT 1 FROM outbox_delivery d WHERE d.consumer = $1 AND d.seq = o.seq)
         ORDER BY o.seq LIMIT $2",
    )
    .bind(consumer)
    .bind(batch)
    .bind(floor)
    .fetch_all(&mut *tx)
    .await?;
    let mut delivered = 0;
    for (seq, changeset, domains, order_keys, payload) in rows {
        if let Err(source) = sink(Event {
            seq,
            changeset,
            domains,
            order_keys,
            payload,
        })
        .await
        {
            tx.commit().await?;
            return Err(RelayError::Sink { seq, source });
        }
        sqlx::query("INSERT INTO outbox_delivery (consumer, seq) VALUES ($1, $2)")
            .bind(consumer)
            .bind(seq)
            .execute(&mut *tx)
            .await?;
        delivered += 1;
    }
    tx.commit().await?;
    Ok(delivered)
}
