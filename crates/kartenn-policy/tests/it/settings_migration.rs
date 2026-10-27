//! The custom roles that cite the administration of the properties or of a family of the
//! catalogue of the policy take the settings of each declarant (spec 027, REQ-027-7), by
//! the renaming table: the migration job, as for spec 018 and 024.

use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::rbac::rename_permissions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::support::env::Env;

/// A custom role `key` stored with `entries`, as an instance before spec 027 holds it.
async fn role(pool: &PgPool, key: &str, entries: &[&str]) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, $2, jsonb_build_object('fr', $2), tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(id)
    .bind(key)
    .execute(pool)
    .await
    .unwrap();
    for entry in entries {
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(id)
        .bind(entry)
        .execute(pool)
        .await
        .unwrap();
    }
    id
}

/// The entries of `role` now, sorted.
async fn entries(pool: &PgPool, role: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT permission FROM role_permission
          WHERE role_id = $1 AND valid_period @> now() ORDER BY permission",
    )
    .bind(role)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// REQ-027-7: `core.property.admin` gives the three actions on the settings of the core,
/// of the machines and of the policy (the flow module has none); the administration of a
/// family of the catalogue of the policy, those of the policy, the rules untouched (P7);
/// a second run changes nothing.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_027_7_roles_take_the_settings(pool: PgPool) {
    let env = Env::new(&pool).await;
    let properties = role(&pool, "proprietes", &["core.property.admin"]).await;
    let catalogue = role(
        &pool,
        "catalogue",
        &[
            "policy.axis.admin",
            "policy.environment.admin",
            "policy.flow_type.admin",
            "policy.service.admin",
            "policy.rule.admin",
        ],
    )
    .await;
    rename_permissions(&env.store, MAX_OPERATIONS)
        .await
        .unwrap()
        .expect("a changeset renames the roles");
    assert_eq!(
        entries(&pool, properties).await,
        [
            "core.settings.apply",
            "core.settings.approval.self",
            "core.settings.edit",
            "machines.settings.apply",
            "machines.settings.approval.self",
            "machines.settings.edit",
            "policy.settings.apply",
            "policy.settings.approval.self",
            "policy.settings.edit",
        ]
    );
    assert_eq!(
        entries(&pool, catalogue).await,
        [
            "policy.rule.admin",
            "policy.settings.apply",
            "policy.settings.approval.self",
            "policy.settings.edit",
        ]
    );
    assert!(
        rename_permissions(&env.store, MAX_OPERATIONS)
            .await
            .unwrap()
            .is_none()
    );
    // No alias: the old names are refused in a role.
    for old in ["core.property.admin", "policy.service.admin"] {
        assert!(!kartenn_core::authz::valid_role_entry(old), "{old}");
    }
}
