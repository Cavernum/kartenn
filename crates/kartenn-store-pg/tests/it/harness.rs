//! Integration test harness: every `#[kartenn_testing::pg_test]` runs against a throwaway
//! database cloned from a template migrated with [`kartenn_store_pg::MIGRATOR`] (ADR 0028).

use sqlx::PgPool;

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
