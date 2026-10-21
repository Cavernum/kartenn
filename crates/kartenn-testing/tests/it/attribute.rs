//! `#[pg_test]` hands the test a pool on a migrated clone.

use sqlx::PgPool;

#[kartenn_testing::pg_test(migrator = "crate::MIGRATOR")]
async fn the_pool_is_on_a_migrated_clone(pool: PgPool) {
    let sum: i64 = sqlx::query_scalar("SELECT sum(n) FROM sample")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sum, 6);
    let name: String = sqlx::query_scalar("SELECT current_database()::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        name,
        kartenn_testing::database_name(concat!(
            module_path!(),
            "::the_pool_is_on_a_migrated_clone"
        ))
    );
}

#[kartenn_testing::pg_test(migrator = "crate::MIGRATOR")]
async fn a_test_may_return_a_result(pool: PgPool) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO sample (n) VALUES (4)")
        .execute(&pool)
        .await?;
    Ok(())
}

/// The grants of the template hold on the clone: the application role reads, and may not
/// write (INV-11).
#[kartenn_testing::pg_test(migrator = "crate::MIGRATOR")]
async fn the_application_role_keeps_its_grants_on_a_clone(pool: PgPool) {
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("SET ROLE kartenn_app")
        .execute(&mut *conn)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sample")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(count, 3);
    let refused = sqlx::query("INSERT INTO sample (n) VALUES (4)")
        .execute(&mut *conn)
        .await
        .unwrap_err();
    let code = refused
        .as_database_error()
        .and_then(|e| e.code().map(std::borrow::Cow::into_owned));
    assert_eq!(code.as_deref(), Some("42501"), "{refused}");
}

#[kartenn_testing::pg_test(migrator = "crate::MIGRATOR")]
#[ignore = "ignored on purpose: the attribute keeps the other attributes"]
async fn an_ignored_test_stays_ignored(_pool: PgPool) {
    panic!("never run");
}
