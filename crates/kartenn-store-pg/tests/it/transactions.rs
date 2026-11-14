//! A transaction begun for a request whose client left (task 021-11p11): the request's
//! future is dropped while its `BEGIN` waits for the database. sqlx 0.9 then forgets the
//! transaction it has sent (the guard of `TransactionManager::begin` rolls back only at a
//! depth it has not raised yet), the release of the connection flushes the `BEGIN`, and the
//! pool lends a connection inside a transaction: reads in a stale snapshot, writes refused
//! (« read-only transaction ») or never committed, then every statement refused once the
//! transaction aborts, until `transaction_timeout` ends the session.

use std::future::{Future, poll_fn};
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Poll;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, Postgres, Transaction};

/// Whether the pool lends a connection inside a transaction once a begin by `begin` is
/// dropped after `polls` polls, for each count up to the one where it completes.
async fn no_transaction_left<F, Fut>(pool: &PgPool, begin: F) -> sqlx::Result<()>
where
    F: Fn(PgPool) -> Fut,
    Fut: Future<Output = sqlx::Result<Transaction<'static, Postgres>>>,
{
    // One connection, so the next acquire gets the one the dropped begin used.
    let one = PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await?;
    for polls in 1..=32 {
        let mut completed = false;
        {
            let mut pending = pin!(begin(one.clone()));
            for _ in 0..polls {
                if poll_fn(|cx| Poll::Ready(pending.as_mut().poll(cx).is_ready())).await {
                    completed = true;
                    break;
                }
                // The database answers in the meantime.
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
        let mut conn = one.acquire().await?;
        // Outside a transaction, each statement is its own, and so is its `now()`.
        let mut starts = Vec::new();
        for _ in 0..2 {
            let start: OffsetDateTime = sqlx::query_scalar("SELECT now()")
                .fetch_one(&mut *conn)
                .await?;
            starts.push(start);
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert_ne!(
            starts[0], starts[1],
            "a begin dropped after {polls} polls left its connection in a transaction"
        );
        if completed {
            drop(conn);
            one.close().await;
            return Ok(());
        }
    }
    panic!("the begin never completed");
}

/// A begin whose caller left while it waited for a connection leaves the queue of the pool
/// with it: no task of its own waits on, to take a connection, `BEGIN` and `ROLLBACK` for
/// nothing ahead of the requests admitted after it.
#[kartenn_testing::pg_test]
async fn a_begin_dropped_while_it_waits_for_a_connection_takes_none(
    pool: PgPool,
) -> sqlx::Result<()> {
    let taken = Arc::new(AtomicUsize::new(0));
    let counted = taken.clone();
    // One connection, each lending of it counted.
    let one = PgPoolOptions::new()
        .max_connections(1)
        .before_acquire(move |_, _| {
            counted.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(true) })
        })
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let held = one.acquire().await?;
    taken.store(0, Ordering::SeqCst);
    for _ in 0..5 {
        let mut waiting = pin!(kartenn_store_pg::transaction::begin(&one));
        let ready = poll_fn(|cx| Poll::Ready(waiting.as_mut().poll(cx).is_ready())).await;
        assert!(!ready, "the connection is held: the begin waits");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    drop(held);
    let tx = kartenn_store_pg::transaction::begin(&one).await?;
    tx.rollback().await?;
    // Any begin left behind would take the connection meanwhile.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        taken.load(Ordering::SeqCst),
        1,
        "only the begin still awaited took the connection"
    );
    one.close().await;
    Ok(())
}

#[kartenn_testing::pg_test]
async fn a_consistent_read_dropped_while_it_begins_leaves_no_transaction(
    pool: PgPool,
) -> sqlx::Result<()> {
    no_transaction_left(&pool, |pool| async move {
        kartenn_store_pg::read::consistent(&pool).await
    })
    .await
}

#[kartenn_testing::pg_test]
async fn a_write_dropped_while_it_begins_leaves_no_transaction(pool: PgPool) -> sqlx::Result<()> {
    no_transaction_left(&pool, |pool| async move {
        kartenn_store_pg::transaction::begin(&pool).await
    })
    .await
}
