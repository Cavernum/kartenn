//! The references of the endpoints (task 011-4, REQ-011-3): closing an address that a
//! living flow names is refused, the flow listed; the cascade closes the flow with it.
//! The flow is written as the operations of task 011-3 will write it: its row and its
//! record in `module_object`.
//!
//! The references of the operations of the module (REQ-002-12, amended; task 002-29d3):
//! each `Ref::Id` of `request`, `register`, `update`, `waive` and `approve_review`
//! designates an object of its kind in force at the start of the period, refused otherwise
//! by `core.reference.unknown`; and an author without the right on the object a reference
//! names gets the same answer whatever its identifier names (INV-63).

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_flow::Flows;
use kartenn_machines::Machines;
use kartenn_policy::Policy;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use tower::ServiceExt;

use crate::extensions::WRITE;
use crate::requests::{Env as Registry, REQUEST, flow, later, policy};

struct Env {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
}

impl Env {
    /// The modules migrated and declared, the entity `acme` and the domain `lab` it owns.
    async fn new(pool: &PgPool) -> Self {
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
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let env = Self {
            pool: pool.clone(),
            store,
            author,
        };
        env.applied(vec![Operation::CreateEntity {
            local: Some(1),
            slug: "acme".into(),
            parent: None,
        }])
        .await;
        let owner: Uuid =
            sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
                .fetch_one(pool)
                .await
                .unwrap();
        let acme = env.slugged("entity", "acme").await;
        env.applied(vec![
            Operation::CreateDomain {
                local: Some(2),
                slug: "lab".into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(2),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(acme.to_string())],
            },
        ])
        .await;
        env
    }

    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) -> Outcome {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        outcome
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

    /// The address `value` of the domain `lab`.
    async fn address(&self, value: &str) -> Uuid {
        let lab = self.slugged("addressing_domain", "lab").await;
        let network: Operation = serde_json::from_value(serde_json::json!({
            "op": "create_prefix", "domain": {"id": lab}, "prefix": "10.0.0.0/24",
            "nature": "network",
        }))
        .unwrap();
        self.applied(vec![network]).await;
        self.applied(vec![Operation::CreateAddress {
            local: None,
            domain: Ref::Id(lab),
            address: value.into(),
            usage: AddressUsage::Host,
        }])
        .await;
        sqlx::query_scalar(
            "SELECT id FROM address WHERE address = $1::inet AND valid_period @> now()",
        )
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A living flow from the address `source`, written as the module writes one.
    async fn flow_from(&self, source: Uuid) -> Uuid {
        let (id, changeset) = (Uuid::now_v7(), Uuid::now_v7());
        let acme = self.slugged("entity", "acme").await;
        sqlx::query(
            "INSERT INTO flow.flow (id, kind, source_kind, source_id, destination_kind,
                                    destination_id, service_id, external_refs, requester_id,
                                    entity_id, valid_period, changeset_id)
             VALUES ($1, 'requested', 'address', $2, 'prefix', gen_random_uuid(),
                     gen_random_uuid(), '[{\"system\": \"00000000-0000-0000-0000-000000000001\",
                     \"id\": \"CHG-1042\"}]', $3, $4, tstzrange(now() - interval '1 hour', NULL), $5)",
        )
        .bind(id)
        .bind(source)
        .bind(self.author)
        .bind(acme)
        .bind(changeset)
        .execute(&self.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO module_object (id, module, kind, valid_period, changeset_id)
             VALUES ($1, 'flow', 'flow.flow', tstzrange(now() - interval '1 hour', NULL), $2)",
        )
        .bind(id)
        .bind(changeset)
        .execute(&self.pool)
        .await
        .unwrap();
        id
    }

    async fn lives(&self, table: &str, id: Uuid) -> bool {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 FROM {table} WHERE id = $1 AND valid_period @> now())"
        )))
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn close_address(id: Uuid, cascade: bool) -> Operation {
    Operation::Close {
        kind: ObjectKind::Address,
        id: Ref::Id(id),
        expected: None,
        cascade,
    }
}

