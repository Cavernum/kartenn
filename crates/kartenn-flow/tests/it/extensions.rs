//! The extension of a flow (task 011-13; F20; REQ-011-4, 6, 22): an earlier end applies,
//! a later one (or none on a dated flow) is triaged on the portion it adds and reviewed
//! unless automatic, under the ceilings of review and waiver and the separation of duties;
//! a flow that has ended is not extended.

use kartenn_service::audit::RequestContext;
use kartenn_service::authz::{Authorizer, Grants, Unrestricted};
use kartenn_service::expected::Sources;
use kartenn_service::operations::ModuleTrigger;
use kartenn_service::pipeline::{MAX_OPERATIONS, PendingKind};
use kartenn_service::review::{reject, validate_pending_with};
use kartenn_service::{Operation, Outcome, RequestError, submit_in};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::requests::{Env, REQUEST, flow, grants, later, policy, proposal, refusals, rules};

/// What the author of an extension holds: the direct write, and the anchor it prolongs,
/// applied by itself (spec 024).
pub(crate) const WRITE: &[&str] = &[
    "flow.edit",
    "flow.approval.self",
    "prop.core.owner.edit",
    "prop.core.owner.approval.self",
];
const DAY: i64 = 24 * 3600;

/// The rule « prod to prod » allowed under reserve (a pattern, REQ-010-32): its flows are
/// reviewed.
pub(crate) async fn under_reserve(env: &Env) {
    let admin: Uuid =
        sqlx::query_scalar("SELECT id FROM policy.policy_flow_type WHERE key = 'admin'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    env.applied(vec![
        policy(
            "pattern.create",
            json!({ "local": 1, "key": "bastion", "labels": { "fr": "Bastion" },
                    "texts": { "fr": { "body": "Passer par le bastion." } } }),
        ),
        policy(
            "rule.create",
            json!({ "key": "prod-prod", "labels": { "fr": "Prod vers prod" }, "effect": "allow",
                    "flow_types": [{ "id": admin }],
                    "source": { "policy.environment": ["prod"] },
                    "destination": { "policy.environment": ["prod"] },
                    "required_patterns": [{ "local": 1 }] }),
        ),
    ])
    .await;
}

async fn validate<A: Authorizer>(
    env: &Env,
    authorizer: &A,
    changeset: Uuid,
    validator: Uuid,
    operations: Vec<Operation>,
) -> Result<Outcome, RequestError> {
    validate_pending_with(
        &env.store,
        authorizer,
        &Sources::none(),
        changeset,
        &validator.into(),
        &RequestContext::person(validator),
        operations,
        MAX_OPERATIONS,
    )
    .await
}

fn approve(flow_id: Uuid, days: i64) -> Operation {
    flow(
        "approve_review",
        json!({ "flow": { "id": flow_id }, "end": later(days) }),
    )
}

fn waive(flow_id: Uuid, days: i64) -> Operation {
    flow(
        "waive",
        json!({ "flow": { "id": flow_id }, "reason": "migration",
                "accepted_risk": "admin access", "end": later(days) }),
    )
}

/// The update of `flow_id` to end `days` from now (`None`: without end).
fn extend(flow_id: Uuid, days: Option<i64>) -> Operation {
    flow(
        "update",
        json!({ "id": { "id": flow_id }, "end": days.map(later) }),
    )
}

