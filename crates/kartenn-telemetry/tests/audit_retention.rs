//! Retention by criticality: a month is dropped only once sealed and exported, and the
//! chain still verifies (spec 012, REQ-012-16; task 012-12).

use std::future::Future;

use kartenn_service::audit::{AuditSink, Instance, SinkError};
use kartenn_store_pg::audit_purge::{Retention, purge};
use kartenn_store_pg::audit_relay::relay_once;
use kartenn_store_pg::audit_seal::seal_once;
use kartenn_store_pg::audit_verify::verify;
use serde_json::Value;
use sqlx::PgPool;

struct Memory;

impl AuditSink for Memory {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn deliver(&self, _: &[Value]) -> impl Future<Output = Result<(), SinkError>> + Send {
        std::future::ready(Ok(()))
    }
}

async fn event(pool: &PgPool, criticality: &str, recorded_at: &str) {
    sqlx::query(
        "INSERT INTO audit.event (id, name, severity, criticality, occurred_at, recorded_at,
                                  actor_kind, outcome, service_version, instance_id)
         VALUES (gen_random_uuid(), 'data.objects.read', 'INFO', $1, $2::timestamptz,
                 $2::timestamptz, 'system', 'success', 'test', gen_random_uuid())",
    )
    .bind(criticality)
    .bind(recorded_at)
    .execute(pool)
    .await
    .unwrap();
}

async fn exists(pool: &PgPool, relation: &str) -> bool {
    sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(relation)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_month_past_retention_goes_once_sealed_and_exported(pool: PgPool) {
    sqlx::query("CALL kartenn_audit_partitions('2026-01-01', 1)")
        .execute(&pool)
        .await
        .unwrap();
    event(&pool, "low", "2026-01-15T10:00:00Z").await;
    event(&pool, "low", "2026-01-20T10:00:00Z").await;
    event(&pool, "normal", "2026-01-20T11:00:00Z").await;
    event(&pool, "low", "now").await;
    let instance = Instance::new("test");
    seal_once(&pool, None, &instance).await.unwrap();

    // Sealed but not exported: kept.
    let first = purge(&pool, &Retention::default(), &["memory"])
        .await
        .unwrap();
    assert_eq!(first.len(), 1, "{first:?}");
    assert!(!first[0].dropped);
    assert!(exists(&pool, "audit.event_low_202601").await);

    relay_once(&pool, &Memory, 500).await.unwrap();
    let second = purge(&pool, &Retention::default(), &["memory"])
        .await
        .unwrap();
    assert!(
        second[0].dropped && second[0].criticality == "low",
        "{second:?}"
    );
    assert!(!exists(&pool, "audit.event_low_202601").await);
    let left: Vec<String> = sqlx::query_scalar("SELECT criticality FROM audit.event ORDER BY seq")
        .fetch_all(&pool)
        .await
        .unwrap();
    // The normal event of January stays a year; the low event of now and the seal's event too.
    assert_eq!(left, ["normal", "low", "normal"]);
    let report = verify(&pool, None).await.unwrap();
    assert_eq!(report.broken, None, "{report:?}");
    assert_eq!(report.purged, 2);
}