#[kartenn_testing::pg_test]
async fn closing_a_cited_address_is_refused_with_its_flows(pool: PgPool) {
    let env = Env::new(&pool).await;
    let address = env.address("10.0.0.1").await;
    let flow = env.flow_from(address).await;
    let refused = env.submit(vec![close_address(address, false)]).await;
    let Outcome::Refused { violations, .. } = &refused else {
        panic!("not refused: {refused:?}");
    };
    assert!(
        violations
            .iter()
            .any(|v| v.rule.id == "core.object.dependents" && v.subjects.contains(&flow)),
        "{violations:?}"
    );
    assert!(env.lives("address", address).await);
    assert!(env.lives("flow.flow", flow).await);
}

#[kartenn_testing::pg_test]
async fn the_cascade_closes_the_flow_with_its_endpoint(pool: PgPool) {
    let env = Env::new(&pool).await;
    let address = env.address("10.0.0.1").await;
    let flow = env.flow_from(address).await;
    let applied = env.applied(vec![close_address(address, true)]).await;
    assert!(!env.lives("address", address).await);
    assert!(!env.lives("flow.flow", flow).await);
    assert!(!env.lives("module_object", flow).await);
    let Outcome::Applied { cascaded, .. } = applied else {
        unreachable!()
    };
    assert!(cascaded.iter().any(|(_, id)| *id == flow), "{cascaded:?}");
}

#[kartenn_testing::pg_test]
async fn an_ended_flow_does_not_hold_its_endpoint(pool: PgPool) {
    let env = Env::new(&pool).await;
    let address = env.address("10.0.0.1").await;
    let flow = env.flow_from(address).await;
    sqlx::query(
        "UPDATE flow.flow SET valid_period = tstzrange(lower(valid_period), now() - interval '1 minute')
          WHERE id = $1",
    )
    .bind(flow)
    .execute(&pool)
    .await
    .unwrap();
    env.applied(vec![close_address(address, false)]).await;
}

const UNKNOWN: &str = "core.reference.unknown";

/// What the waiver holder holds on the entity (REQ-011-9), and the reviewer (REQ-011-22).
const WAIVE: &[&str] = &["flow.waiver.grant"];
const REVIEW: &[&str] = &["flow.review"];

/// The objects the references of the operations designate: those of `acme`, in reach,
/// those of `other`, out of reach, and those closed.
struct Objects {
    registry: Registry,
    authorizer: PgAuthorizer,
    other: Uuid,
    gone_entity: Uuid,
    /// The domain of `other`, an object of another kind than every reference.
    far: Uuid,
    /// The addresses `prod` and `dev` of `acme`, and of `other`.
    prod: Uuid,
    dev: Uuid,
    far_prod: Uuid,
    far_dev: Uuid,
    /// An address of `acme`, closed.
    gone_address: Uuid,
    ssh: Uuid,
    /// A service of the policy, closed.
    old_service: Uuid,
    /// A flow of `acme`, a flow of `other` and a flow of `other` closed.
    flow: Uuid,
    far_flow: Uuid,
    gone_flow: Uuid,
}

impl Objects {
    async fn new(pool: &PgPool) -> Self {
        let registry = Registry::new(pool).await;
        let authorizer = PgAuthorizer::new(pool.clone()).with_modules(registry.registry.clone());
        let other = entity(&registry, "other").await;
        let gone_entity = entity(&registry, "gone").await;
        registry.applied(vec![close("entity", gone_entity)]).await;
        let far = domain(&registry, "far", other).await;
        let prod = registry.address("10.1.0.0/24", "10.1.0.1", "prod").await;
        let dev = registry.address("10.2.0.0/24", "10.2.0.1", "dev").await;
        let gone_address = registry.address("10.3.0.0/24", "10.3.0.1", "dev").await;
        registry.applied(vec![close("address", gone_address)]).await;
        let far_prod = address_in(&registry, far, ("192.0.2.0/25", "192.0.2.1"), "prod").await;
        let far_dev = address_in(&registry, far, ("192.0.2.128/25", "192.0.2.129"), "dev").await;
        let ssh = id_of(
            &registry,
            "SELECT id FROM policy.policy_service WHERE key = 'ssh'",
        )
        .await;
        let admin = id_of(
            &registry,
            "SELECT id FROM policy.policy_flow_type WHERE key = 'admin'",
        )
        .await;
        registry
            .applied(vec![policy(
                "service.create",
                json!({ "key": "old", "labels": { "fr": "Ancien" }, "flow_type": { "id": admin },
                        "entries": [{ "transport": "tcp", "ports": 23 }] }),
            )])
            .await;
        let old_service = id_of(
            &registry,
            "SELECT id FROM policy.policy_service WHERE key = 'old'",
        )
        .await;
        registry
            .applied(vec![close("policy.service", old_service)])
            .await;
        let mut objects = Self {
            registry,
            authorizer,
            other,
            gone_entity,
            far,
            prod,
            dev,
            far_prod,
            far_dev,
            gone_address,
            ssh,
            old_service,
            flow: Uuid::nil(),
            far_flow: Uuid::nil(),
            gone_flow: Uuid::nil(),
        };
        let acme = objects.registry.acme;
        objects.flow = objects.requested(prod, dev, acme).await;
        objects.far_flow = objects.requested(far_prod, far_dev, other).await;
        objects.gone_flow = objects.requested(far_prod, dev, other).await;
        objects
            .registry
            .applied(vec![close("flow.flow", objects.gone_flow)])
            .await;
        objects
    }

