//! System time and history (REQ-004-16, INV-11, INV-12; ADR 0017).

use sqlx::{PgPool, Row};

async fn demo_table(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(
        "CREATE TABLE demo (
            id uuid NOT NULL,
            label text NOT NULL,
            valid_period tstzrange NOT NULL,
            sys_start timestamptz NOT NULL DEFAULT now(),
            changeset_id uuid NOT NULL,
            PRIMARY KEY (id, valid_period WITHOUT OVERLAPS)
        );
        CALL kartenn_make_versioned('demo');
        CALL kartenn_history_partitions('demo', date '2026-01-01', 3);",
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn one_transaction_one_system_time(pool: PgPool) -> sqlx::Result<()> {
    demo_table(&pool).await?;
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(
        "INSERT INTO demo (id, label, valid_period, changeset_id)
         SELECT gen_random_uuid(), 'x' || g, tstzrange(now(), NULL), gen_random_uuid() FROM generate_series(1, 3) g;
         UPDATE demo SET label = label || '!' WHERE label = 'x1';",
    )
    .execute(&mut *tx)
    .await?;
    let row = sqlx::query(
        "SELECT (SELECT count(DISTINCT sys_start) FROM demo) AS starts,
                (SELECT bool_and(sys_start = now()) FROM demo) AS at_tx_time,
                (SELECT count(*) FROM demo_history WHERE sys_end = now()) AS closed_now",
    )
    .fetch_one(&mut *tx)
    .await?;
    assert_eq!(
        row.get::<i64, _>("starts"),
        1,
        "one system time per transaction"
    );
    assert!(row.get::<bool, _>("at_tx_time"));
    assert_eq!(
        row.get::<i64, _>("closed_now"),
        1,
        "the updated row was historised now"
    );
    tx.commit().await?;
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn manual_sql_cannot_bypass_history(pool: PgPool) -> sqlx::Result<()> {
    demo_table(&pool).await?;
    sqlx::raw_sql(
        "INSERT INTO demo (id, label, valid_period, changeset_id, sys_start)
         VALUES (gen_random_uuid(), 'a', tstzrange('2026-01-01', NULL), gen_random_uuid(), '1999-01-01');
         UPDATE demo SET label = 'b';
         DELETE FROM demo;",
    )
    .execute(&pool)
    .await?;
    let history: i64 = sqlx::query_scalar("SELECT count(*) FROM demo_history")
        .fetch_one(&pool)
        .await?;
    assert_eq!(history, 2, "update and delete both historised");
    let forged: i64 =
        sqlx::query_scalar("SELECT count(*) FROM demo_history WHERE sys_start = '1999-01-01'")
            .fetch_one(&pool)
            .await?;
    assert_eq!(forged, 0, "a client-supplied sys_start is overridden");
    let contiguous: bool = sqlx::query_scalar(
        "SELECT (SELECT sys_end FROM demo_history WHERE label = 'a') = (SELECT sys_start FROM demo_history WHERE label = 'b')",
    )
    .fetch_one(&pool)
    .await?;
    assert!(contiguous, "history versions are contiguous");
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn application_role_cannot_write_history(pool: PgPool) -> sqlx::Result<()> {
    demo_table(&pool).await?;
    let mut conn = pool.acquire().await?;
    sqlx::raw_sql("SET ROLE kartenn_app")
        .execute(&mut *conn)
        .await?;
    sqlx::raw_sql(
        "INSERT INTO demo (id, label, valid_period, changeset_id) VALUES (gen_random_uuid(), 'a', tstzrange(now(), NULL), gen_random_uuid());
         UPDATE demo SET label = 'b';",
    )
    .execute(&mut *conn)
    .await?;
    let historised: i64 = sqlx::query_scalar("SELECT count(*) FROM demo_history")
        .fetch_one(&mut *conn)
        .await?;
    assert_eq!(
        historised, 1,
        "the trigger writes history on behalf of the application role"
    );
    for forbidden in [
        "INSERT INTO demo_history SELECT *, now() FROM demo",
        "UPDATE demo_history SET label = 'forged'",
        "DELETE FROM demo_history",
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
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn tables_without_versioning_columns_are_refused(pool: PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql("CREATE TABLE plain (id uuid)")
        .execute(&pool)
        .await?;
    let err = sqlx::raw_sql("CALL kartenn_make_versioned('plain')")
        .execute(&pool)
        .await
        .expect_err("refused");
    assert!(
        err.to_string().contains("lacks the versioning columns"),
        "{err}"
    );
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_added_column_is_historised_by_name(pool: PgPool) -> sqlx::Result<()> {
    demo_table(&pool).await?;
    sqlx::raw_sql(
        "INSERT INTO demo (id, label, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'old', tstzrange(now(), NULL), gen_random_uuid());
         CALL kartenn_add_versioned_column('demo', 'flag boolean NOT NULL DEFAULT false');
         UPDATE demo SET flag = true, label = 'new';
         DELETE FROM demo;",
    )
    .execute(&pool)
    .await?;
    let rows: Vec<(String, bool)> =
        sqlx::query_as("SELECT label, flag FROM demo_history ORDER BY label")
            .fetch_all(&pool)
            .await?;
    assert_eq!(rows, vec![("new".into(), true), ("old".into(), false)]);
    Ok(())
}
