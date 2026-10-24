//! The audit journal in PostgreSQL: events recorded with their context, atomically with
//! the action, and a failure that fails the action (spec 012, REQ-012-9 to 11; task 012-5).

use kartenn_core::audit::{AUTHZ_DENIED, OBJECTS_READ};
use kartenn_service::audit::{AuditJournal, Instance, Outcome, RequestContext};
use kartenn_service::{Store, WriteTx};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

async fn count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit.event")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn an_event_is_recorded_with_its_context(pool: PgPool) {
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("1.2.3"));
    let person = Uuid::now_v7();
    let mut context = RequestContext::person(person);
    context.source.ip = Some("192.0.2.7".parse().unwrap());
    context.correlation.request_id = Some("req-42".into());
    let object = Uuid::now_v7();
    let event = context
        .event(&AUTHZ_DENIED, Outcome::Denied)
        .unwrap()
        .target("prefix", [object])
        .detail("permission", "prop.custom.env.edit")
        .unwrap();
    journal.record(&event).await.unwrap();
    let row: (
        String,
        String,
        String,
        Uuid,
        String,
        Vec<Uuid>,
        String,
        String,
        serde_json::Value,
    ) = sqlx::query_as(
        "SELECT name, criticality, severity, actor_id, host(source_ip), target_ids,
                    request_id, service_version, details
             FROM audit.event",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "authz.access.denied");
    assert_eq!((row.1.as_str(), row.2.as_str()), ("high", "WARN"));
    assert_eq!(row.3, person);
    assert_eq!(row.4, "192.0.2.7");
    assert_eq!(row.5, vec![object]);
    assert_eq!((row.6.as_str(), row.7.as_str()), ("req-42", "1.2.3"));
    assert_eq!(row.8["permission"], "prop.custom.env.edit");
}

#[kartenn_testing::pg_test]
async fn an_event_in_a_transaction_shares_its_fate(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let event = RequestContext::system()
        .event(&OBJECTS_READ, Outcome::Success)
        .unwrap();
    let mut tx = store.begin().await.unwrap();
    tx.audit(&event).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(count(&pool).await, 0, "rolled back with the action");
    let mut tx = store.begin().await.unwrap();
    tx.audit(&event).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(count(&pool).await, 1);
}

#[kartenn_testing::pg_test]
async fn a_failed_audit_fails_the_action(pool: PgPool) {
    // The audit cannot be written: every insertion is refused.
    sqlx::raw_sql(
        "CREATE FUNCTION refuse() RETURNS trigger LANGUAGE plpgsql AS
             $$ BEGIN RAISE 'audit unavailable'; END $$;
         CREATE TRIGGER refuse BEFORE INSERT ON audit.event FOR EACH ROW EXECUTE FUNCTION refuse();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let event = RequestContext::system()
        .event(&OBJECTS_READ, Outcome::Success)
        .unwrap();
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("1.2.3"));
    assert!(journal.record(&event).await.is_err());
    // An action and its event in one transaction: the failed event takes the action down
    // (PostgreSQL turns the commit of an aborted transaction into a rollback).
    let store = PgStore::new(pool.clone());
    let mut tx = store.begin().await.unwrap();
    tx.insert_domain(Uuid::now_v7(), "probe", None, Uuid::now_v7())
        .await
        .unwrap();
    assert!(tx.audit(&event).await.is_err());
    let _ = tx.commit().await;
    let domains: i64 = sqlx::query_scalar("SELECT count(*) FROM addressing_domain")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(domains, 0, "the action was not kept");
}
