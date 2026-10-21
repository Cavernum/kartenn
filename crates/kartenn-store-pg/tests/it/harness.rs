//! Integration test harness: every `#[kartenn_testing::pg_test]` runs against a throwaway
//! database cloned from a template migrated with [`kartenn_store_pg::MIGRATOR`] (ADR 0028).

use sqlx::PgPool;

#[kartenn_testing::pg_test]
async fn the_application_reads_the_applied_migrations_and_nothing_more(
    pool: PgPool,
) -> sqlx::Result<()> {
    let mut conn = pool.acquire().await?;
    sqlx::query("SET ROLE kartenn_app")
        .execute(&mut *conn)
        .await?;
    // The readiness query of /health/ready (REQ-015-11).
    let latest: Option<i64> =
        sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&mut *conn)
            .await?;
    assert!(
        latest.is_some(),
        "the instance role reads the applied migrations"
    );
    // INV-11: reading only.
    for write in [
        "DELETE FROM _sqlx_migrations",
        "UPDATE _sqlx_migrations SET success = false",
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (999999, 'forged', true, '\\x00', 0)",
    ] {
        assert!(
            sqlx::query(write).execute(&mut *conn).await.is_err(),
            "the instance role must not write migrations: {write}"
        );
    }
    Ok(())
}

#[kartenn_testing::pg_test]
async fn throwaway_database_is_migrated(pool: PgPool) -> sqlx::Result<()> {
    let installed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'btree_gist')",
    )
    .fetch_one(&pool)
    .await?;
    assert!(installed, "btree_gist must be installed by the migrations");
    Ok(())
}

#[kartenn_testing::pg_test]
async fn application_role_has_no_administration_rights(pool: PgPool) -> sqlx::Result<()> {
    let mut conn = pool.acquire().await?;
    sqlx::query("SET ROLE kartenn_app")
        .execute(&mut *conn)
        .await?;
    let denied = sqlx::query("CREATE ROLE kartenn_test_forbidden")
        .execute(&mut *conn)
        .await;
    assert!(
        denied.is_err(),
        "the application role must not create roles"
    );
    Ok(())
}
