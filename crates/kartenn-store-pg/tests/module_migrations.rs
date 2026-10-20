//! Migrations of the business modules (spec 008, REQ-008-2; task 008-2).

use std::sync::Arc;

use kartenn_service::module::{Module, ModuleMigration, ModuleRegistry};
use kartenn_store_pg::modules::{ModuleMigrationError, migrate};
use sqlx::PgPool;

#[derive(Debug)]
struct Shelf(&'static [ModuleMigration]);

impl Module for Shelf {
    fn name(&self) -> &'static str {
        "shelf"
    }
    fn version(&self) -> &'static str {
        "0.0.0"
    }
    fn migrations(&self) -> &'static [ModuleMigration] {
        self.0
    }
}

static FIRST: &[ModuleMigration] = &[
    ModuleMigration {
        version: 1,
        name: "book",
        sql: "CREATE TABLE book (id uuid PRIMARY KEY, title text NOT NULL);",
    },
    ModuleMigration {
        version: 2,
        name: "author",
        sql: "ALTER TABLE book ADD COLUMN author text; INSERT INTO book VALUES (gen_random_uuid(), 'one', 'a');",
    },
];

static CHANGED: &[ModuleMigration] = &[
    ModuleMigration {
        version: 1,
        name: "book",
        sql: "CREATE TABLE book (id uuid PRIMARY KEY, title text);",
    },
    ModuleMigration {
        version: 2,
        name: "author",
        sql: "ALTER TABLE book ADD COLUMN author text; INSERT INTO book VALUES (gen_random_uuid(), 'one', 'a');",
    },
];

fn registry(migrations: &'static [ModuleMigration]) -> ModuleRegistry {
    ModuleRegistry::new(vec![Arc::new(Shelf(migrations)) as Arc<dyn Module>]).unwrap()
}

async fn books(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM shelf.book")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn module_migrations_apply_once(pool: PgPool) {
    migrate(&pool, &registry(FIRST)).await.unwrap();
    migrate(&pool, &registry(FIRST)).await.unwrap();
    // The table is in the schema of the module, not in the core's.
    assert_eq!(books(&pool).await, 1);
    let tracked: Vec<(i64, String)> = sqlx::query_as(
        "SELECT version, name FROM module_migration WHERE module = 'shelf' ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(tracked, [(1, "book".into()), (2, "author".into())]);
    let in_public: bool = sqlx::query_scalar("SELECT to_regclass('public.book') IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!in_public);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_changed_migration_stops_the_job(pool: PgPool) {
    migrate(&pool, &registry(FIRST)).await.unwrap();
    let error = migrate(&pool, &registry(CHANGED)).await.unwrap_err();
    assert!(
        matches!(&error, ModuleMigrationError::Changed { module, version: 1, .. } if module == "shelf"),
        "{error}"
    );
    let message = error.to_string();
    assert!(
        message.contains("shelf") && message.contains('1'),
        "{message}"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_build_without_the_module_leaves_its_schema(pool: PgPool) {
    migrate(&pool, &registry(FIRST)).await.unwrap();
    migrate(&pool, ModuleRegistry::empty()).await.unwrap();
    assert_eq!(books(&pool).await, 1);
    let tracked: i64 =
        sqlx::query_scalar("SELECT count(*) FROM module_migration WHERE module = 'shelf'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(tracked, 2);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn module_object_is_versioned(pool: PgPool) {
    let history: bool =
        sqlx::query_scalar("SELECT to_regclass('public.module_object_history') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(history);
}