    /// The flow `ssh` from `source` to `destination` for `entity`, requested and applied.
    async fn requested(&self, source: Uuid, destination: Uuid, entity: Uuid) -> Uuid {
        let mut fields = self.registry.fields(source, destination).await;
        fields["entity"] = json!({ "id": entity });
        self.registry.applied(vec![flow("request", fields)]).await;
        sqlx::query_scalar(
            "SELECT id FROM flow.flow WHERE source_id = $1 AND destination_id = $2
              ORDER BY lower(valid_period) DESC LIMIT 1",
        )
        .bind(source)
        .bind(destination)
        .fetch_one(&self.registry.pool)
        .await
        .unwrap()
    }

    /// A person holding `entries` on `acme`.
    async fn holding(&self, entries: &[&str]) -> Uuid {
        let person = self.registry.person().await;
        self.registry
            .grant(person, entries, Some(self.registry.acme))
            .await;
        person
    }

    /// `operation` submitted by `author` under the RBAC of the store, and its refusal as he
    /// sees it, the identifier `x` it cites masked.
    async fn seen_as(&self, author: Uuid, operation: Operation, x: Uuid) -> (Outcome, Vec<Seen>) {
        let since = Uuid::now_v7();
        let outcome = self
            .registry
            .submit_as(&self.authorizer, author, vec![operation])
            .await;
        let seen = seen(&outcome, x, since);
        (outcome, seen)
    }

    /// The fields of a flow `ssh` from `prod` to `dev` for `acme`, with `extra`.
    async fn fields(&self, extra: Value) -> Value {
        let mut fields = self.registry.fields(self.prod, self.dev).await;
        merge(&mut fields, extra);
        fields.as_object_mut().unwrap().remove("local");
        fields
    }

    /// What the operations of the tests write: the versions of the flows, the flows.
    async fn written(&self) -> (i64, i64) {
        sqlx::query_as("SELECT count(*), count(DISTINCT id) FROM flow.flow")
            .fetch_one(&self.registry.pool)
            .await
            .unwrap()
    }
}

fn merge(into: &mut Value, extra: Value) {
    if let (Value::Object(into), Value::Object(extra)) = (into, extra) {
        into.extend(extra);
    }
}

async fn id_of(env: &Registry, sql: &'static str) -> Uuid {
    sqlx::query_scalar(sql).fetch_one(&env.pool).await.unwrap()
}

async fn entity(env: &Registry, slug: &str) -> Uuid {
    env.applied(vec![Operation::CreateEntity {
        local: None,
        slug: slug.into(),
        parent: None,
    }])
    .await;
    env.slugged("entity", slug).await
}

/// The domain `slug` owned by `owner`.
async fn domain(env: &Registry, slug: &str, owner: Uuid) -> Uuid {
    env.applied(vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: slug.into(),
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(env.property("core.owner").await),
            values: vec![RawInput::Text(owner.to_string())],
        },
    ])
    .await;
    env.slugged("addressing_domain", slug).await
}

