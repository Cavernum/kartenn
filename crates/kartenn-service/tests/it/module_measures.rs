//! Approval policies on the measures of modules, and the default policies a module installs
//! once (spec 016, REQ-016-7, 8).

use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::ModuleTrigger;
use kartenn_service::pipeline::{MAX_OPERATIONS, PendingKind};
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit};
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::dummy_module as dummy;

async fn person(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'tester') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

fn request(author: Uuid, operations: Vec<Operation>) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
    }
}

fn policy(name: &str, measure: &str, threshold: u64) -> Operation {
    Operation::SetApprovalPolicy {
        name: name.into(),
        min_operations: None,
        min_diff_objects: None,
        property_keys: Vec::new(),
        domains: Vec::new(),
        authors: Vec::new(),
        scopes: Vec::new(),
        author_roles: Vec::new(),
        author_groups: Vec::new(),
        bound_roles: Vec::new(),
        module_triggers: vec![ModuleTrigger {
            measure: measure.into(),
            threshold,
        }],
        enabled: true,
    }
}

#[kartenn_testing::pg_test]
async fn a_policy_on_a_module_measure_holds_the_changeset(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let admin = person(&pool).await;
    let set = submit(
        &store,
        &Unrestricted,
        &request(admin, vec![policy("things", "dummy.things", 1)]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(set, Outcome::Applied { .. }), "{set:?}");
    let author = person(&pool).await;
    // A changeset that writes a thing: measured 1, held.
    let held = submit(
        &store,
        &Unrestricted,
        &request(author, vec![dummy::create_thing(json!({"label": "held"}))]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(
        matches!(
            held,
            Outcome::Pending {
                kind: PendingKind::Approval,
                ..
            }
        ),
        "{held:?}"
    );
    // A changeset without a thing: measured 0, applied.
    let applied = submit(
        &store,
        &Unrestricted,
        &request(
            author,
            vec![Operation::CreateDomain {
                local: None,
                slug: "free".into(),
            }],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(applied, Outcome::Applied { .. }), "{applied:?}");
}

#[kartenn_testing::pg_test]
async fn an_unknown_measure_is_refused(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let admin = person(&pool).await;
    let outcome = submit(
        &store,
        &Unrestricted,
        &request(admin, vec![policy("unknown", "dummy.unknown", 1)]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(
        violations.iter().map(|v| v.rule.id).collect::<Vec<_>>(),
        ["core.approval_policy.trigger"]
    );
}

#[kartenn_testing::pg_test]
async fn a_default_policy_is_installed_once(pool: PgPool) {
    let modules = dummy::registry();
    let installed = kartenn_store_pg::pending::install_default_policies(&pool, &modules)
        .await
        .unwrap();
    assert_eq!(installed, ["dummy-many-things"]);
    let (enabled, origin, triggers): (bool, String, serde_json::Value) = sqlx::query_as(
        "SELECT enabled, origin, module_triggers FROM approval_policy WHERE name = 'dummy-many-things'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(enabled);
    assert_eq!(origin, "module");
    assert_eq!(triggers, json!([["dummy.things", 10]]));
    // Disabled by an administrator, it stays so after a new deployment.
    sqlx::query("UPDATE approval_policy SET enabled = false WHERE name = 'dummy-many-things'")
        .execute(&pool)
        .await
        .unwrap();
    let again = kartenn_store_pg::pending::install_default_policies(&pool, &modules)
        .await
        .unwrap();
    assert!(again.is_empty());
    // Removed, it is not installed again either.
    sqlx::query("DELETE FROM approval_policy WHERE name = 'dummy-many-things'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        kartenn_store_pg::pending::install_default_policies(&pool, &modules)
            .await
            .unwrap()
            .is_empty()
    );
    let enabled: Option<bool> =
        sqlx::query_scalar("SELECT enabled FROM approval_policy WHERE name = 'dummy-many-things'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(enabled, None);
}
