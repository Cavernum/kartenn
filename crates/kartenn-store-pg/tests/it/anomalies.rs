//! The identity of an anomaly (spec 021, A39, task 11n): one per rule, object and property.

use kartenn_core::rules::{PROP_EXPECTED_MISMATCH, Violation};
use kartenn_store_pg::anomalies::{self, AnomalyFilter};
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

/// An applied changeset, to open anomalies by.
async fn changeset(pool: &PgPool) -> sqlx::Result<Uuid> {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
    )
    .fetch_one(pool)
    .await?;
    sqlx::query_scalar(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
         VALUES (gen_random_uuid(), $1, 'test', 'applied', 'direct', tstzrange(now(), NULL))
         RETURNING id",
    )
    .bind(author)
    .fetch_one(pool)
    .await
}

/// The deviation of `key` on `object`, resolved `resolved`.
fn gap(object: Uuid, key: &str, resolved: &str) -> Violation {
    Violation::new(
        &PROP_EXPECTED_MISMATCH,
        format!("{key}: resolved {resolved:?}, expected \"blue\" by src@1 (rule)"),
    )
    .with_subjects([object])
    .with_detail(json!({
        "property": key, "resolved": resolved, "expected": "blue",
        "source": "src", "revision": "1",
    }))
}

/// Records `violations` of a changeset that evaluated the deviations of `object` anew.
async fn record(pool: &PgPool, object: Uuid, violations: &[Violation]) -> sqlx::Result<()> {
    let changeset = changeset(pool).await?;
    let mut tx = pool.begin().await?;
    anomalies::record(
        &mut tx,
        changeset,
        &[(object, "prop.resolution.")],
        violations,
    )
    .await?;
    tx.commit().await
}

/// The open deviations of `property` (all of them when `None`), by property.
async fn open(pool: &PgPool, property: Option<&str>) -> sqlx::Result<Vec<(String, Uuid)>> {
    let filter = AnomalyFilter {
        rule: Some(PROP_EXPECTED_MISMATCH.id),
        property,
        ..AnomalyFilter::default()
    };
    let mut found: Vec<(String, Uuid)> = anomalies::open(pool, &filter)
        .await?
        .into_iter()
        .map(|a| {
            let key = a.detail.unwrap()["property"].as_str().unwrap().to_owned();
            (key, a.id)
        })
        .collect();
    found.sort();
    Ok(found)
}

#[kartenn_testing::pg_test]
async fn two_properties_in_deviation_are_two_gaps(pool: PgPool) -> sqlx::Result<()> {
    // No object row: an anomaly keys its object by identifier only.
    let object = Uuid::from_u128(0x11);
    record(
        &pool,
        object,
        &[
            gap(object, "policy.env", "test"),
            gap(object, "policy.zone", "red"),
        ],
    )
    .await?;
    let both = open(&pool, None).await?;
    let keys: Vec<&str> = both.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["policy.env", "policy.zone"]);
    // The `property` filter reads the detail.
    assert_eq!(open(&pool, Some("policy.zone")).await?, both[1..]);

    // The zone corrected: its gap closes, that of the environment stays, refreshed.
    record(&pool, object, &[gap(object, "policy.env", "prod")]).await?;
    assert_eq!(open(&pool, None).await?, both[..1]);
    assert!(open(&pool, Some("policy.zone")).await?.is_empty());
    let message: String = sqlx::query_scalar("SELECT message FROM anomaly WHERE id = $1")
        .bind(both[0].1)
        .fetch_one(&pool)
        .await?;
    assert!(message.contains("\"prod\""), "{message}");
    Ok(())
}
