//! The pool of an instance bounds its transactions (spec 006, REQ-006-6; task 006-9): the
//! frozen past of the reads rests on it.

use std::time::Duration;

use sqlx::PgPool;

#[kartenn_testing::pg_test]
async fn a_transaction_ends_within_the_bound(pool: PgPool) -> sqlx::Result<()> {
    let options = (*pool.connect_options()).clone();
    let bounded = PgPool::connect_with(kartenn_server::app::bounded(
        options,
        Duration::from_millis(500),
    ))
    .await?;
    let setting: String = sqlx::query_scalar("SHOW transaction_timeout")
        .fetch_one(&bounded)
        .await?;
    assert_eq!(setting, "500ms");
    let mut tx = bounded.begin().await?;
    let slow = sqlx::query("SELECT pg_sleep(1.5)").execute(&mut *tx).await;
    assert!(
        slow.is_err(),
        "a transaction longer than the bound is ended"
    );
    Ok(())
}