/// The address `address` of the network `network` of `domain`, classified `environment`.
async fn address_in(
    env: &Registry,
    domain: Uuid,
    (network, address): (&str, &str),
    environment: &str,
) -> Uuid {
    let property = env.property("policy.environment").await;
    env.applied(vec![
        Operation::CreatePrefix {
            local: Some(1),
            domain: Ref::Id(domain),
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
            domain: Ref::Id(domain),
            address: address.into(),
            usage: AddressUsage::Host,
        },
    ])
    .await;
    sqlx::query_scalar("SELECT id FROM address WHERE address = $1::inet")
        .bind(address)
        .fetch_one(&env.pool)
        .await
        .unwrap()
}

fn close(kind: &str, id: Uuid) -> Operation {
    Operation::Close {
        kind: ObjectKind::from_name(kind).unwrap(),
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    }
}

fn endpoint(id: Uuid) -> Value {
    json!({ "kind": "address", "id": { "id": id } })
}

fn waive(id: Uuid) -> Operation {
    flow(
        "waive",
        json!({ "flow": { "id": id }, "reason": "r", "accepted_risk": "a", "end": later(10) }),
    )
}

fn approve_review(id: Uuid) -> Operation {
    flow(
        "approve_review",
        json!({ "flow": { "id": id }, "end": later(10) }),
    )
}

fn update(id: Uuid, extra: Value) -> Operation {
    let mut fields = json!({ "id": { "id": id } });
    merge(&mut fields, extra);
    flow("update", fields)
}

/// The rules and the objects of a refusal.
fn refusal_of(outcome: &Outcome) -> (Vec<&'static str>, BTreeSet<Uuid>) {
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("expected a refusal, got {outcome:?}")
    };
    (
        violations.iter().map(|v| v.rule.id).collect(),
        violations
            .iter()
            .flat_map(|v| v.subjects.iter().copied())
            .collect(),
    )
}

