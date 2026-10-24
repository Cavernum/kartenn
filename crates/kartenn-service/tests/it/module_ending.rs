//! A pending changeset holding operations of a module that ends without being applied:
//! the module learns of it in the transaction that ends it (spec 016, REQ-016-9; task
//! 016-10).

use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::ModuleTrigger;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::review::{expire_pending, reject, withdraw};
use kartenn_service::{ChangesetRequest, Operation, Outcome, RequestError, submit};
use kartenn_store_pg::write::PgStore;
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
        apply_own: true,
    }
}

/// A store whose policy holds every changeset writing a thing, and the author of the
/// pending changesets.
async fn holding(pool: &PgPool) -> (PgStore, Uuid) {
    let store = dummy::store(pool).await;
    let admin = person(pool).await;
    let policy = Operation::SetApprovalPolicy {
        name: "things".into(),
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
            measure: "dummy.things".into(),
            threshold: 1,
        }],
        enabled: true,
    };
    let set = submit(
        &store,
        &Unrestricted,
        &request(admin, vec![policy]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(set, Outcome::Applied { .. }), "{set:?}");
    (store, person(pool).await)
}

/// A pending changeset of `author` writing two things.
async fn pending(store: &PgStore, author: Uuid, label: &str) -> Uuid {
    let outcome = submit(
        store,
        &Unrestricted,
        &request(
            author,
            vec![
                dummy::create_thing(json!({"label": format!("{label}-1")})),
                dummy::create_thing(json!({"label": format!("{label}-2")})),
            ],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    match outcome {
        Outcome::Pending { changeset, .. } => changeset,
        other => panic!("expected pending, got {other:?}"),
    }
}

/// What the module recorded for `changeset`: how, motive, decider, operations.
async fn ending(
    pool: &PgPool,
    changeset: Uuid,
) -> Vec<(String, Option<String>, Option<Uuid>, i32)> {
    sqlx::query_as(
        "SELECT how, motive, by_id, operations FROM dummy.ending WHERE changeset_id = $1",
    )
    .bind(changeset)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn state(pool: &PgPool, changeset: Uuid) -> String {
    sqlx::query_scalar("SELECT state FROM changeset WHERE id = $1")
        .bind(changeset)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn a_rejection_calls_the_module_in_its_transaction(pool: PgPool) {
    let (store, author) = holding(&pool).await;
    let id = pending(&store, author, "rejected").await;
    let validator = person(&pool).await;
    reject(&store, &Unrestricted, id, &validator.into(), "not now")
        .await
        .unwrap();
    assert_eq!(state(&pool, id).await, "rejected");
    assert_eq!(
        ending(&pool, id).await,
        [(
            "Rejected".into(),
            Some("not now".into()),
            Some(validator),
            2
        )]
    );
}

#[kartenn_testing::pg_test]
async fn a_withdrawal_and_an_expiry_call_the_module(pool: PgPool) {
    let (store, author) = holding(&pool).await;
    let withdrawn = pending(&store, author, "withdrawn").await;
    withdraw(&store, withdrawn, author).await.unwrap();
    assert_eq!(
        ending(&pool, withdrawn).await,
        [("Withdrawn".into(), None, Some(author), 2)]
    );
    let late = pending(&store, author, "late").await;
    sqlx::query("UPDATE changeset SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(late)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(expire_pending(&store).await.unwrap(), [late]);
    assert_eq!(state(&pool, late).await, "expired");
    assert_eq!(
        ending(&pool, late).await,
        [("Expired".into(), None, None, 2)]
    );
}

#[kartenn_testing::pg_test]
async fn a_failing_module_rolls_the_rejection_back(pool: PgPool) {
    let (store, author) = holding(&pool).await;
    let id = pending(&store, author, "kept").await;
    let validator = person(&pool).await;
    let refused = reject(&store, &Unrestricted, id, &validator.into(), "fail").await;
    assert!(
        matches!(refused, Err(RequestError::Store(_))),
        "{refused:?}"
    );
    // Nothing of the rejection remains: still pending, nothing recorded by the module.
    assert_eq!(state(&pool, id).await, "pending");
    assert!(ending(&pool, id).await.is_empty());
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event
         WHERE name = 'changeset.state.rejected' AND changeset_id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, 0);
}
