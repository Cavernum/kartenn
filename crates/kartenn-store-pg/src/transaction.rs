//! Transactions begun on the pool whatever happens to the future that waits for them
//! (task 021-11p11).
//!
//! sqlx 0.9 forgets a transaction whose `BEGIN` is dropped while it waits for the database:
//! the guard of `PgTransactionManager::begin` queues a rollback only at a depth it raises
//! once the `BEGIN` has answered, so a drop in between leaves the `BEGIN` sent, or queued
//! for the ping of the release, and the pool lends the connection inside that transaction.
//! A request whose client leaves is such a drop: hyper drops the future of its handler.
//! The next borrowers then read in its snapshot, write in it (refused in a read-only one,
//! never committed in another), then are all refused once it aborts, until
//! `transaction_timeout` ends the session.
//!
//! Here the `BEGIN` runs in a task of its own, which a dropped caller does not cancel:
//! begun, the transaction returns to the caller, or, the caller gone, is dropped by the
//! task and rolled back by sqlx as any other. Every transaction of the pool begins here
//! (`Pool::begin` and `Pool::begin_with` are not called elsewhere in this crate).

use sqlx::{PgPool, Postgres, Transaction};
use tracing::Instrument;

/// A transaction on `pool`, begun even if the caller's future is dropped meanwhile.
///
/// # Errors
///
/// Database errors; [`sqlx::Error::WorkerCrashed`] if the runtime stops before the task.
pub async fn begin(pool: &PgPool) -> sqlx::Result<Transaction<'static, Postgres>> {
    let pool = pool.clone();
    joined(tokio::spawn(
        async move { pool.begin().await }.in_current_span(),
    ))
    .await
}

/// A transaction on `pool` begun by `statement` (`BEGIN ISOLATION LEVEL …`), as
/// [`begin`].
///
/// # Errors
///
/// As [`begin`].
pub async fn begin_with(
    pool: &PgPool,
    statement: &'static str,
) -> sqlx::Result<Transaction<'static, Postgres>> {
    let pool = pool.clone();
    joined(tokio::spawn(
        async move { pool.begin_with(statement).await }.in_current_span(),
    ))
    .await
}

async fn joined(
    task: tokio::task::JoinHandle<sqlx::Result<Transaction<'static, Postgres>>>,
) -> sqlx::Result<Transaction<'static, Postgres>> {
    match task.await {
        Ok(begun) => begun,
        Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
        Err(_) => Err(sqlx::Error::WorkerCrashed),
    }
}
