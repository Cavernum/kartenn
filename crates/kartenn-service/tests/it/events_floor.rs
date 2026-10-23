//! The floor in memory of the shared reader of the event stream (spec 022, E6,
//! REQ-022-7): a long transaction that holds a lower number is never skipped, and the
//! floor stays below it while it runs.

use std::collections::BTreeSet;

use kartenn_store_pg::events::{Floor, raise, read};
use sqlx::PgPool;
use sqlx::types::Uuid;

async fn changeset(pool: &PgPool) -> Uuid {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
         VALUES (gen_random_uuid(), $1, 'm', 'applied', 'direct', tstzrange(now(), NULL))
         RETURNING id",
    )
    .bind(author)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// One pass of a reader: the floor raised when it can, then the events above it.
async fn pass(pool: &PgPool, floor: &mut Floor, seen: &mut BTreeSet<i64>) -> Vec<i64> {
    let mut conn = pool.acquire().await.unwrap();
    raise(&mut conn, floor, seen).await.unwrap();
    let got: Vec<i64> = read(&mut conn, floor.floor, seen, 1_000)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.seq)
        .collect();
    seen.extend(&got);
    got
}

#[kartenn_testing::pg_test]
async fn a_long_transaction_holding_a_lower_number_is_never_skipped(pool: PgPool) {
    let (c1, c2) = (changeset(&pool).await, changeset(&pool).await);
    let mut floor = Floor::at(0);
    let mut seen = BTreeSet::new();
    assert!(pass(&pool, &mut floor, &mut seen).await.is_empty());
    let mut long = pool.begin().await.unwrap();
    let low: i64 = sqlx::query_scalar(
        "INSERT INTO outbox (changeset_id, payload) VALUES ($1, '{}') RETURNING seq",
    )
    .bind(c1)
    .fetch_one(&mut *long)
    .await
    .unwrap();
    let high: i64 = sqlx::query_scalar(
        "INSERT INTO outbox (changeset_id, payload) VALUES ($1, '{}') RETURNING seq",
    )
    .bind(c2)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(low < high);
    // The later one is read at once; the floor stays below the running one, pass after
    // pass.
    assert_eq!(pass(&pool, &mut floor, &mut seen).await, vec![high]);
    for _ in 0..5 {
        assert!(pass(&pool, &mut floor, &mut seen).await.is_empty());
        assert!(floor.floor < low, "the floor never passes a running number");
    }
    long.commit().await.unwrap();
    // Committed afterwards, it is read, once.
    assert_eq!(pass(&pool, &mut floor, &mut seen).await, vec![low]);
    for _ in 0..3 {
        assert!(pass(&pool, &mut floor, &mut seen).await.is_empty());
    }
    assert!(floor.floor <= high);
}