async fn flow_from(pool: &PgPool, source: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM flow.flow WHERE source_id = $1")
        .bind(source)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The end of the flow `id` (its last version).
async fn end_of(pool: &PgPool, id: Uuid) -> Option<OffsetDateTime> {
    sqlx::query_scalar(
        "SELECT upper(valid_period) FROM flow.flow WHERE id = $1
          ORDER BY lower(valid_period) DESC LIMIT 1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Whether `end` is `days` from now, within a minute.
fn about(end: Option<OffsetDateTime>, days: i64) -> bool {
    let wanted = OffsetDateTime::now_utc() + time::Duration::days(days);
    end.is_some_and(|e| (e - wanted).abs() < time::Duration::minutes(1))
}

/// A flow requested by `requester` from `source` to `destination`, held for review and
/// approved by `reviewer` with an end `days` from now and the operations `extra` (a
/// waiver).
#[allow(clippy::too_many_arguments)] // the parties, the endpoints and the review
pub(crate) async fn instructed<A: Authorizer>(
    env: &Env,
    authorizer: &A,
    (requester, reviewer): (Uuid, Uuid),
    (source, destination): (Uuid, Uuid),
    days: i64,
    waived: Option<i64>,
) -> Uuid {
    let id = proposal(
        &env.submit_as(
            authorizer,
            requester,
            vec![flow("request", env.fields(source, destination).await)],
        )
        .await,
    );
    let mut operations = vec![flow(
        "approve_review",
        json!({ "flow": { "local": 1 }, "end": later(days) }),
    )];
    if let Some(waived) = waived {
        operations.push(flow(
            "waive",
            json!({ "flow": { "local": 1 }, "reason": "migration",
                    "accepted_risk": "admin access", "end": later(waived) }),
        ));
    }
    let result = validate(env, authorizer, id, reviewer, operations)
        .await
        .unwrap();
    assert!(matches!(result, Outcome::Applied { .. }), "{result:?}");
    flow_from(&env.pool, source).await
}

/// The parties of a review: requester, reviewer and author of the extension.
pub(crate) async fn parties(env: &Env) -> (Uuid, Uuid, Uuid, Grants) {
    let (requester, reviewer, extender) =
        (env.person().await, env.person().await, env.person().await);
    let authorizer = WRITE
        .iter()
        .fold(grants(requester, REQUEST), |g, p| g.everywhere(extender, p));
    // The reviewer applies the proposals it validates (spec 024, REQ-024-11).
    let authorizer = [
        "flow.review",
        "flow.waiver.grant",
        "flow.request",
        "flow.edit",
        "flow.apply",
        "prop.core.owner.edit",
        "prop.core.owner.apply",
    ]
    .iter()
    .fold(authorizer, |g, p| g.everywhere(reviewer, p));
    (requester, reviewer, extender, authorizer)
}

/// REQ-011-4: advancing the end of an active flow applies, without review.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_4_an_earlier_end_applies(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let mut fields = env.fields(prod, dev).await;
    fields["end"] = json!(later(60));
    env.applied(vec![flow("request", fields)]).await;
    let id = flow_from(&pool, prod).await;
    let (_, _, extender, authorizer) = parties(&env).await;
    let outcome = env
        .submit_as(&authorizer, extender, vec![extend(id, Some(30))])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert!(about(end_of(&pool, id).await, 30));
}

/// REQ-011-4, 21: an extension whose added portion is automatic applies at once, the
/// version, the anchor `core.owner`, the object of the module and the evaluation carried
/// to the new end; removing the end of a dated flow extends it too.
#[kartenn_testing::pg_test]
async fn an_automatic_extension_applies(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let mut fields = env.fields(prod, dev).await;
    fields["end"] = json!(later(30));
    env.applied(vec![flow("request", fields)]).await;
    let id = flow_from(&pool, prod).await;
    let (_, _, extender, authorizer) = parties(&env).await;
    let outcome = env
        .submit_as(&authorizer, extender, vec![extend(id, Some(90))])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let end = end_of(&pool, id).await;
    assert!(about(end, 90));
    let (object, anchored, evaluated): (bool, bool, bool) = sqlx::query_as(
        "SELECT (SELECT range_agg(valid_period) FROM module_object WHERE id = $1)
                    @> tstzrange(now(), $2),
                (SELECT range_agg(a.valid_period) FROM property_assignment a
                   JOIN property_declaration d ON d.id = a.property_id
                  WHERE a.object_id = $1 AND d.key = 'core.owner') @> tstzrange(now(), $2),
                (SELECT range_agg(valid_period) FROM flow.flow_summary WHERE flow_id = $1)
                    @> tstzrange(now(), $2)",
    )
    .bind(id)
    .bind(end)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        object && anchored && evaluated,
        "{object} {anchored} {evaluated}"
    );

    let outcome = env
        .submit_as(&authorizer, extender, vec![extend(id, None)])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    assert_eq!(end_of(&pool, id).await, None);
}

/// REQ-011-4, 22: an extension whose added portion is allowed under reserve waits for a
/// review, the flow keeping its end; approved with an end 13 months from the decision, it
/// is refused; at 12 months, it applies.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_4_an_extension_under_reserve_is_reviewed(pool: PgPool) {
    let env = Env::new(&pool).await;
    under_reserve(&env).await;
    let a = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let b = env.address("10.3.0.0/24", "10.3.0.1", "prod").await;
    let (requester, reviewer, extender, authorizer) = parties(&env).await;
    let id = instructed(&env, &authorizer, (requester, reviewer), (a, b), 210, None).await;
    let before = end_of(&pool, id).await;

    let pending = proposal(
        &env.submit_as(&authorizer, extender, vec![extend(id, Some(400))])
            .await,
    );
    assert_eq!(end_of(&pool, id).await, before, "kept until approved");
    let result = validate(&env, &authorizer, pending, reviewer, vec![approve(id, 400)]).await;
    let found = refusals(&result);
    assert!(found.contains(&"flow.review.end_required"), "{found:?}");
    assert_eq!(end_of(&pool, id).await, before);

    let pending = proposal(
        &env.submit_as(&authorizer, extender, vec![extend(id, Some(400))])
            .await,
    );
    let result = validate(&env, &authorizer, pending, reviewer, vec![approve(id, 360)])
        .await
        .unwrap();
    assert!(matches!(result, Outcome::Applied { .. }), "{result:?}");
    let end = end_of(&pool, id).await;
    assert!(about(end, 360), "{end:?}");
    let evaluated: Option<OffsetDateTime> = sqlx::query_scalar(
        "SELECT max(upper(valid_period)) FROM flow.flow_summary WHERE flow_id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(evaluated, end, "the added portion is evaluated");
}

/// REQ-011-4, 9: an extension whose added portion the policy denies is approved only with
/// a waiver that covers it (`flow.waiver.required` otherwise).
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_4_a_denied_extension_needs_a_waiver(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let (requester, reviewer, extender, authorizer) = parties(&env).await;
    let id = instructed(
        &env,
        &authorizer,
        (requester, reviewer),
        (dev, prod),
        90,
        Some(60),
    )
    .await;

    let pending = proposal(
        &env.submit_as(&authorizer, extender, vec![extend(id, Some(150))])
            .await,
    );
    let result = validate(&env, &authorizer, pending, reviewer, vec![approve(id, 150)]).await;
    let found = refusals(&result);
    assert!(found.contains(&"flow.waiver.required"), "{found:?}");
    assert!(about(end_of(&pool, id).await, 90));

    let pending = proposal(
        &env.submit_as(&authorizer, extender, vec![extend(id, Some(150))])
            .await,
    );
    let result = validate(
        &env,
        &authorizer,
        pending,
        reviewer,
        vec![approve(id, 150), waive(id, 149)],
    )
    .await
    .unwrap();
    assert!(matches!(result, Outcome::Applied { .. }), "{result:?}");
    assert!(about(end_of(&pool, id).await, 150));
}

/// REQ-011-22: the author of an extension does not review it (`flow.separation`), even
/// when a service account submitted it on their behalf.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_4_the_author_does_not_review_the_extension(pool: PgPool) {
    let env = Env::new(&pool).await;
    under_reserve(&env).await;
    let a = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let b = env.address("10.3.0.0/24", "10.3.0.1", "prod").await;
    let (requester, reviewer, extender, authorizer) = parties(&env).await;
    let service = env.person().await;
    let authorizer = WRITE
        .iter()
        .fold(authorizer, |g, p| g.everywhere(service, p))
        .everywhere(extender, "flow.review");
    let id = instructed(&env, &authorizer, (requester, reviewer), (a, b), 90, None).await;
    let mut context = RequestContext::person(service);
    context.on_behalf_of = Some(extender);
    let pending = proposal(
        &submit_in(
            &env.store,
            &authorizer,
            &Sources::none(),
            &context,
            &env.request(service, vec![extend(id, Some(180))]),
            MAX_OPERATIONS,
        )
        .await
        .unwrap(),
    );
    let result = validate(&env, &authorizer, pending, extender, vec![approve(id, 180)]).await;
    let found = refusals(&result);
    assert!(found.contains(&"flow.separation"), "{found:?}");
    assert!(about(end_of(&pool, id).await, 90));
}

