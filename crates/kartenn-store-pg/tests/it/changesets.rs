//! Changeset, audit and outbox tables (spec 004, task 004-5).

use sqlx::{PgPool, Row};

async fn person(pool: &PgPool) -> sqlx::Result<sqlx::types::Uuid> {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
    )
    .fetch_one(pool)
    .await
}

async fn changeset(
    pool: &PgPool,
    author: sqlx::types::Uuid,
    state: &str,
) -> sqlx::Result<sqlx::types::Uuid> {
    sqlx::query_scalar(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period, expires_at, request)
         VALUES (gen_random_uuid(), $1, 'test', $2, 'direct', tstzrange(now(), NULL),
                 CASE WHEN $2 = 'pending' THEN now() + interval '30 days' END,
                 CASE WHEN $2 = 'pending' THEN '{}'::jsonb END)
         RETURNING id",
    )
    .bind(author)
    .bind(state)
    .fetch_one(pool)
    .await
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn changeset_constraints(pool: PgPool) -> sqlx::Result<()> {
    let author = person(&pool).await?;
    assert!(changeset(&pool, author, "applied").await.is_ok());
    assert!(changeset(&pool, author, "pending").await.is_ok());
    assert!(
        changeset(&pool, author, "lost").await.is_err(),
        "unknown state"
    );
    // A pending changeset keeps its request, to be replayed (REQ-004-22).
    let without_request = sqlx::query(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period, expires_at)
         VALUES (gen_random_uuid(), $1, 'test', 'pending', 'approval', tstzrange(now(), NULL),
                 now() + interval '30 days')",
    )
    .bind(author)
    .execute(&pool)
    .await;
    assert!(without_request.is_err());
    let empty_motive = sqlx::query(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
         VALUES (gen_random_uuid(), $1, '', 'applied', 'direct', tstzrange(now(), NULL))",
    )
    .bind(author)
    .execute(&pool)
    .await;
    assert!(empty_motive.is_err(), "REQ-004-1: motive required");
    let id = changeset(&pool, author, "pending").await?;
    let self_validated = sqlx::query("UPDATE changeset SET validator_id = author_id WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;
    assert!(
        self_validated.is_err(),
        "REQ-004-21: the author never validates"
    );
    let rejected_without_reason =
        sqlx::query("UPDATE changeset SET state = 'rejected' WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await;
    assert!(
        rejected_without_reason.is_err(),
        "REQ-004-23: rejection carries a motive"
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn idempotency_keys_are_unique_per_identity(pool: PgPool) -> sqlx::Result<()> {
    let author = person(&pool).await?;
    let cs = changeset(&pool, author, "applied").await?;
    let insert =
        "INSERT INTO idempotency_key VALUES ($1, 'k1', '\\x01', $2, now() + interval '24 hours')";
    sqlx::query(insert)
        .bind(author)
        .bind(cs)
        .execute(&pool)
        .await?;
    assert!(
        sqlx::query(insert)
            .bind(author)
            .bind(cs)
            .execute(&pool)
            .await
            .is_err()
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn outbox_records_the_writing_transaction(pool: PgPool) -> sqlx::Result<()> {
    let author = person(&pool).await?;
    let cs = changeset(&pool, author, "applied").await?;
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO outbox (changeset_id, payload) VALUES ($1, '{}'), ($1, '{}')")
        .bind(cs)
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query(
        "SELECT count(DISTINCT xid) AS xids, bool_and(xid = pg_current_xact_id()) AS this_tx,
                max(seq) > min(seq) AS ordered
         FROM outbox",
    )
    .fetch_one(&mut *tx)
    .await?;
    assert_eq!(row.get::<i64, _>("xids"), 1);
    assert!(row.get::<bool, _>("this_tx"));
    assert!(row.get::<bool, _>("ordered"));
    tx.commit().await?;
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_application_role_never_deletes_audit(pool: PgPool) -> sqlx::Result<()> {
    let author = person(&pool).await?;
    let cs = changeset(&pool, author, "applied").await?;
    let mut conn = pool.acquire().await?;
    sqlx::raw_sql("SET ROLE kartenn_app")
        .execute(&mut *conn)
        .await?;
    for forbidden in [
        "DELETE FROM changeset",
        "DELETE FROM outbox",
        "UPDATE changeset SET motive = 'rewritten'",
    ] {
        let err = sqlx::raw_sql(forbidden)
            .execute(&mut *conn)
            .await
            .expect_err(forbidden);
        assert!(
            err.to_string().contains("permission denied"),
            "{forbidden}: {err}"
        );
    }
    sqlx::query("UPDATE changeset SET state = 'refused' WHERE id = $1")
        .bind(cs)
        .execute(&mut *conn)
        .await?;
    Ok(())
}
