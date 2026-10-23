//! Labels of the policy (spec 018, REQ-018-8, 13, Q-018-7; task 018-6): the labels kept
//! until now become labels of the default language, and those of the values of the axes
//! live in the core.

use std::sync::Arc;

use kartenn_policy::module::Policy;
use kartenn_service::module::{Module, ModuleMigration, ModuleRegistry};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Connection, PgConnection, PgPool};

/// The migrations of the policy, alone: up to `labels` excluded, or all of them.
#[derive(Debug)]
struct Migrations(bool);

impl Module for Migrations {
    fn name(&self) -> &'static str {
        "policy"
    }
    fn version(&self) -> &'static str {
        "0.0.0"
    }
    fn migrations(&self) -> &'static [ModuleMigration] {
        let all = Policy.migrations();
        if self.0 {
            return all;
        }
        let at = all.iter().position(|m| m.name == "labels").unwrap();
        &all[..at]
    }
}

fn core_version(name: &str) -> i64 {
    kartenn_store_pg::MIGRATOR
        .iter()
        .find(|m| m.description == name)
        .map(|m| m.version)
        .expect("a migration of the core")
}

#[kartenn_testing::pg_test]
async fn environment_labels_live_in_the_core(_pool: PgPool) {
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
        .run_to(core_version("labels") - 1, &pool)
        .await
        .unwrap();
    let before = ModuleRegistry::new(vec![Arc::new(Migrations(false)) as Arc<dyn Module>]).unwrap();
    kartenn_store_pg::modules::migrate(&pool, &before)
        .await
        .unwrap();
    sqlx::raw_sql(
        "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers,
                                           valid_period, changeset_id)
         VALUES ('00000000-0000-7000-8000-0000000000e1', 'policy.environment', 'enum', 'single',
                 '{entity}', tstzrange(now() - interval '1 day', NULL), gen_random_uuid());
         INSERT INTO property_enum_value (id, property_id, key, valid_period, changeset_id)
         VALUES (gen_random_uuid(), '00000000-0000-7000-8000-0000000000e1', 'prod',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid()),
                (gen_random_uuid(), '00000000-0000-7000-8000-0000000000e1', 'dev',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid());
         INSERT INTO policy.policy_value (id, property_key, value_key, label, valid_period,
                                          changeset_id)
         VALUES (gen_random_uuid(), 'policy.environment', 'prod', 'Production',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid());
         INSERT INTO policy.policy_flow_type (id, key, label, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'ssh', 'Administration à distance',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid());",
    )
    .execute(&pool)
    .await
    .unwrap();
    kartenn_store_pg::MIGRATOR.run(&pool).await.unwrap();
    let after = ModuleRegistry::new(vec![Arc::new(Migrations(true)) as Arc<dyn Module>]).unwrap();
    kartenn_store_pg::modules::migrate(&pool, &after)
        .await
        .unwrap();
    let labels: Vec<(String, Value)> =
        sqlx::query_as("SELECT key, labels FROM property_enum_value ORDER BY key")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        labels,
        [
            ("dev".to_owned(), json!({})),
            ("prod".to_owned(), json!({"fr": "Production"}))
        ]
    );
    let flow_type: Value =
        sqlx::query_scalar("SELECT labels FROM policy.policy_flow_type WHERE key = 'ssh'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(flow_type, json!({"fr": "Administration à distance"}));
    pool.close().await;
    kartenn_testing::drop_database(&mut admin, &name)
        .await
        .unwrap();
}
