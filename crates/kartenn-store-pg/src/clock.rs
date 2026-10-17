//! The clock of the instance against the clock of PostgreSQL (spec 012, REQ-012-32;
//! 012/L14): the base orders the audit (`recorded_at`), an instance whose clock drifts
//! would misdate its events and its technical logs.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kartenn_service::StoreError;
use sqlx::PgPool;
use sqlx::types::time::OffsetDateTime;

/// Beyond this, the drift is reported.
pub const SKEW_TOLERANCE: Duration = Duration::from_secs(1);

fn seconds(t: SystemTime) -> f64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -e.duration().as_secs_f64(),
    }
}

/// Clock of the instance minus clock of PostgreSQL, in seconds, half the round trip
/// taken out; `now` reads the clock of the instance (the system clock in production).
///
/// # Errors
///
/// Database errors.
pub async fn clock_skew(pool: &PgPool, now: impl Fn() -> SystemTime) -> Result<f64, StoreError> {
    let before = seconds(now());
    let base: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(pool)
        .await
        .map_err(|e| StoreError(e.to_string()))?;
    let after = seconds(now());
    let base = seconds(SystemTime::from(base));
    Ok(f64::midpoint(before, after) - base)
}
