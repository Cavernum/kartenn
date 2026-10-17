//! The clock of the instance against the clock of PostgreSQL (spec 012, REQ-012-32; task
//! 012-18).

use std::time::{Duration, SystemTime};

use kartenn_store_pg::clock::{SKEW_TOLERANCE, clock_skew};
use sqlx::PgPool;

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_drifting_clock_is_measured(pool: PgPool) {
    let synced = clock_skew(&pool, SystemTime::now).await.unwrap();
    assert!(synced.abs() < SKEW_TOLERANCE.as_secs_f64(), "{synced}");
    let ahead = clock_skew(&pool, || SystemTime::now() + Duration::from_secs(5))
        .await
        .unwrap();
    assert!((ahead - 5.0).abs() < 0.5, "{ahead}");
}
