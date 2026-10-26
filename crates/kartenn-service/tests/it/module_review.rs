//! Proposals a module requires a review of: forced even for an author who may write, validated
//! only with the review permission, with operations of review joined to the validation
//! (spec 016, REQ-016-10, 11; task 016-11).

use kartenn_core::rules;
use kartenn_service::audit::RequestContext;
use kartenn_service::authz::{Grants, Unrestricted};
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
        approval_self: true,
    }
}

/// An author who edits things and applies their own changesets (spec 024).
fn editor(author: Uuid) -> Grants {
    Grants::new()
        .everywhere(author, "dummy.thing.edit")
        .everywhere(author, "dummy.thing.approval.self")
}

/// The author's edit, still held when validated, and what `validator` holds: the
/// application of the things and, when `review`, their review (spec 024, REQ-024-11).
fn applier(author: Uuid, validator: Uuid, review: bool) -> Grants {
    let grants = Grants::new()
        .everywhere(author, "dummy.thing.edit")
        .everywhere(validator, "dummy.thing.apply");
    if review {
        grants.everywhere(validator, "dummy.thing.review")
    } else {
        grants
    }
}

/// An author who may write things, and a forced proposal of theirs: a thing whose label
/// asks for a review (`review-…`, see the module of test).
async fn forced(pool: &PgPool, store: &PgStore) -> (Uuid, Uuid) {
    let author = person(pool).await;
    let writer = editor(author);
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
    validate_as(
        store,
        grants,
        id,
        validator,
        &RequestContext::person(validator),
        operations,
    )
    .await
}

async fn validate_as(
    store: &PgStore,
    grants: &Grants,
    id: Uuid,
    validator: Uuid,
    context: &RequestContext,
    operations: Vec<Operation>,
) -> Result<Outcome, RequestError> {
    validate_pending_with(
        store,
        grants,
        &Sources::none(),
        id,
        &validator.into(),
        context,
        operations,
        MAX_OPERATIONS,
    )
    .await
}

/// The `authz.access.denied` events of a changeset, by rule.
async fn denials(pool: &PgPool, changeset: Uuid) -> Vec<Option<String>> {
    sqlx::query_scalar(
        "SELECT reason FROM audit.event
         WHERE name = 'authz.access.denied' AND changeset_id = $1 ORDER BY seq",
    )
    .bind(changeset)
    .fetch_all(pool)
    .await
    .unwrap()
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
    let writer = editor(author);
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

// REQ-016-11
#[kartenn_testing::pg_test]
async fn validation_needs_the_review_permission(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let (author, id) = forced(&pool, &store).await;
    let validator = person(&pool).await;
    // The apply permission alone: refused, the proposal stays pending.
    let writer = applier(author, validator, false);
    let refused = validate(&store, &writer, id, validator, Vec::new()).await;
    assert_eq!(rule(&refused), rules::APPROVAL_FORBIDDEN.id);
    // The author, even with approval.self and the review permission: refused.
    let author_reviews = editor(author).everywhere(author, "dummy.thing.review");
    let own = validate(&store, &author_reviews, id, author, Vec::new()).await;
    assert_eq!(rule(&own), rules::APPROVAL_SELF.id);
    // Apply and review: applied.
    let reviewer = applier(author, validator, true);
    let applied = validate(&store, &reviewer, id, validator, Vec::new()).await;
    assert!(
        matches!(applied, Ok(Outcome::Applied { .. })),
        "{applied:?}"
    );
    assert_eq!(labels(&pool).await, ["review-me"]);
}

// REQ-016-11, INV-61
#[kartenn_testing::pg_test]
async fn review_operations_apply_with_the_proposal(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let (author, id) = forced(&pool, &store).await;
    let validator = person(&pool).await;
    let reviewer = applier(author, validator, true);
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
    // The attempt is recorded before the refusal (INV-61).
    assert_eq!(
        denials(&pool, id).await,
        [Some(rules::REVIEW_OPERATION.id.to_owned())]
    );
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

// REQ-016-11, INV-61
#[kartenn_testing::pg_test]
async fn without_the_review_permission_joined_operations_are_refused_too(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let (author, id) = forced(&pool, &store).await;
    let validator = person(&pool).await;
    let writer = applier(author, validator, false);
    let refused = validate(
        &store,
        &writer,
        id,
        validator,
        vec![dummy::update_thing(
            json!({"id": Ref::Local(1), "label": "reviewed"}),
        )],
    )
    .await;
    assert_eq!(rule(&refused), rules::APPROVAL_FORBIDDEN.id);
    assert!(labels(&pool).await.is_empty());
}

// REQ-016-11, INV-61
#[kartenn_testing::pg_test]
async fn review_operations_join_a_proposal_only(pool: PgPool) {
    let store = dummy::store(&pool).await;
    // Every changeset now waits for an approval.
    let admin = person(&pool).await;
    let policy = submit(
        &store,
        &Unrestricted,
        &request(
            admin,
            vec![Operation::SetApprovalPolicy {
                name: "all".into(),
                min_operations: Some(1),
                min_diff_objects: None,
                property_keys: Vec::new(),
                domains: Vec::new(),
                scopes: Vec::new(),
                author_roles: Vec::new(),
                author_groups: Vec::new(),
                bound_roles: Vec::new(),
                module_triggers: Vec::new(),
                enabled: true,
            }],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(policy, Outcome::Applied { .. }), "{policy:?}");
    let author = person(&pool).await;
    let writer = editor(author);
    let held = submit(
        &store,
        &writer,
        &request(
            author,
            vec![dummy::create_thing(json!({"label": "plain", "local": 1}))],
        ),
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let Outcome::Pending {
        changeset,
        kind: PendingKind::Approval,
        ..
    } = held
    else {
        panic!("expected an approval, got {held:?}");
    };
    // An operation of review joined to an approval: refused, recorded.
    let approver = person(&pool).await;
    let refused = validate(
        &store,
        &Grants::new().everywhere(approver, "*"),
        changeset,
        approver,
        vec![dummy::update_thing(
            json!({"id": Ref::Local(1), "label": "reviewed"}),
        )],
    )
    .await;
    match refused {
        Ok(Outcome::Refused { violations, .. }) => {
            assert_eq!(violations[0].rule.id, rules::REVIEW_OPERATION.id);
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(
        denials(&pool, changeset).await,
        [Some(rules::REVIEW_OPERATION.id.to_owned())]
    );
}

// REQ-016-13
#[kartenn_testing::pg_test]
async fn a_validation_for_a_requester_names_it_in_its_events(pool: PgPool) {
    let store = dummy::store(&pool).await;
    let (author, id) = forced(&pool, &store).await;
    let validator = person(&pool).await;
    let requester = person(&pool).await;
    let reviewer = applier(author, validator, true);
    let mut context = RequestContext::person(validator);
    context.on_behalf_of = Some(requester);
    let applied = validate_as(&store, &reviewer, id, validator, &context, Vec::new()).await;
    assert!(
        matches!(applied, Ok(Outcome::Applied { .. })),
        "{applied:?}"
    );
    let named: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT details->>'on_behalf_of' FROM audit.event
         WHERE changeset_id = $1 AND actor_id = $2",
    )
    .bind(id)
    .bind(validator)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!named.is_empty());
    assert!(
        named
            .iter()
            .all(|n| n.as_deref() == Some(&requester.to_string())),
        "{named:?}"
    );
}
