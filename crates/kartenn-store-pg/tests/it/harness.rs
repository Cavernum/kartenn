//! Integration test harness: every `#[sqlx::test]` runs against a throwaway database
//! migrated with [`kartenn_store_pg::MIGRATOR`].

use sqlx::PgPool;

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn throwaway_database_is_migrated(pool: PgPool) -> sqlx::Result<()> {
    let installed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'btree_gist')",
    )
    .fetch_one(&pool)
    .await?;
    assert!(installed, "btree_gist must be installed by the migrations");
    Ok(())
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
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