/// REQ-011-4: a flow that has ended is not extended (`flow.update.ended`).
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_4_an_ended_flow_is_not_extended(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let end: String = sqlx::query_scalar(
        "SELECT to_char((now() + interval '2 seconds') AT TIME ZONE 'UTC',
                        'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut fields = env.fields(prod, dev).await;
    fields["end"] = Value::String(end);
    env.applied(vec![flow("request", fields)]).await;
    let id = flow_from(&pool, prod).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let outcome = env
        .submit_as(&Unrestricted, env.author, vec![extend(id, Some(30))])
        .await;
    assert_eq!(rules(&outcome), ["flow.update.ended"]);
}

/// REQ-011-6: under an active policy `flow.created ≥ 1`, an automatic extension waits for
/// an approval, as a creation.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_6_a_policy_holds_an_extension(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let mut fields = env.fields(prod, dev).await;
    fields["end"] = json!(later(30));
    env.applied(vec![flow("request", fields)]).await;
    let id = flow_from(&pool, prod).await;
    env.applied(vec![Operation::SetApprovalPolicy {
        name: "flow.created".into(),
        min_operations: None,
        min_diff_objects: None,
        property_keys: Vec::new(),
        domains: Vec::new(),
        scopes: Vec::new(),
        author_roles: Vec::new(),
        author_groups: Vec::new(),
        bound_roles: Vec::new(),
        module_triggers: vec![ModuleTrigger {
            measure: "flow.created".into(),
            threshold: 1,
        }],
        enabled: true,
    }])
    .await;
    let (_, _, extender, authorizer) = parties(&env).await;
    let outcome = env
        .submit_as(&authorizer, extender, vec![extend(id, Some(60))])
        .await;
    assert!(
        matches!(
            outcome,
            Outcome::Pending {
                kind: PendingKind::Approval,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert!(about(end_of(&pool, id).await, 30));
}

/// REQ-011-22, 25 (N38): an extension rejected at review is a non-conformity of its
/// author, with its triage; the flow keeps its end.
#[kartenn_testing::pg_test]
async fn a_rejected_extension_is_a_nonconformity(pool: PgPool) {
    let env = Env::new(&pool).await;
    under_reserve(&env).await;
    let a = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let b = env.address("10.3.0.0/24", "10.3.0.1", "prod").await;
    let (requester, reviewer, extender, authorizer) = parties(&env).await;
    let id = instructed(&env, &authorizer, (requester, reviewer), (a, b), 90, None).await;
    let pending = proposal(
        &env.submit_as(&authorizer, extender, vec![extend(id, Some(180))])
            .await,
    );
    reject(
        &env.store,
        &authorizer,
        pending,
        &reviewer.into(),
        "no longer needed",
    )
    .await
    .unwrap();
    let (who, entity, outcome, request): (Uuid, Uuid, Option<String>, Value) = sqlx::query_as(
        "SELECT requester_id, entity_id, evaluation->>'outcome', request
           FROM flow.flow_rejection WHERE changeset_id = $1",
    )
    .bind(pending)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((who, entity), (extender, env.acme));
    assert_eq!(outcome.as_deref(), Some("review"));
    assert_eq!(request["id"]["id"], json!(id));
    assert!(about(end_of(&pool, id).await, 90));
    let _ = DAY;
}
