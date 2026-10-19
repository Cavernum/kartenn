//! The leases of the background tasks (spec 006, REQ-006-38): an instance takes a due task
//! whose lease is free or expired, renews the lease while it runs, then reschedules the
//! task one period later and frees the lease. The database clock decides (`now()`), so a
//! skewed instance clock changes nothing.

use std::time::Duration;

use kartenn_service::port::StoreError;
use sqlx::PgPool;
use sqlx::types::Uuid;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Takes, for `holder` and `lease`, the earliest due task whose lease is free or expired;
/// the other instances skip it while it is locked. Returns its name.
///
/// # Errors
///
/// Database errors.
pub async fn claim(
    pool: &PgPool,
    holder: Uuid,
    lease: Duration,
) -> Result<Option<String>, StoreError> {
    sqlx::query_scalar(
        "UPDATE job SET holder = $1, lease_until = now() + make_interval(secs => $2),
                        heartbeat_at = now()
          WHERE name = (SELECT name FROM job
                         WHERE due_at <= now() AND (lease_until IS NULL OR lease_until < now())
                         ORDER BY due_at
                         FOR UPDATE SKIP LOCKED LIMIT 1)
         RETURNING name",
    )
    .bind(holder)
    .bind(lease.as_secs_f64())
    .fetch_optional(pool)
    .await
    .map_err(store_err)
}

/// Renews the lease of `name` held by `holder`; false if the lease was lost (expired and
/// taken by another instance).
///
/// # Errors
///
/// Database errors.
pub async fn heartbeat(
    pool: &PgPool,
    name: &str,
    holder: Uuid,
    lease: Duration,
) -> Result<bool, StoreError> {
    sqlx::query(
        "UPDATE job SET lease_until = now() + make_interval(secs => $3), heartbeat_at = now()
          WHERE name = $1 AND holder = $2",
    )
    .bind(name)
    .bind(holder)
    .bind(lease.as_secs_f64())
    .execute(pool)
    .await
    .map(|done| done.rows_affected() == 1)
    .map_err(store_err)
}

/// Reschedules `name` one period from now and frees its lease, if `holder` still holds
/// it; false otherwise.
///
/// # Errors
///
/// Database errors.
pub async fn finish(pool: &PgPool, name: &str, holder: Uuid) -> Result<bool, StoreError> {
    sqlx::query(
        "UPDATE job SET due_at = now() + period, holder = NULL, lease_until = NULL
          WHERE name = $1 AND holder = $2",
    )
    .bind(name)
    .bind(holder)
    .execute(pool)
    .await
    .map(|done| done.rows_affected() == 1)
    .map_err(store_err)
}

/// Makes `name` due now, unless it is due already.
///
/// # Errors
///
/// Database errors.
pub async fn due_now(pool: &PgPool, name: &str) -> Result<(), StoreError> {
    sqlx::query("UPDATE job SET due_at = least(due_at, now()) WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(store_err)
}
