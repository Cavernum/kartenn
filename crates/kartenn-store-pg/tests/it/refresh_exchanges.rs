//! The claims of the refresh exchanges shared by the instances (spec 021, A24, A25):
//! a lease not over keeps the others waiting, a lease over is taken over, and the holder
//! that lost its claim neither keeps its result nor gives up the claim of another.

use std::time::Duration;

use kartenn_service::authn::{Claim, RefreshExchanges};
use kartenn_store_pg::refresh_exchanges::{PgRefreshExchanges, purge};
use sqlx::PgPool;
use sqlx::types::Uuid;

const LEASE: Duration = Duration::from_secs(40);
const WINDOW: Duration = Duration::from_secs(30);

/// Brings the end of the lease or window of `key` to the past.
async fn over(pool: &PgPool, key: &[u8; 32]) {
    sqlx::query(
        "UPDATE refresh_exchange SET ends_at = clock_timestamp() - interval '1 second'
          WHERE key = $1",
    )
    .bind(&key[..])
    .execute(pool)
    .await
    .unwrap();
}

/// The holder and sealed result of `key`.
async fn row(pool: &PgPool, key: &[u8; 32]) -> Option<(Option<Uuid>, Option<String>)> {
    sqlx::query_as("SELECT holder, sealed FROM refresh_exchange WHERE key = $1")
        .bind(&key[..])
        .fetch_optional(pool)
        .await
        .unwrap()
}

/// A claims then stops: B waits while the lease runs, A's own claim is not taken over
/// either, then B takes it over at the end of the lease; A's late result neither replaces
/// B's claim nor gives it up; B's result is kept for the next one.
#[kartenn_testing::pg_test]
async fn a_lease_over_is_taken_over_and_a_lost_claim_keeps_nothing(pool: PgPool) {
    let exchanges = PgRefreshExchanges::new(pool.clone());
    let key = [7u8; 32];
    let (a, b, c) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());

    assert_eq!(exchanges.claim(&key, a, LEASE).await.unwrap(), Claim::Ours);
    assert_eq!(exchanges.claim(&key, b, LEASE).await.unwrap(), Claim::Busy);
    // Its exchange runs apart from the request that claimed it: its own instance waits.
    assert_eq!(exchanges.claim(&key, a, LEASE).await.unwrap(), Claim::Busy);

    // A stopped: its lease over, B takes the claim over.
    over(&pool, &key).await;
    assert_eq!(exchanges.claim(&key, b, LEASE).await.unwrap(), Claim::Ours);
    assert_eq!(row(&pool, &key).await, Some((Some(b), None)));

    // A late: its result is not kept, its claim not given up, B's claim stays.
    assert!(!exchanges.keep(&key, a, "late", WINDOW).await.unwrap());
    exchanges.release(&key, a).await.unwrap();
    assert_eq!(row(&pool, &key).await, Some((Some(b), None)));

    // B keeps its result: the next one gets it within the window.
    assert!(exchanges.keep(&key, b, "sealed", WINDOW).await.unwrap());
    assert_eq!(
        exchanges.claim(&key, c, LEASE).await.unwrap(),
        Claim::Kept("sealed".into())
    );
    assert_eq!(purge(&pool).await.unwrap(), 0);

    // Past the window, the row reads as absent: claimed again, then given up.
    over(&pool, &key).await;
    assert_eq!(exchanges.claim(&key, c, LEASE).await.unwrap(), Claim::Ours);
    exchanges.release(&key, c).await.unwrap();
    assert_eq!(row(&pool, &key).await, None);
}

/// A key is a 32-octet fingerprint; a row holds a claim or a result, not both, not none.
#[kartenn_testing::pg_test]
async fn a_row_is_a_claim_or_a_result(pool: PgPool) {
    for (key, holder, sealed) in [
        (vec![1u8; 31], Some(Uuid::now_v7()), None),
        (vec![2u8; 32], None, None),
        (vec![3u8; 32], Some(Uuid::now_v7()), Some("sealed")),
    ] {
        let inserted = sqlx::query(
            "INSERT INTO refresh_exchange (key, holder, sealed, ends_at)
             VALUES ($1, $2, $3, clock_timestamp())",
        )
        .bind(&key)
        .bind(holder)
        .bind(sealed)
        .execute(&pool)
        .await;
        assert!(inserted.is_err(), "{key:?} {holder:?} {sealed:?}");
    }
}
