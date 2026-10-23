//! Labels by language (spec 018, REQ-018-8, 13; task 018-6): the labels kept until now
//! become labels of the default language, on the rows and their history.

use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Connection, PgConnection, PgPool};

/// The version of the core migration `name`.
fn version(name: &str) -> i64 {
    kartenn_store_pg::MIGRATOR
        .iter()
        .find(|m| m.description == name)
        .map(|m| m.version)
        .expect("a migration of the core")
}

#[kartenn_testing::pg_test]
async fn existing_labels_become_default_language_labels(_pool: PgPool) {
    let master = kartenn_testing::master_options();
    let mut admin = PgConnection::connect_with(&master).await.unwrap();
    let name = kartenn_testing::database_name(concat!(module_path!(), "::before"));
    kartenn_testing::create_empty(&mut admin, &name)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(master.clone().database(&name))
        .await
        .unwrap();
    kartenn_store_pg::MIGRATOR
        .run_to(version("labels") - 1, &pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO role (id, key, label, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'auditor', 'Auditeur', tstzrange(now(), NULL), gen_random_uuid())",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE role SET label = 'Auditrice'")
        .execute(&pool)
        .await
        .unwrap();
    kartenn_store_pg::MIGRATOR.run(&pool).await.unwrap();
    let current: Value = sqlx::query_scalar("SELECT labels FROM role WHERE key = 'auditor'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(current, json!({"fr": "Auditrice"}));
    let former: Value = sqlx::query_scalar("SELECT labels FROM role_history WHERE key = 'auditor'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(former, json!({"fr": "Auditeur"}));
    let versions: i64 = sqlx::query_scalar("SELECT count(*) FROM role_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(versions, 1, "labelling the rows is not a new version");
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM label_revision")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(revision, 0);
    let refused = sqlx::query("UPDATE role SET labels = '[]'")
        .execute(&pool)
        .await;
    assert!(refused.is_err(), "labels are an object");
    pool.close().await;
    kartenn_testing::drop_database(&mut admin, &name)
        .await
        .unwrap();
}