/// A violation as its author sees it: rule, message, operation, objects and detail.
type Seen = (&'static str, String, Option<usize>, Vec<String>, String);

/// The refusal of `outcome` as its author sees it, the identifier `x` it cites written
/// `<x>` wherever it appears, and an object the submission created (identified after
/// `since`) written `<new>`; nothing for an outcome that is no refusal.
fn seen(outcome: &Outcome, x: Uuid, since: Uuid) -> Vec<Seen> {
    let mask = |s: String| s.replace(&x.to_string(), "<x>");
    let object = |s: Uuid| match s {
        s if s == x => "<x>".to_owned(),
        s if s > since => "<new>".to_owned(),
        s => s.to_string(),
    };
    match outcome {
        Outcome::Refused { violations, .. } => violations
            .iter()
            .map(|v| {
                (
                    v.rule.id,
                    mask(v.message.clone()),
                    v.operation,
                    v.subjects.iter().copied().map(object).collect(),
                    mask(v.detail.as_ref().map(Value::to_string).unwrap_or_default()),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The identifier at `path` in the body of the operation of the module `op`.
fn body_id(op: &Operation, path: &[&str]) -> Uuid {
    let Operation::Module(m) = op else {
        panic!("not an operation of a module: {op:?}")
    };
    let mut value = &m.body[path[0]];
    for key in &path[1..] {
        value = &value[*key];
    }
    value.as_str().unwrap().parse().unwrap()
}

/// REQ-002-12 (amendment of 2026-11-14), task 002-29d3: every reference of every operation
/// of the module, field by field, is read before it is written; a new operation type fails
/// here until it has a sample.
#[kartenn_testing::pg_test]
async fn every_reference_of_every_operation_is_checked(pool: PgPool) {
    let o = Objects::new(&pool).await;
    let id = Uuid::now_v7;
    let (unknown, source, destination, service) = (id(), id(), id(), id());
    let mut samples: Vec<(Operation, BTreeSet<Uuid>)> = Vec::new();
    for name in ["request", "register"] {
        let all = json!({ "source": endpoint(id()), "destination": endpoint(id()),
                          "service": { "id": id() }, "entity": { "id": id() } });
        let op = flow(name, o.fields(all).await);
        let named = [
            body_id(&op, &["source", "id", "id"]),
            body_id(&op, &["destination", "id", "id"]),
            body_id(&op, &["service", "id"]),
            body_id(&op, &["entity", "id"]),
        ];
        samples.push((op, named.into()));
    }
    samples.extend([
        (
            update(unknown, json!({ "justification": "j" })),
            [unknown].into(),
        ),
        // The fields of the identity of a flow in force.
        (
            update(
                o.flow,
                json!({ "source": endpoint(source), "destination": endpoint(destination),
                        "service": { "id": service } }),
            ),
            [source, destination, service].into(),
        ),
        (waive(unknown), [unknown].into()),
        (approve_review(unknown), [unknown].into()),
    ]);
    let types: BTreeSet<String> = Flows
        .operations()
        .iter()
        .map(|o| format!("flow.{}", o.name))
        .collect();
    let covered: BTreeSet<String> = samples
        .iter()
        .map(|(op, _)| match op {
            Operation::Module(m) => m.tag(),
            other => panic!("not an operation of the module: {other:?}"),
        })
        .collect();
    assert_eq!(covered, types, "an operation of the module has no sample");
    let before = o.written().await;
    for (op, expected) in samples {
        let outcome = o
            .registry
            .submit_as(&Unrestricted, o.registry.author, vec![op.clone()])
            .await;
        let (rules, named) = refusal_of(&outcome);
        assert!(rules.iter().all(|r| *r == UNKNOWN), "{op:?}: {rules:?}");
        assert_eq!(named, expected, "{op:?}: every reference is checked");
    }
    assert_eq!(o.written().await, before, "nothing is written");
}

/// REQ-002-12 (amendment of 2026-11-14), task 002-29d3: a reference of another kind or
/// closed is refused as one that names nothing, and nothing is written.
#[kartenn_testing::pg_test]
async fn the_references_designate_objects_in_force(pool: PgPool) {
    let o = Objects::new(&pool).await;
    let cases: Vec<(&str, Operation, Uuid)> = vec![
        (
            "source: closed",
            flow(
                "request",
                o.fields(json!({ "source": endpoint(o.gone_address) }))
                    .await,
            ),
            o.gone_address,
        ),
        (
            "destination: a domain",
            flow(
                "register",
                o.fields(json!({ "destination": endpoint(o.far) })).await,
            ),
            o.far,
        ),
        (
            "service: closed",
            flow(
                "request",
                o.fields(json!({ "service": { "id": o.old_service } }))
                    .await,
            ),
            o.old_service,
        ),
        (
            "service: an address",
            flow(
                "register",
                o.fields(json!({ "service": { "id": o.prod } })).await,
            ),
            o.prod,
        ),
        (
            "entity: closed",
            flow(
                "request",
                o.fields(json!({ "entity": { "id": o.gone_entity } })).await,
            ),
            o.gone_entity,
        ),
        (
            "entity: a domain",
            flow(
                "request",
                o.fields(json!({ "entity": { "id": o.far } })).await,
            ),
            o.far,
        ),
        (
            "update: an address",
            update(o.prod, json!({ "justification": "j" })),
            o.prod,
        ),
        (
            "update: closed",
            update(o.gone_flow, json!({ "justification": "j" })),
            o.gone_flow,
        ),
        (
            "update source: closed",
            update(o.flow, json!({ "source": endpoint(o.gone_address) })),
            o.gone_address,
        ),
        ("waive: closed", waive(o.gone_flow), o.gone_flow),
        ("approve_review: an address", approve_review(o.prod), o.prod),
    ];
    let before = o.written().await;
    for (case, operation, named) in cases {
        let outcome = o
            .registry
            .submit_as(&Unrestricted, o.registry.author, vec![operation])
            .await;
        assert_eq!(
            refusal_of(&outcome),
            (vec![UNKNOWN], [named].into()),
            "{case}"
        );
    }
    assert_eq!(o.written().await, before, "nothing is written");
}

/// An operation citing the identifier it is given.
type Make<'a> = Box<dyn Fn(Uuid) -> Operation + 'a>;

/// A case of a leak: its name, its author, the operation citing `x`, the object in force
/// out of reach, the hidden ones, and whether the reference needs a right.
type Case<'a> = (&'a str, Uuid, Make<'a>, Uuid, Vec<Uuid>, bool);

/// INV-63 on the references of the module (task 002-29d3), each field of each operation:
/// an author without the right on the object a reference names gets the same answer
/// whatever its identifier names (nothing, an object of another kind, an object closed, an
/// object in force out of his reach) — rules, messages, operations, objects and details,
/// the identifier cited masked. Where the reference needs a right, the refusal of his
/// rights; where it needs none (the endpoints and the service of a request, F12, and the
/// service of a direct write: an oracle left to task 31), the operation out of reach is
/// accepted and every other answer is the same `core.reference.unknown`.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // the cases, field by field
async fn no_right_reaches_through_a_hidden_reference(pool: PgPool) {
    let o = Objects::new(&pool).await;
    let requester = o.holding(REQUEST).await;
    let editor = o.holding(&[WRITE, &["core.address.read"]].concat()).await;
    let waiver = o.holding(WAIVE).await;
    let reviewer = o.holding(REVIEW).await;
    let unknown = Uuid::now_v7();
    let base = o.fields(json!({})).await;
    let with = move |name: &'static str, extra: Value| {
        let mut fields = base.clone();
        merge(&mut fields, extra);
        flow(name, fields)
    };
    let (with, far_flow, gone_address) = (&with, o.far_flow, o.gone_address);
    let addresses = vec![unknown, o.far, o.gone_address];
    let services = vec![unknown, o.far_prod, o.old_service];
    let entities = vec![unknown, o.far, o.gone_entity];
    let flows = vec![unknown, o.far_prod, o.gone_flow];
    let mut cases: Vec<Case> = Vec::new();
    for (name, who, needs_right) in [("request", requester, false), ("register", editor, true)] {
        cases.extend([
            (
                name,
                who,
                Box::new(move |x| with(name, json!({ "source": endpoint(x) }))) as Make,
                o.far_prod,
                addresses.clone(),
                needs_right,
            ),
            (
                name,
                who,
                Box::new(move |x| with(name, json!({ "destination": endpoint(x) }))),
                o.far_dev,
                addresses.clone(),
                needs_right,
            ),
            (
                name,
                who,
                Box::new(move |x| with(name, json!({ "service": { "id": x } }))),
                o.ssh,
                services.clone(),
                false,
            ),
            (
                name,
                who,
                Box::new(move |x| with(name, json!({ "entity": { "id": x } }))),
                o.other,
                entities.clone(),
                true,
            ),
        ]);
    }
    cases.extend([
        (
            "update.id",
            editor,
            Box::new(|x| update(x, json!({ "justification": "j" }))) as Make,
            o.far_flow,
            flows.clone(),
            true,
        ),
        // A reference the author reads, closed: refused alike whether the flow is or not.
        (
            "update.id, a source closed",
            editor,
            Box::new(move |x| update(x, json!({ "source": endpoint(gone_address) }))),
            o.far_flow,
            flows.clone(),
            true,
        ),
        // The identity of a flow out of reach is not compared before his rights: its
        // endpoints and its service are no oracle.
        (
            "update.source",
            editor,
            Box::new(move |x| update(far_flow, json!({ "source": endpoint(x) }))),
            o.far_prod,
            addresses.clone(),
            true,
        ),
        (
            "update.destination",
            editor,
            Box::new(move |x| update(far_flow, json!({ "destination": endpoint(x) }))),
            o.far_dev,
            addresses.clone(),
            true,
        ),
        (
            "update.service",
            editor,
            Box::new(move |x| update(far_flow, json!({ "service": { "id": x } }))),
            o.ssh,
            services.clone(),
            true,
        ),
        (
            "waive.flow",
            waiver,
            Box::new(waive),
            o.far_flow,
            flows.clone(),
            true,
        ),
        (
            "approve_review.flow",
            reviewer,
            Box::new(approve_review),
            o.far_flow,
            flows,
            true,
        ),
    ]);
    let mut leaks = Vec::new();
    for (i, (name, who, make, reach, hidden, needs_right)) in cases.iter().enumerate() {
        // Hidden first: an operation out of reach accepted changes what follows.
        let mut answers = Vec::new();
        for h in hidden {
            answers.push((*h, o.seen_as(*who, make(*h), *h).await));
        }
        let (out_of_reach, seen_out_of_reach) = o.seen_as(*who, make(*reach), *reach).await;
        let expected = if *needs_right {
            seen_out_of_reach
        } else {
            assert!(
                matches!(
                    out_of_reach,
                    Outcome::Applied { .. } | Outcome::Pending { .. }
                ),
                "{i} {name}: {out_of_reach:?}"
            );
            let (_, (first, seen_first)) = &answers[0];
            assert_eq!(refusal_of(first).0, [UNKNOWN], "{i} {name}");
            seen_first.clone()
        };
        assert!(!expected.is_empty(), "{i} {name}: {out_of_reach:?}");
        for (h, (answer, found)) in &answers {
            if *found != expected {
                leaks.push(format!(
                    "{i} {name}, hidden {h}: {answer:?} / expected {expected:?}"
                ));
            }
        }
    }
    // The kind of a flow out of reach is no oracle either.
    let kind = |k: &str| update(far_flow, json!({ "kind": k }));
    let ((observed, seen_observed), (requested, seen_requested)) = (
        o.seen_as(editor, kind("observed"), far_flow).await,
        o.seen_as(editor, kind("requested"), far_flow).await,
    );
    if seen_observed != seen_requested {
        leaks.push(format!("update.kind: {observed:?} / {requested:?}"));
    }
    assert!(leaks.is_empty(), "{leaks:#?}");
}

/// INV-63 on the extension of a flow (F20): an author without the right on a flow that has
/// ended gets the answer of a flow that does not exist or is out of his reach. Red: the
/// plan refuses `flow.update.ended` before the rights, and refused after them the flow
/// ended would be a reference to an object not in force, `core.reference.unknown` for
/// whoever reads it where REQ-011-4 says `flow.update.ended` (task 002-29d5, to arbitrate).
#[kartenn_testing::pg_test]
#[ignore = "task 002-29d5: the refusal of an ended flow, to arbitrate"]
async fn an_ended_flow_answers_as_an_absent_one(pool: PgPool) {
    let o = Objects::new(&pool).await;
    let editor = o.holding(WRITE).await;
    let extend = |x| update(x, json!({ "end": later(30) }));
    let (_, expected) = o.seen_as(editor, extend(o.far_flow), o.far_flow).await;
    assert!(!expected.is_empty());
    for x in [Uuid::now_v7(), o.gone_flow] {
        let (answer, found) = o.seen_as(editor, extend(x), x).await;
        assert_eq!(found, expected, "{x}: {answer:?}");
    }
}

/// The answer to `POST /v1/changesets` of `operations`: its status and its bytes, the
/// identifiers of the changeset refused and of the problem (new at each call) masked.
async fn posted(app: &axum::Router, operations: &[Operation]) -> (StatusCode, String) {
    let body = json!({ "motive": "m", "operations": operations });
    let request = Request::post("/v1/changesets?approval=self")
        .header(header::AUTHORIZATION, "Bearer t")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let mut text = String::from_utf8(bytes.to_vec()).unwrap();
    let problem: Value = serde_json::from_str(&text).unwrap();
    for (pointer, mask) in [
        ("/changeset/id", "<changeset>"),
        ("/instance", "<instance>"),
    ] {
        if let Some(id) = problem.pointer(pointer).and_then(Value::as_str) {
            let id = id.trim_start_matches("urn:uuid:");
            text = text.replace(id, mask);
        }
    }
    (status, text)
}

/// INV-63 through HTTP (task 002-29d3): a waiver of a flow that does not exist, closed or
/// out of reach answers the same status and the same bytes, the identifier cited masked.
#[kartenn_testing::pg_test]
async fn a_hidden_flow_answers_the_same_bytes(pool: PgPool) {
    let o = Objects::new(&pool).await;
    let app = o.registry.app(o.holding(WAIVE).await);
    let mut answers = Vec::new();
    for x in [Uuid::now_v7(), o.far_prod, o.gone_flow, o.far_flow] {
        let (status, bytes) = posted(&app, &[waive(x)]).await;
        answers.push((status, bytes.replace(&x.to_string(), "<x>")));
    }
    assert_eq!(
        answers[0].0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{answers:?}"
    );
    assert!(answers.iter().all(|a| *a == answers[0]), "{answers:#?}");
}
