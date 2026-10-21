//! Proposals a module requires a review of: forced even for an author who may write, validated
//! only with the review permission, with operations of review joined to the validation
//! (spec 016, REQ-016-10, 11; task 016-11).

use kartenn_core::rules;
use kartenn_service::authz::Grants;
use kartenn_service::expected::Sources;
use kartenn_service::pipeline::{MAX_OPERATIONS, PendingKind};
use kartenn_service::review::validate_pending_with;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, RequestError, submit};
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
    }
}

/// An author who may write things, and a forced proposal of theirs: a thing whose label
/// asks for a review (`review-…`, see the module of test).
async fn forced(pool: &PgPool, store: &PgStore) -> (Uuid, Uuid) {
    let author = person(pool).await;
    let writer = Grants::new().everywhere(author, "dummy.thing.write");
    let outcome = submit(
        store,
        &writer,
        &request(
            author,
            vec![dummy::create_thing(
                json!({"label": "review-me", "local": 1}),
            )],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    match outcome {
        Outcome::Pending {
            changeset,
            kind: PendingKind::Proposal,
            ..
        } => (author, changeset),
        other => panic!("expected a forced proposal, got {other:?}"),
    }
}

async fn validate(
    store: &PgStore,
    grants: &Grants,
    id: Uuid,
    validator: Uuid,
    operations: Vec<Operation>,
) -> Result<Outcome, RequestError> {
    validate_pending_with(
        store,
        grants,
        &Sources::none(),
        id,
        &validator.into(),
        operations,
        MAX_OPERATIONS,
    )
    .await
}

async fn labels(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT label FROM dummy.thing ORDER BY label")
        .fetch_all(pool)
        .await
        .unwrap()
}

fn rule(result: &Result<Outcome, RequestError>) -> &'static str {
    match result {
        Err(RequestError::Review(v)) => v.rule.id,
        other => panic!("expected a refused review, got {other:?}"),
    }
}

#[kartenn_testing::pg_test]
async fn a_reviewed_operation_makes_a_proposal_despite_write(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let (_, id) = forced(&pool, &store).await;
    // Nothing written, the review permission recorded among the rights it requires.
    assert!(labels(&pool).await.is_empty());
    let required: serde_json::Value =
        sqlx::query_scalar("SELECT required_rights FROM changeset WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        required.to_string().contains("dummy.thing.review"),
        "{required}"
    );
    // A thing that asks for no review is written directly.
    let author = person(&pool).await;
    let writer = Grants::new().everywhere(author, "dummy.thing.write");
    let direct = submit(
        &store,
        &writer,
        &request(author, vec![dummy::create_thing(json!({"label": "plain"}))]),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(direct, Outcome::Applied { .. }), "{direct:?}");
}

#[kartenn_testing::pg_test]
async fn validation_needs_the_review_permission(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let (author, id) = forced(&pool, &store).await;
    let validator = person(&pool).await;
    // The write permission alone: refused, the proposal stays pending.
    let writer = Grants::new().everywhere(validator, "dummy.thing.write");
    let refused = validate(&store, &writer, id, validator, Vec::new()).await;
    assert_eq!(rule(&refused), rules::APPROVAL_FORBIDDEN.id);
    // The author, even with the review permission: refused.
    let author_reviews = Grants::new()
        .everywhere(author, "dummy.thing.write")
        .everywhere(author, "dummy.thing.review");
    let own = validate(&store, &author_reviews, id, author, Vec::new()).await;
    assert_eq!(rule(&own), rules::APPROVAL_SELF.id);
    // Write and review: applied.
    let reviewer = Grants::new()
        .everywhere(validator, "dummy.thing.write")
        .everywhere(validator, "dummy.thing.review");
    let applied = validate(&store, &reviewer, id, validator, Vec::new()).await;
    assert!(
        matches!(applied, Ok(Outcome::Applied { .. })),
        "{applied:?}"
    );
    assert_eq!(labels(&pool).await, ["review-me"]);
}

#[kartenn_testing::pg_test]
async fn review_operations_apply_with_the_proposal(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let (_, id) = forced(&pool, &store).await;
    let validator = person(&pool).await;
    let reviewer = Grants::new()
        .everywhere(validator, "dummy.thing.write")
        .everywhere(validator, "dummy.thing.review");
    // An operation that is not one of review: refused, nothing written, still pending.
    let foreign = validate(
        &store,
        &reviewer,
        id,
        validator,
        vec![dummy::create_thing(json!({"label": "smuggled"}))],
    )
    .await;
    match foreign {
        Ok(Outcome::Refused { violations, .. }) => {
            assert_eq!(violations[0].rule.id, rules::REVIEW_OPERATION.id);
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    let state: String = sqlx::query_scalar("SELECT state FROM changeset WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "pending");
    // A relabelling joined by the reviewer: one changeset, its operation included.
    let joined = validate(
        &store,
        &reviewer,
        id,
        validator,
        vec![dummy::update_thing(
            json!({"id": Ref::Local(1), "label": "reviewed"}),
        )],
    )
    .await;
    assert!(matches!(joined, Ok(Outcome::Applied { .. })), "{joined:?}");
    assert_eq!(labels(&pool).await, ["reviewed"]);
    let operations: i32 = sqlx::query_scalar(
        "SELECT jsonb_array_length(request->'operations') FROM changeset WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(operations, 2);
}
