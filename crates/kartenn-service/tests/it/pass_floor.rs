//! Floors of the passes (migration 0027): a pass reads from its floor up; every row below
//! the floor is handled, a running transaction holding a lower number is never skipped,
//! and the hourly sweep catches a row the floor passed (spec 012, REQ-012-13, 15).

use std::sync::Mutex;

use kartenn_core::audit::{OBJECTS_READ, Outcome};
use kartenn_service::audit::{AuditJournal, AuditSink, Instance, RequestContext, SinkError};
use kartenn_service::{Store, WriteTx};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::audit_relay::relay_once;
use kartenn_store_pg::audit_seal::seal_once;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;

#[derive(Default)]
struct Sink {
    received: Mutex<Vec<i64>>,
}

impl AuditSink for Sink {
    fn name(&self) -> &'static str {
        "archive"
    }

    fn deliver(
        &self,
        events: &[serde_json::Value],
    ) -> impl Future<Output = Result<(), SinkError>> + Send {
        self.received
            .lock()
            .unwrap()
            .extend(events.iter().map(|e| e["seq"].as_i64().unwrap()));
        std::future::ready(Ok(()))
    }
}

fn event() -> kartenn_core::audit::AuditEvent {
    RequestContext::system()
        .event(&OBJECTS_READ, Outcome::Success)
        .unwrap()
}

async fn record(pool: &PgPool, n: usize) {
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("test"));
    for _ in 0..n {
        journal.record(&event()).await.unwrap();
    }
}

async fn floor(pool: &PgPool, pass: &str) -> i64 {
    sqlx::query_scalar("SELECT floor FROM pass_floor WHERE pass = $1")
        .bind(pass)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Events below the floor of the relay that the output has not received.
async fn missed(pool: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM audit.event e
         WHERE e.seq < (SELECT floor FROM pass_floor WHERE pass = 'audit.relay/archive')
           AND NOT EXISTS (SELECT 1 FROM audit.delivery d
                           WHERE d.sink = 'archive' AND d.event_seq = e.seq)",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_floor_rises_behind_the_handled_rows(pool: PgPool) {
    let sink = Sink::default();
    record(&pool, 20).await;
    let max: i64 = sqlx::query_scalar("SELECT max(seq) FROM audit.event")
        .fetch_one(&pool)
        .await
        .unwrap();
    // The floor rises once the transactions of the whole server that could hold a lower
    // number have ended: other tests run beside this one, some for tens of seconds (a
    // million-element import), hence up to a minute.
    for _ in 0..1200 {
        relay_once(&pool, &sink, 500).await.unwrap();
        while seal_once(&pool, None, &Instance::new("test"))
            .await
            .unwrap()
            .is_some()
        {}
        if floor(&pool, "audit.relay/archive").await > max && floor(&pool, "audit.seal").await > max
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        floor(&pool, "audit.relay/archive").await > max,
        "past the delivered events"
    );
    assert!(
        floor(&pool, "audit.seal").await > max,
        "past the sealed events"
    );
    assert_eq!(missed(&pool).await, 0);
    let unsealed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event e
         WHERE e.seq < (SELECT floor FROM pass_floor WHERE pass = 'audit.seal')
           AND NOT EXISTS (SELECT 1 FROM audit.sealed s WHERE s.event_seq = e.seq)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unsealed, 0, "every event below the floor is sealed");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_running_transaction_holds_the_floor(pool: PgPool) {
    let sink = Sink::default();
    let store = PgStore::new(pool.clone());
    let mut long = store.begin().await.unwrap();
    long.audit(&event()).await.unwrap();
    let held: i64 = sqlx::query_scalar("SELECT last_value FROM audit.event_seq_seq")
        .fetch_one(&pool)
        .await
        .unwrap_or(1);
    record(&pool, 5).await;
    for _ in 0..4 {
        relay_once(&pool, &sink, 500).await.unwrap();
    }
    assert_eq!(
        sink.received.lock().unwrap().len(),
        5,
        "committed events go"
    );
    assert!(
        floor(&pool, "audit.relay/archive").await <= held,
        "the floor waits for the running transaction"
    );
    long.commit().await.unwrap();
    for _ in 0..3 {
        relay_once(&pool, &sink, 500).await.unwrap();
    }
    let mut received = sink.received.lock().unwrap().clone();
    received.sort_unstable();
    received.dedup();
    assert_eq!(received.len(), 6, "the late event is delivered, once");
    assert_eq!(missed(&pool).await, 0);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_sweep_catches_a_row_the_floor_passed(pool: PgPool) {
    let sink = Sink::default();
    record(&pool, 3).await;
    relay_once(&pool, &sink, 500).await.unwrap();
    record(&pool, 1).await;
    // A floor past an event not delivered: what a number drawn before its transaction
    // had an id could cause.
    sqlx::query(
        "UPDATE pass_floor SET floor = (SELECT max(seq) + 1 FROM audit.event),
                               pending_max = NULL, pending_xmax = NULL
         WHERE pass = 'audit.relay/archive'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(relay_once(&pool, &sink, 500).await.unwrap(), 0, "passed");
    sqlx::query(
        "UPDATE pass_floor SET swept_at = now() - interval '2 hours'
         WHERE pass = 'audit.relay/archive'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        relay_once(&pool, &sink, 500).await.unwrap(),
        1,
        "the sweep delivers it"
    );
    assert_eq!(sink.received.lock().unwrap().len(), 4);
}
