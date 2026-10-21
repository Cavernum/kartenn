//! The audit schema: insert and read only for the application, attribution enforced,
//! partitions by criticality and month kept ahead (spec 012, REQ-012-9, 11, 12, 16;
//! task 012-4).

use kartenn_store_pg::maintenance::history_partitions;
use sqlx::PgPool;
use sqlx::types::Uuid;

const INSERT: &str = "INSERT INTO audit.event
    (id, name, severity, criticality, occurred_at, actor_kind, actor_id, claimed, outcome,
     service_version, instance_id)
    VALUES (gen_random_uuid(), 'authz.access.denied', 'WARN', $1, now(), $2, $3, $4, 'denied',
            '0.0.0', gen_random_uuid())";

#[kartenn_testing::pg_test]
async fn the_application_only_inserts_and_reads(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE kartenn_app")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(INSERT)
        .bind("high")
        .bind("person")
        .bind(Uuid::now_v7())
        .bind(None::<String>)
        .execute(&mut *tx)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit.event")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(count, 1);
    for forbidden in [
        "UPDATE audit.event SET motive = 'x'",
        "DELETE FROM audit.event",
        "TRUNCATE audit.event",
        "DELETE FROM audit.seal",
        "UPDATE audit.instance_key SET hmac_key = hmac_key",
        "INSERT INTO audit.instance_key VALUES (1, '\\x00')",
    ] {
        sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
        let err = sqlx::query(forbidden).execute(&mut *tx).await.unwrap_err();
        assert!(
            err.to_string().contains("permission denied"),
            "{forbidden}: {err}"
        );
        sqlx::query("ROLLBACK TO SAVEPOINT s")
            .execute(&mut *tx)
            .await
            .unwrap();
    }
}

#[kartenn_testing::pg_test]
async fn every_event_is_attributed(pool: PgPool) {
    let insert = |criticality: &'static str, kind: &'static str, claimed: Option<&'static str>| {
        let pool = pool.clone();
        async move {
            sqlx::query(INSERT)
                .bind(criticality)
                .bind(kind)
                .bind(None::<Uuid>)
                .bind(claimed)
                .execute(&pool)
                .await
        }
    };
    assert!(
        insert("high", "person", None).await.is_err(),
        "person without id"
    );
    assert!(
        insert("high", "unknown", None).await.is_err(),
        "unknown without fingerprint"
    );
    assert!(insert("high", "unknown", Some("9f86d081")).await.is_ok());
    assert!(insert("normal", "system", None).await.is_ok());
    assert!(
        insert("medium", "system", None).await.is_err(),
        "unknown criticality"
    );
}

#[kartenn_testing::pg_test]
async fn partitions_by_criticality_and_month(pool: PgPool) {
    history_partitions(&pool, 3).await.unwrap();
    let month: String = sqlx::query_scalar(
        "SELECT to_char(date_trunc('month', now()) + interval '2 months', 'YYYYMM')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for table in ["event", "sealed", "delivery"] {
        for criticality in ["low", "normal", "high"] {
            let name = format!("audit.{table}_{criticality}_{month}");
            let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
                .bind(&name)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert!(exists, "{name}");
        }
    }
    // An event lands in the partition of its criticality and month, not the default.
    sqlx::query(INSERT)
        .bind("low")
        .bind("system")
        .bind(None::<Uuid>)
        .bind(None::<String>)
        .execute(&pool)
        .await
        .unwrap();
    let partition: String = sqlx::query_scalar("SELECT tableoid::regclass::text FROM audit.event")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        partition.starts_with("audit.event_low_2") && !partition.ends_with("default"),
        "{partition}"
    );
    let key: i32 = sqlx::query_scalar("SELECT octet_length(hmac_key) FROM audit.instance_key")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(key, 32);
}
