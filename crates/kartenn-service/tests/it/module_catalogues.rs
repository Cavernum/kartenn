//! The catalogues of permissions and rules, merged with those of the modules of the build
//! (spec 008, REQ-008-8; task 008-6). Each test runs in its own process (nextest): the
//! catalogue of a test without the module never saw it.

use crate::support::kit_module as kit;

use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::rbac::sync_roles;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit};
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;

async fn set_role(pool: &PgPool, store: &PgStore, entries: &[&str]) -> Outcome {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let request = ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations: vec![Operation::SetRole {
            key: "kit_reader".into(),
            label: "Kit reader".into(),
            entries: entries.iter().map(|e| (*e).to_owned()).collect(),
        }],
        simulate: false,
        idempotency_key: None,
    };
    submit(store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap()
}

async fn entries(pool: &PgPool, role: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT p.permission FROM role r JOIN role_permission p ON p.role_id = r.id
         WHERE r.key = $1 AND upper_inf(p.valid_period) ORDER BY 1",
    )
    .bind(role)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[kartenn_testing::pg_test]
async fn a_role_with_a_module_permission(pool: PgPool) {
    let store = kit::store(&pool);
    let outcome = set_role(&pool, &store, &["kit.group.read", "kit.item.read"]).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert_eq!(
        entries(&pool, "kit_reader").await,
        ["kit.group.read", "kit.item.read"]
    );
    // The delivered roles that read read the objects of the module; the module's rules
    // read back from the journal.
    let roles = sync_roles(&store, MAX_OPERATIONS).await.unwrap();
    assert!(matches!(roles, Some(Outcome::Applied { .. })), "{roles:?}");
    let reader = entries(&pool, "reader").await;
    assert!(reader.contains(&"kit.group.read".to_owned()), "{reader:?}");
    assert!(reader.contains(&"kit.item.read".to_owned()), "{reader:?}");
    assert!(
        !entries(&pool, "editor")
            .await
            .contains(&"kit.group.write".to_owned())
    );
    assert!(kartenn_core::rules::rule("kit.object.invalid").is_some());
    assert!(kartenn_core::authz::permission("kit.group.write").is_some());
}

#[kartenn_testing::pg_test]
async fn unknown_module_permission_refused(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let outcome = set_role(&pool, &store, &["kit.group.read"]).await;
    match outcome {
        Outcome::Refused { violations, .. } => {
            let rules: Vec<_> = violations.iter().map(|v| v.rule.id).collect();
            assert_eq!(rules, ["core.role.invalid"]);
        }
        other => panic!("not refused: {other:?}"),
    }
    assert!(kartenn_core::rules::rule("kit.object.invalid").is_none());
}
