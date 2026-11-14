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
//! Here the connection is taken in the caller's future, which a dropped caller leaves with
//! its place in the queue of the pool; only the `BEGIN` on the connection taken runs in a
//! task of its own, which a dropped caller does not cancel: begun, the transaction returns
//! to the caller, or, the caller gone, is dropped by the task and rolled back by sqlx as any
//! other. The task costs a spawn per transaction. The `BEGIN` goes through
//! `Transaction::begin` on the connection, what `Pool::begin` does once it has one (public
//! but hidden in sqlx's documentation: to check at each upgrade of sqlx). Every transaction
//! of the pool begins here (`Pool::begin` and `Pool::begin_with` are not called elsewhere
//! in this crate).

use sqlx::{PgPool, Postgres, SqlSafeStr, SqlStr, Transaction};
use tracing::Instrument;

/// A transaction on `pool`, begun even if the caller's future is dropped once it holds a
/// connection.
///
/// # Errors
///
/// Database errors; [`sqlx::Error::WorkerCrashed`] if the runtime stops before the task.
pub async fn begin(pool: &PgPool) -> sqlx::Result<Transaction<'static, Postgres>> {
    on_taken(pool, None).await
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
    on_taken(pool, Some(statement.into_sql_str())).await
}

async fn on_taken(
    pool: &PgPool,
    statement: Option<SqlStr>,
) -> sqlx::Result<Transaction<'static, Postgres>> {
    // Cancellable: a caller dropped here takes no connection.
    let conn = pool.acquire().await?;
    let task = tokio::spawn(Transaction::begin(conn, statement).in_current_span());
    match task.await {
        Ok(begun) => begun,
        Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
        Err(_) => Err(sqlx::Error::WorkerCrashed),
    }
}
