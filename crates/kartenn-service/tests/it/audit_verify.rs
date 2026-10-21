//! Verifying the audit journal: modification, deletion and addition detected, purges
//! allowed, signatures checked (spec 012, REQ-012-14, 16; task 012-10).

use kartenn_core::audit::seal::SealKey;
use kartenn_core::audit::{OBJECTS_READ, Outcome};
use kartenn_service::audit::{AuditJournal, Instance, RequestContext};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::audit_seal::seal_once;
use kartenn_store_pg::audit_verify::{verify, verify_and_record};
use sqlx::PgPool;

fn instance() -> Instance {
    Instance::new("test")
}

/// Two signed batches: 3 events, then 2 more with the first seal's event.
async fn journal(pool: &PgPool) -> SealKey {
    let key = SealKey::from_bytes(&[7; 32]);
    let journal = PgAuditJournal::new(pool.clone(), instance());
    for batch in [3, 2] {
        for _ in 0..batch {
            let event = RequestContext::system()
                .event(&OBJECTS_READ, Outcome::Success)
                .unwrap();
            journal.record(&event).await.unwrap();
        }
        seal_once(pool, Some(&key), &instance()).await.unwrap();
    }
    key
}

async fn exec(pool: &PgPool, sql: &str) {
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_intact_journal_verifies(pool: PgPool) {
    let key = journal(&pool).await;
    let report = verify(&pool, Some(&key.public())).await.unwrap();
    assert_eq!(report.broken, None);
    assert_eq!((report.seals, report.events, report.purged), (2, 6, 0));
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_modified_event_breaks_its_seal(pool: PgPool) {
    journal(&pool).await;
    exec(
        &pool,
        "UPDATE audit.event SET motive = 'rewritten' WHERE seq = 2",
    )
    .await;
    let broken = verify(&pool, None).await.unwrap().broken.unwrap();
    assert_eq!(broken.seal, 1);
    assert!(broken.reason.contains("event 2 was modified"), "{broken:?}");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_deleted_event_breaks_its_seal_unless_purged(pool: PgPool) {
    journal(&pool).await;
    exec(&pool, "DELETE FROM audit.event WHERE seq = 5").await;
    let broken = verify(&pool, None).await.unwrap().broken.unwrap();
    assert_eq!(broken.seal, 2);
    assert!(broken.reason.contains("deleted"), "{broken:?}");
    // The same absence after a recorded purge of its partition is retention.
    exec(
        &pool,
        "DELETE FROM audit.event WHERE criticality = 'low';
         INSERT INTO audit.purge (criticality, month) VALUES ('low', date_trunc('month', now())::date)",
    )
    .await;
    let report = verify(&pool, None).await.unwrap();
    assert_eq!(report.broken, None, "{report:?}");
    assert_eq!(
        (report.events, report.purged),
        (1, 5),
        "the seal's event is normal"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_tampered_seal_or_member_breaks_the_chain(pool: PgPool) {
    journal(&pool).await;
    exec(
        &pool,
        "UPDATE audit.seal SET hash = sha256('forged') WHERE seq = 1",
    )
    .await;
    let broken = verify(&pool, None).await.unwrap().broken.unwrap();
    assert_eq!(broken.seal, 1, "{broken:?}");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_added_member_breaks_its_seal(pool: PgPool) {
    journal(&pool).await;
    exec(
        &pool,
        "INSERT INTO audit.sealed (seal_seq, event_seq, criticality, recorded_at, event_hash)
         VALUES (1, 999, 'low', now(), sha256('x'))",
    )
    .await;
    let broken = verify(&pool, None).await.unwrap().broken.unwrap();
    assert_eq!(broken.seal, 1);
    assert!(broken.reason.contains("members"), "{broken:?}");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn another_key_does_not_verify_and_the_break_is_recorded(pool: PgPool) {
    journal(&pool).await;
    let other = SealKey::from_bytes(&[8; 32]);
    let report = verify_and_record(&pool, Some(&other.public()), &instance())
        .await
        .unwrap();
    assert!(report.broken.unwrap().reason.contains("signature"));
    let recorded: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'audit.chain.broken'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recorded, 1);
}
