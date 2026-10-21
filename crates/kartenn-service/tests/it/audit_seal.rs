//! Sealing the audit journal in chained batches (spec 012, REQ-012-13; task 012-9).

use kartenn_core::audit::seal::{SealKey, verify};
use kartenn_core::audit::{OBJECTS_READ, Outcome};
use kartenn_service::audit::{AuditJournal, Instance, RequestContext};
use kartenn_service::{Store, WriteTx};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::audit_seal::seal_once;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;

fn instance() -> Instance {
    Instance::new("test")
}

async fn record(pool: &PgPool) {
    let event = RequestContext::system()
        .event(&OBJECTS_READ, Outcome::Success)
        .unwrap();
    PgAuditJournal::new(pool.clone(), instance())
        .record(&event)
        .await
        .unwrap();
}

/// `(prev_hash, hash, count, signature)` of the seals, in order.
type SealRow = (Vec<u8>, Vec<u8>, i32, Option<Vec<u8>>);

async fn seals(pool: &PgPool) -> Vec<SealRow> {
    sqlx::query_as("SELECT prev_hash, hash, count, signature FROM audit.seal ORDER BY seq")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn batches_are_chained_signed_and_stop_when_idle(pool: PgPool) {
    let key = SealKey::from_bytes(&[7; 32]);
    for _ in 0..3 {
        record(&pool).await;
    }
    let first = seal_once(&pool, Some(&key), &instance())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.count, 3);
    // Its own event, audit.seal.created, goes into the next batch, which records none.
    let second = seal_once(&pool, Some(&key), &instance())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.count, 1);
    assert!(
        seal_once(&pool, Some(&key), &instance())
            .await
            .unwrap()
            .is_none(),
        "an idle journal is not sealed again"
    );
    let rows = seals(&pool).await;
    assert_eq!(rows[0].0, vec![0; 32], "the first seal starts the chain");
    assert_eq!(rows[1].0, rows[0].1, "each seal names the previous one");
    for (_, hash, _, signature) in &rows {
        let hash: [u8; 32] = hash.as_slice().try_into().unwrap();
        assert!(verify(&key.public(), &hash, signature.as_ref().unwrap()));
    }
    let (events, sealed): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM audit.event), (SELECT count(*) FROM audit.sealed)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((events, sealed), (4, 4));
}

#[kartenn_testing::pg_test]
async fn an_event_committed_late_enters_the_next_batch(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    // A long transaction holds an event with a smaller sequence…
    let mut long = store.begin().await.unwrap();
    long.audit(
        &RequestContext::system()
            .event(&OBJECTS_READ, Outcome::Success)
            .unwrap(),
    )
    .await
    .unwrap();
    // … while a later one commits and is sealed, unsigned.
    record(&pool).await;
    let first = seal_once(&pool, None, &instance()).await.unwrap().unwrap();
    assert_eq!(first.count, 1);
    long.commit().await.unwrap();
    let next = seal_once(&pool, None, &instance()).await.unwrap().unwrap();
    assert_eq!(next.count, 2, "the late event and the first seal's event");
    let unsealed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event e
         WHERE NOT EXISTS (SELECT 1 FROM audit.sealed s WHERE s.event_seq = e.seq)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        unsealed, 1,
        "only the second seal's own event, for the next batch"
    );
    assert!(seals(&pool).await.iter().all(|s| s.3.is_none()));
}
