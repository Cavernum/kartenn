//! Requests of flows, their triage, review, waiver and rejection (task 011-3; REQ-011-8,
//! 9, 17, 21, 22): through the write pipeline, with the modules the registry depends on.

use std::sync::Arc;

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_flow::Flows;
use kartenn_machines::Machines;
use kartenn_policy::Policy;
use kartenn_service::audit::RequestContext;
use kartenn_service::authz::{Authorizer, Grants, Unrestricted};
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::expected::Sources;
use kartenn_service::module::ModuleRegistry;
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::{MAX_OPERATIONS, PendingKind};
use kartenn_service::review::{reject, validate_pending_with};
use kartenn_service::{
    ChangesetRequest, ObjectKind, Operation, Outcome, Ref, RequestError, submit, submit_in,
};
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

/// What a requester holds on the entity (REQ-011-17): the request, and the anchor of the
/// flow it creates (`core.owner`, set by the module in the changeset), applied by itself
/// when no review is required (spec 024).
pub(crate) const REQUEST: &[&str] = &[
    "flow.request",
    "prop.core.owner.edit",
    "prop.core.owner.approval.self",
];

pub(crate) struct Env {
    pub(crate) pool: PgPool,
    pub(crate) store: PgStore,
    pub(crate) author: Uuid,
    pub(crate) acme: Uuid,
    pub(crate) domain: Uuid,
    pub(crate) registry: Arc<ModuleRegistry>,
}

pub(crate) fn policy(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("policy.{name}").into();
    serde_json::from_value(object).unwrap()
}

pub(crate) fn flow(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("flow.{name}").into();
    serde_json::from_value(object).unwrap()
}

impl Env {
    /// The modules, the entity `acme`, the domain `lab` it owns, and the policy: the
    /// environments `prod` and `dev`, the service `ssh` of the flow type `admin`, and the
    /// rule « prod to dev for admin ».
    pub(crate) async fn new(pool: &PgPool) -> Self {
        let registry = Arc::new(
            ModuleRegistry::new(vec![Arc::new(Machines), Arc::new(Policy), Arc::new(Flows)])
                .unwrap(),
        );
        kartenn_store_pg::modules::migrate(pool, &registry)
            .await
            .unwrap();
        let store = PgStore::new(pool.clone()).with_modules(registry.clone());
        let declarations: Vec<_> = std::iter::once(core_declarations_for(&registry))
            .chain(registry.modules().filter_map(|m| m.declarations()))
            .collect();
        for outcome in sync_modules(&store, &Unrestricted, &declarations, MAX_OPERATIONS)
            .await
            .unwrap()
        {
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
        let mut env = Self {
            pool: pool.clone(),
            store,
            author: Uuid::nil(),
            acme: Uuid::nil(),
            domain: Uuid::nil(),
            registry,
        };
        env.author = env.person().await;
        env.applied(vec![Operation::CreateEntity {
            local: Some(1),
            slug: "acme".into(),
            parent: None,
        }])
        .await;
        env.acme = env.slugged("entity", "acme").await;
        let owner = env.property("core.owner").await;
        env.applied(vec![
            Operation::CreateDomain {
                local: Some(2),
                slug: "lab".into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(2),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(env.acme.to_string())],
            },
        ])
        .await;
        env.domain = env.slugged("addressing_domain", "lab").await;
        // The system the external references of the flows name, by its key (REQ-016-3).
        env.applied(vec![Operation::AddEnumValue {
            local: None,
            property: Ref::Id(env.property("core.external_system").await),
            key: "ticketing".into(),
            labels: std::collections::BTreeMap::new(),
        }])
        .await;
        env.applied(vec![
            policy("environment.create", json!({ "key": "prod" })),
            policy("environment.create", json!({ "key": "dev" })),
            policy(
                "flow_type.create",
                json!({ "local": 1, "key": "admin", "labels": { "fr": "Administration" } }),
            ),
            policy(
                "service.create",
                json!({ "key": "ssh", "labels": { "fr": "SSH" }, "flow_type": { "local": 1 },
                        "entries": [{ "transport": "tcp", "ports": 22 }] }),
            ),
            policy(
                "rule.create",
                json!({ "key": "prod-dev", "labels": { "fr": "Prod vers dev" }, "effect": "allow",
                        "flow_types": [{ "local": 1 }],
                        "source": { "policy.environment": ["prod"] },
                        "destination": { "policy.environment": ["dev"] } }),
            ),
        ])
        .await;
        env
    }

