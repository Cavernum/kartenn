//! The rights of the policy (spec 010, S15 amended by Q17, REQ-010-24): one read for every
//! object of the policy, one administration by family, and the environments under
//! `policy.environment.admin` besides the write of the values.

use kartenn_service::authz::Grants;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{Operation, Outcome, submit};
use serde_json::json;
use sqlx::PgPool;

use crate::support::env::{Env, messages, op, rules};

/// `operations` submitted by the author of `env` under `grants`.
async fn submit_with(env: &Env, grants: &Grants, operations: Vec<Operation>) -> Outcome {
    submit(
        &env.store,
        grants,
        &env.request(operations, None),
        MAX_OPERATIONS,
    )
    .await
    .unwrap()
}

#[kartenn_testing::pg_test]
async fn the_catalogue_is_that_of_s15(pool: PgPool) {
    let _env = Env::new(&pool).await;
    for name in [
        "policy.read",
        "policy.verdict.read",
        "policy.flow.request",
        "policy.axis.admin",
        "policy.environment.admin",
        "policy.flow_type.admin",
        "policy.service.admin",
        "policy.rule.admin",
        "policy.inference.admin",
    ] {
        assert!(kartenn_core::authz::permission(name).is_some(), "{name}");
    }
    assert!(kartenn_core::authz::permission("policy.axis.write").is_none());
    assert!(kartenn_core::authz::permission("policy.matrix.admin").is_none());
    // Every object of the policy is read under one permission, written under its family's.
    assert_eq!(
        kartenn_service::reads::read_permission("policy.signal"),
        "policy.read"
    );
    assert_eq!(
        kartenn_core::authz::type_write_permission("policy.condition"),
        "policy.rule.admin"
    );
    assert_eq!(
        kartenn_core::authz::type_write_permission("policy.value"),
        "policy.axis.admin"
    );
    // A type without declared permissions keeps the convention.
    assert_eq!(
        kartenn_core::authz::type_write_permission("vlan.vlan"),
        "vlan.vlan.write"
    );
}

#[kartenn_testing::pg_test]
async fn a_family_is_administered_under_its_own_permission(pool: PgPool) {
    let env = Env::new(&pool).await;
    let flow_type = || {
        vec![op(
            "flow_type.create",
            json!({ "key": "adm", "label": "Adm" }),
        )]
    };
    let rules_only = Grants::new().everywhere(env.author, "policy.rule.admin");
    let refused = submit_with(&env, &rules_only, flow_type()).await;
    assert_eq!(rules(&refused), ["core.object.forbidden"]);
    assert!(
        messages(&refused).contains("policy.flow_type.admin"),
        "{}",
        messages(&refused)
    );
    let admin = Grants::new().everywhere(env.author, "policy.flow_type.admin");
    let applied = submit_with(&env, &admin, flow_type()).await;
    assert!(matches!(applied, Outcome::Applied { .. }), "{applied:?}");
}

#[kartenn_testing::pg_test]
async fn an_environment_needs_the_environment_administration(pool: PgPool) {
    let env = Env::new(&pool).await;
    let create = || {
        vec![op(
            "environment.create",
            json!({ "key": "recette", "label": "Recette" }),
        )]
    };
    // The values of the axes, and the value of the closed list the core adds, without the
    // administration of the environments: refused.
    let without = Grants::new()
        .everywhere(env.author, "policy.axis.admin")
        .everywhere(env.author, "core.property.admin");
    let refused = submit_with(&env, &without, create()).await;
    assert_eq!(rules(&refused), ["core.object.forbidden"]);
    assert!(
        messages(&refused).contains("policy.environment.admin"),
        "{}",
        messages(&refused)
    );
    let with = without.everywhere(env.author, "policy.environment.admin");
    let applied = submit_with(&env, &with, create()).await;
    assert!(matches!(applied, Outcome::Applied { .. }), "{applied:?}");
}
