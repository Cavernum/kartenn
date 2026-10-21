//! The template engine: one template per fingerprint, built once under concurrency; the
//! database of a failed test kept.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::migrate::Migrator;
use sqlx::{AssertSqlSafe, Connection, Executor, PgConnection};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

async fn master() -> PgConnection {
    PgConnection::connect_with(&kartenn_testing::master_options())
        .await
        .unwrap()
}

/// Migrations no other test shares: a fingerprint, so a template, of their own.
fn unique_migrations(dir: &Path, extra: bool) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::fs::write(
        dir.join("0001_unique.sql"),
        format!(
            "-- {nonce} {}\nCREATE TABLE unique_t (n integer);",
            std::process::id()
        ),
    )
    .unwrap();
    if extra {
        std::fs::write(
            dir.join("0002_more.sql"),
            "CREATE TABLE more_t (n integer);",
        )
        .unwrap();
    }
}

async fn migrator(dir: &Path) -> Migrator {
    Migrator::new(dir).await.unwrap()
}

async fn databases_like(conn: &mut PgConnection, prefix: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT datname::text FROM pg_database WHERE starts_with(datname, $1)")
        .bind(prefix)
        .fetch_all(&mut *conn)
        .await
        .unwrap()
}

async fn exec(conn: &mut PgConnection, sql: String) {
    conn.execute(AssertSqlSafe(sql)).await.unwrap();
}

async fn drop_template(conn: &mut PgConnection, template: &str) {
    exec(
        conn,
        format!(r#"ALTER DATABASE "{template}" WITH IS_TEMPLATE false"#),
    )
    .await;
    kartenn_testing::drop_database(conn, template)
        .await
        .unwrap();
    sqlx::query("DELETE FROM _kartenn_test.templates WHERE name = $1")
        .bind(template)
        .execute(&mut *conn)
        .await
        .unwrap();
}

#[test]
fn eight_concurrent_builders_make_one_template() {
    let dir = tempfile::tempdir().unwrap();
    unique_migrations(dir.path(), false);
    let migrator: &'static Migrator = Box::leak(Box::new(runtime().block_on(migrator(dir.path()))));
    // Eight threads, each with its runtime and its connection, as eight test processes.
    let builders: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(move || {
                runtime().block_on(async {
                    let mut conn = master().await;
                    let template = kartenn_testing::build_template(&mut conn, migrator)
                        .await
                        .unwrap();
                    conn.close().await.unwrap();
                    template
                })
            })
        })
        .collect();
    let mut names: Vec<String> = builders.into_iter().map(|b| b.join().unwrap()).collect();
    runtime().block_on(async {
        names.dedup();
        assert_eq!(names.len(), 1, "{names:?}");
        let template = &names[0];
        let print = kartenn_testing::fingerprint(migrator);
        let mut conn = master().await;
        assert_eq!(
            databases_like(&mut conn, template).await,
            vec![template.clone()]
        );
        assert!(
            databases_like(
                &mut conn,
                &format!("{}{print}", kartenn_testing::BUILD_PREFIX)
            )
            .await
            .is_empty()
        );
        let frozen: (bool, bool) = sqlx::query_as(
            "SELECT datistemplate, datallowconn FROM pg_database WHERE datname = $1",
        )
        .bind(template)
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(frozen, (true, false));
        drop_template(&mut conn, template).await;
    });
}

#[test]
fn a_migration_added_changes_the_fingerprint() {
    let (one, two) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    unique_migrations(one.path(), false);
    std::fs::copy(
        one.path().join("0001_unique.sql"),
        two.path().join("0001_unique.sql"),
    )
    .unwrap();
    runtime().block_on(async {
        let base = migrator(one.path()).await;
        let same = migrator(one.path()).await;
        std::fs::write(
            two.path().join("0002_more.sql"),
            "CREATE TABLE more_t (n integer);",
        )
        .unwrap();
        let more = migrator(two.path()).await;
        assert_eq!(
            kartenn_testing::fingerprint(&base),
            kartenn_testing::fingerprint(&same)
        );
        assert_ne!(
            kartenn_testing::fingerprint(&base),
            kartenn_testing::fingerprint(&more)
        );
    });
}

#[test]
fn a_failed_test_keeps_its_database_a_passed_one_drops_it() {
    let dir = tempfile::tempdir().unwrap();
    unique_migrations(dir.path(), true);
    let migrator: &'static Migrator = Box::leak(Box::new(runtime().block_on(migrator(dir.path()))));
    let failed = "kartenn_testing::engine::a_failed_test";
    let passed = "kartenn_testing::engine::a_passed_test";

    let outcome: Result<(), &str> = kartenn_testing::run(migrator, failed, |pool| async move {
        sqlx::query("INSERT INTO more_t (n) VALUES (1)")
            .execute(&pool)
            .await
            .unwrap();
        Err("failed on purpose")
    });
    assert!(outcome.is_err());
    kartenn_testing::run(migrator, passed, |pool| async move {
        // A clone of the template: the row of the failed test is not there.
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM more_t")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 0);
    });

    runtime().block_on(async {
        let mut conn = master().await;
        let kept = kartenn_testing::database_name(failed);
        assert_eq!(databases_like(&mut conn, &kept).await, vec![kept.clone()]);
        let rows: i64 = {
            let mut db =
                PgConnection::connect_with(&kartenn_testing::master_options().database(&kept))
                    .await
                    .unwrap();
            let rows = sqlx::query_scalar("SELECT count(*) FROM more_t")
                .fetch_one(&mut db)
                .await
                .unwrap();
            db.close().await.unwrap();
            rows
        };
        assert_eq!(rows, 1, "the failed test's database is kept as it left it");
        assert!(
            databases_like(&mut conn, &kartenn_testing::database_name(passed))
                .await
                .is_empty()
        );
        kartenn_testing::drop_database(&mut conn, &kept)
            .await
            .unwrap();
        let template = format!(
            "{}{}",
            kartenn_testing::TEMPLATE_PREFIX,
            kartenn_testing::fingerprint(migrator)
        );
        drop_template(&mut conn, &template).await;
    });
}