    pub(crate) async fn person(&self) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    #[allow(clippy::unused_self)] // a request of the environment, as its helpers read
    pub(crate) fn request(&self, author: Uuid, operations: Vec<Operation>) -> ChangesetRequest {
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

    pub(crate) async fn submit_as<A: Authorizer>(
        &self,
        authorizer: &A,
        author: Uuid,
        operations: Vec<Operation>,
    ) -> Outcome {
        submit(
            &self.store,
            authorizer,
            &self.request(author, operations),
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
    }

    pub(crate) async fn applied(&self, operations: Vec<Operation>) -> Outcome {
        let outcome = self.submit_as(&Unrestricted, self.author, operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        outcome
    }

    pub(crate) async fn property(&self, key: &str) -> Uuid {
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = $1")
            .bind(key)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn slugged(&self, table: &str, slug: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE slug = $1 AND valid_period @> now()"
        )))
        .bind(slug)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// An address of a network of the domain classified `environment`.
    pub(crate) async fn address(&self, network: &str, address: &str, environment: &str) -> Uuid {
        let property = self.property("policy.environment").await;
        self.applied(vec![
            Operation::CreatePrefix {
                local: Some(1),
                domain: Ref::Id(self.domain),
                prefix: network.into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Prefix,
                property: Ref::Id(property),
                values: vec![RawInput::Text(environment.into())],
            },
            Operation::CreateAddress {
                local: None,
                domain: Ref::Id(self.domain),
                address: address.into(),
                usage: AddressUsage::Host,
            },
        ])
        .await;
        sqlx::query_scalar("SELECT id FROM address WHERE address = $1::inet")
            .bind(address)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn ssh(&self) -> Uuid {
        sqlx::query_scalar("SELECT id FROM policy.policy_service WHERE key = 'ssh'")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// The fields of a flow `ssh` from `source` to `destination` for `acme`.
    pub(crate) async fn fields(&self, source: Uuid, destination: Uuid) -> Value {
        json!({
            "local": 1,
            "source": { "kind": "address", "id": { "id": source } },
            "destination": { "kind": "address", "id": { "id": destination } },
            "service": { "id": self.ssh().await },
            "external_refs": [{ "system": "ticketing", "id": "CHG-1042" }],
            "entity": { "id": self.acme },
        })
    }

    pub(crate) async fn flows(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM flow.flow")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

pub(crate) fn grants(subject: Uuid, permissions: &[&str]) -> Grants {
    permissions
        .iter()
        .fold(Grants::new(), |g, p| g.everywhere(subject, p))
}

pub(crate) fn later(days: i64) -> String {
    let at = OffsetDateTime::now_utc() + time::Duration::days(days);
    at.format(&time::format_description::well_known::Rfc3339)
        .unwrap()
}

pub(crate) fn rules(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

pub(crate) fn proposal(outcome: &Outcome) -> Uuid {
    match outcome {
        Outcome::Pending {
            changeset,
            kind: PendingKind::Proposal,
            ..
        } => *changeset,
        other => panic!("not a proposal: {other:?}"),
    }
}

/// The rules that refused a validation, whichever way it was refused.
pub(crate) fn refusals(result: &Result<Outcome, RequestError>) -> Vec<&'static str> {
    match result {
        Ok(Outcome::FailedRevalidation { violations, .. }) => {
            violations.iter().map(|v| v.rule.id).collect()
        }
        Ok(outcome) => rules(outcome),
        Err(RequestError::Review(v)) => vec![v.rule.id],
        Err(other) => panic!("not refused: {other:?}"),
    }
}

/// A dated request (`end`) in a changeset of open period applies: the anchor of the flow
/// and its defaults stop with it, never outliving the flow (REQ-011-5, REQ-002-11).
#[kartenn_testing::pg_test]
async fn a_dated_request_in_an_open_changeset_applies(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let requester = env.person().await;
    let mut fields = env.fields(prod, dev).await;
    fields["end"] = json!(later(30));
    let outcome = env
        .submit_as(
            &grants(requester, REQUEST),
            requester,
            vec![flow("request", fields)],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let ends: bool =
        sqlx::query_scalar("SELECT bool_and(NOT upper_inf(valid_period)) FROM flow.flow")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(ends, "the flow ends");
}

/// REQ-011-21: a compliant request, every condition met, is applied with its evaluation
/// (summary `allowed`, outcome `automatic`), frozen at its registration (REQ-011-23).
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_21_a_compliant_request_applies(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let requester = env.person().await;
    let outcome = env
        .submit_as(
            &grants(requester, REQUEST),
            requester,
            vec![flow("request", env.fields(prod, dev).await)],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let (summary, outcome, requested_by): (String, String, Uuid) = sqlx::query_as(
        "SELECT r.summary, r.outcome, f.requester_id
           FROM flow.flow_registration r JOIN flow.flow f ON f.id = r.flow_id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (summary.as_str(), outcome.as_str()),
        ("allowed", "automatic")
    );
    assert_eq!(requested_by, requester);
    let pairs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM flow.flow_evaluation WHERE verdict = 'allowed'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pairs, 1);
}

/// REQ-011-21: a request the policy denies (no rule allows dev to prod) waits for review:
/// a proposal, even for a requester who may request; nothing written.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_21_a_denied_request_waits_for_review(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let requester = env.person().await;
    let outcome = env
        .submit_as(
            &grants(requester, REQUEST),
            requester,
            vec![flow("request", env.fields(dev, prod).await)],
        )
        .await;
    proposal(&outcome);
    assert_eq!(env.flows().await, 0);
}

/// REQ-011-8: the direct write of a denied flow, without review nor waiver, is refused,
/// the policy cited.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_8_a_denied_direct_write_is_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let outcome = env
        .submit_as(
            &Unrestricted,
            env.author,
            vec![flow("register", env.fields(dev, prod).await)],
        )
        .await;
    let found = rules(&outcome);
    assert!(found.contains(&"flow.review.required"), "{found:?}");
    assert!(found.contains(&"flow.waiver.required"), "{found:?}");
    assert_eq!(env.flows().await, 0);
}

/// REQ-011-22, 9: a denied request, reviewed by its validator with an end and a waiver, is
/// applied, the reviewer and the waiver recorded.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_22_a_reviewed_and_waived_request_applies(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let (requester, reviewer) = (env.person().await, env.person().await);
    let authorizer = grants(requester, REQUEST)
        .everywhere(reviewer, "flow.request")
        .everywhere(reviewer, "prop.core.owner.apply")
        .everywhere(reviewer, "flow.review")
        .everywhere(reviewer, "flow.waiver.grant");
    let id = proposal(
        &env.submit_as(
            &authorizer,
            requester,
            vec![flow("request", env.fields(dev, prod).await)],
        )
        .await,
    );
    let result = validate_pending_with(
        &env.store,
        &authorizer,
        &Sources::none(),
        id,
        &reviewer.into(),
        &RequestContext::person(reviewer),
        vec![
            flow(
                "approve_review",
                json!({ "flow": { "local": 1 }, "end": later(90) }),
            ),
            flow(
                "waive",
                json!({ "flow": { "local": 1 }, "reason": "migration", "accepted_risk": "admin access",
                        "end": later(60) }),
            ),
        ],
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(result, Outcome::Applied { .. }), "{result:?}");
    let (reviewed_by, requester_id): (Option<Uuid>, Uuid) =
        sqlx::query_as("SELECT reviewed_by, requester_id FROM flow.flow")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((reviewed_by, requester_id), (Some(reviewer), requester));
    let (granted_by,): (Uuid,) = sqlx::query_as("SELECT granted_by FROM flow.flow_waiver")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(granted_by, reviewer);
}

/// REQ-011-22: the requester does not review their own request (separation of duties,
/// C1), even through a service account that requested on their behalf.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_22_the_requester_does_not_review(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let (service, requester) = (env.person().await, env.person().await);
    let authorizer = grants(service, REQUEST)
        .everywhere(requester, "flow.request")
        .everywhere(requester, "prop.core.owner.apply")
        .everywhere(requester, "flow.review")
        .everywhere(requester, "flow.waiver.grant");
    let mut context = RequestContext::person(service);
    context.on_behalf_of = Some(requester);
    let id = proposal(
        &submit_in(
            &env.store,
            &authorizer,
            &Sources::none(),
            &context,
            &env.request(service, vec![flow("request", env.fields(dev, prod).await)]),
            MAX_OPERATIONS,
        )
        .await
        .unwrap(),
    );
    let result = validate_pending_with(
        &env.store,
        &authorizer,
        &Sources::none(),
        id,
        &requester.into(),
        &RequestContext::person(requester),
        vec![
            flow(
                "approve_review",
                json!({ "flow": { "local": 1 }, "end": later(90) }),
            ),
            flow(
                "waive",
                json!({ "flow": { "local": 1 }, "reason": "migration", "accepted_risk": "admin access",
                        "end": later(60) }),
            ),
        ],
        MAX_OPERATIONS,
    )
    .await;
    let found = refusals(&result);
    assert!(found.contains(&"flow.separation"), "{found:?}");
    assert!(found.contains(&"flow.waiver.invalid"), "{found:?}");
    assert_eq!(env.flows().await, 0);
}

/// REQ-011-9, 22: a waiver beyond six months and a review beyond twelve are refused.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_9_ceilings_of_waiver_and_review(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let (requester, reviewer) = (env.person().await, env.person().await);
    let authorizer = grants(requester, REQUEST)
        .everywhere(reviewer, "flow.request")
        .everywhere(reviewer, "prop.core.owner.apply")
        .everywhere(reviewer, "flow.review")
        .everywhere(reviewer, "flow.waiver.grant");
    let id = proposal(
        &env.submit_as(
            &authorizer,
            requester,
            vec![flow("request", env.fields(dev, prod).await)],
        )
        .await,
    );
    let result = validate_pending_with(
        &env.store,
        &authorizer,
        &Sources::none(),
        id,
        &reviewer.into(),
        &RequestContext::person(reviewer),
        vec![
            flow(
                "approve_review",
                json!({ "flow": { "local": 1 }, "end": later(400) }),
            ),
            flow(
                "waive",
                json!({ "flow": { "local": 1 }, "reason": "migration", "accepted_risk": "admin access",
                        "end": later(240) }),
            ),
        ],
        MAX_OPERATIONS,
    )
    .await;
    let found = refusals(&result);
    assert!(found.contains(&"flow.review.end_required"), "{found:?}");
    assert!(found.contains(&"flow.waiver.invalid"), "{found:?}");
}

/// REQ-011-22, 25: a request rejected at review is a non-conformity of its requester, with
/// the motive and the evaluation of its triage.
#[kartenn_testing::pg_test]
async fn a_rejected_request_is_a_nonconformity(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let (requester, reviewer) = (env.person().await, env.person().await);
    let authorizer = grants(requester, REQUEST)
        .everywhere(reviewer, "flow.review")
        .everywhere(reviewer, "flow.request")
        .everywhere(reviewer, "prop.core.owner.apply");
    let id = proposal(
        &env.submit_as(
            &authorizer,
            requester,
            vec![flow("request", env.fields(dev, prod).await)],
        )
        .await,
    );
    reject(
        &env.store,
        &authorizer,
        id,
        &reviewer.into(),
        "no administration from dev",
    )
    .await
    .unwrap();
    let (reason, by, who, entity, summary): (String, Uuid, Uuid, Uuid, Option<String>) =
        sqlx::query_as(
            "SELECT reason, reviewed_by, requester_id, entity_id, evaluation->>'summary'
               FROM flow.flow_rejection WHERE changeset_id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reason, "no administration from dev");
    assert_eq!((by, who, entity), (reviewer, requester, env.acme));
    assert_eq!(summary.as_deref(), Some("denied"));
}

/// REQ-011-17: a request needs no read of its endpoints; the direct write by
/// `flow.edit` towards an object its author may not read is refused as an absent
/// object.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_17_reads_of_the_endpoints(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let requester = env.person().await;
    let outcome = env
        .submit_as(
            &grants(requester, REQUEST),
            requester,
            vec![flow("request", env.fields(prod, dev).await)],
        )
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let writer = env.person().await;
    let outcome = env
        .submit_as(
            &grants(
                writer,
                &[
                    "flow.edit",
                    "flow.approval.self",
                    "prop.core.owner.edit",
                    "prop.core.owner.approval.self",
                ],
            ),
            writer,
            vec![flow("register", env.fields(prod, dev).await)],
        )
        .await;
    assert!(
        rules(&outcome).contains(&"core.object.forbidden"),
        "{outcome:?}"
    );
    assert_eq!(env.flows().await, 1);
}

/// REQ-011-1: a request without external reference is refused (`flow.reference.missing`).
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_1_a_reference_is_required(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let mut fields = env.fields(prod, dev).await;
    fields["external_refs"] = json!([]);
    let outcome = env
        .submit_as(&Unrestricted, env.author, vec![flow("request", fields)])
        .await;
    assert!(
        rules(&outcome).contains(&"flow.reference.missing"),
        "{outcome:?}"
    );
}

/// REQ-011-1, REQ-016-3: an external reference names its system by the key of its value of
/// `core.external_system`, stored by the value's identifier, which is accepted as well; a
/// system that names no value is refused (`flow.reference.system`), never dropped.
#[kartenn_testing::pg_test]
async fn an_external_reference_names_its_system_by_its_key(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let mut fields = env.fields(prod, dev).await;
    for unknown in [json!("tickets"), json!(Uuid::nil())] {
        fields["external_refs"] = json!([{ "system": unknown, "id": "CHG-1" }]);
        let outcome = env
            .submit_as(
                &Unrestricted,
                env.author,
                vec![flow("request", fields.clone())],
            )
            .await;
        assert_eq!(rules(&outcome), ["flow.reference.system"], "{outcome:?}");
    }
    fields["external_refs"] = json!([{ "system": "ticketing", "id": "CHG-1" }]);
    fields["on_behalf_of"] = json!({ "external": { "system": "nobody", "id": "u-1" } });
    let outcome = env
        .submit_as(
            &Unrestricted,
            env.author,
            vec![flow("request", fields.clone())],
        )
        .await;
    assert_eq!(rules(&outcome), ["flow.reference.system"], "{outcome:?}");
    fields["on_behalf_of"] = json!({ "external": { "system": "ticketing", "id": "u-1" } });
    env.applied(vec![flow("request", fields)]).await;
    let system: Uuid =
        sqlx::query_scalar("SELECT id FROM property_enum_value WHERE key = 'ticketing'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (refs, on_behalf_of): (Value, Value) =
        sqlx::query_as("SELECT external_refs, on_behalf_of FROM flow.flow")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(refs, json!([{ "system": system, "id": "CHG-1" }]));
    assert_eq!(on_behalf_of["external"]["system"], json!(system));

    // The identifier the flow is read with is accepted when it is updated.
    let id: Uuid = sqlx::query_scalar("SELECT id FROM flow.flow")
        .fetch_one(&pool)
        .await
        .unwrap();
    env.applied(vec![flow(
        "update",
        json!({ "id": { "id": id },
                "external_refs": [{ "system": system, "id": "CHG-2", "label": "deux" }] }),
    )])
    .await;
    let refs: Value =
        sqlx::query_scalar("SELECT external_refs FROM flow.flow WHERE valid_period @> now()")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        refs,
        json!([{ "system": system, "id": "CHG-2", "label": "deux" }])
    );
}

/// REQ-011-4: changing the service of a flow is refused; its justification changes.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_4_the_identity_of_a_flow_does_not_change(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    env.applied(vec![flow("request", env.fields(prod, dev).await)])
        .await;
    let id: Uuid = sqlx::query_scalar("SELECT id FROM flow.flow")
        .fetch_one(&pool)
        .await
        .unwrap();
    let outcome = env
        .submit_as(
            &Unrestricted,
            env.author,
            vec![flow(
                "update",
                json!({ "id": { "id": id }, "destination": { "kind": "address", "id": { "id": prod } } }),
            )],
        )
        .await;
    assert!(
        rules(&outcome).contains(&"flow.update.identity"),
        "{outcome:?}"
    );
    env.applied(vec![flow(
        "update",
        json!({ "id": { "id": id }, "justification": "backups" }),
    )])
    .await;
    let justification: Option<String> =
        sqlx::query_scalar("SELECT justification FROM flow.flow WHERE valid_period @> now()")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(justification.as_deref(), Some("backups"));
}
