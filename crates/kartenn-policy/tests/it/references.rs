//! The references of the operations of the policy (REQ-002-12, amended; task 002-29d4):
//! each `Ref::Id` of each of its 22 operation types designates an object of its kind in
//! force at the start of the period, refused otherwise by `core.reference.unknown` naming
//! the field; and an author without the right on the object a reference names gets the same
//! answer whatever its identifier names (INV-63): nothing, an object of another kind, an
//! object closed, an object in force out of his reach.

use std::collections::BTreeSet;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_policy::Policy;
use kartenn_service::module::Module;
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use crate::support::env::{Env, op};

const UNKNOWN: &str = "core.reference.unknown";

/// The operation types without a reference by identifier: they name their objects by key.
const NO_REFERENCE: &[&str] = &[
    "policy.axis.designate",
    "policy.value.describe",
    "policy.environment.create",
    "policy.environment.close",
    "policy.examples.install",
    "policy.flow_type.create",
    "policy.pattern.create",
    "policy.signal.create",
    "policy.exempt_role.create",
];

/// The tables whose living rows a refused changeset leaves alone.
const TABLES: &[&str] = &[
    "policy.policy_axis",
    "policy.policy_value",
    "policy.policy_flow_type",
    "policy.policy_service",
    "policy.policy_pattern",
    "policy.policy_rule",
    "policy.policy_condition",
    "policy.policy_signal",
    "policy.policy_exempt_role",
    "policy.policy_inference_rule",
    "module_object",
];

/// The objects of the tests: of each kind a reference may name, one in force and one
/// closed; the entity `other`, the domain `far` it owns, a domain of `acme` closed.
struct World {
    env: Env,
    other: Uuid,
    far: Uuid,
    gone_domain: Uuid,
    axis: Uuid,
    gone_axis: Uuid,
    flow_type: Uuid,
    gone_flow_type: Uuid,
    service: Uuid,
    gone_service: Uuid,
    pattern: Uuid,
    gone_pattern: Uuid,
    rule: Uuid,
    gone_rule: Uuid,
    condition: Uuid,
    gone_condition: Uuid,
    signal: Uuid,
    gone_signal: Uuid,
    /// Rules of inference: without a domain, in `far`, in `lab`, in `lab` closed.
    inference: Uuid,
    far_inference: Uuid,
    lab_inference: Uuid,
    gone_inference: Uuid,
}

fn close(kind: &str, id: Uuid) -> Operation {
    Operation::Close {
        kind: ObjectKind::from_name(kind).unwrap(),
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    }
}

fn labels(text: &str) -> Value {
    json!({ "fr": text })
}

fn texts() -> Value {
    json!({ "fr": { "body": "Un texte." } })
}

fn entries() -> Value {
    json!([{ "transport": "tcp", "ports": 22 }])
}

/// A new key for each object an operation creates.
fn key() -> String {
    format!("k{}", Uuid::now_v7().simple())
}

impl World {
    #[allow(clippy::too_many_lines)] // one object of each kind, then its closed twin
    async fn new(pool: &PgPool) -> Self {
        let env = Env::new(pool).await;
        let owner = env.property("core.owner").await;
        let acme = env.id("entity", "acme").await;
        env.applied(vec![Operation::CreateEntity {
            local: None,
            slug: "other".into(),
            parent: None,
        }])
        .await;
        let other = env.id("entity", "other").await;
        let domain = |local: u32, slug: &str, owner_of: Uuid| {
            vec![
                Operation::CreateDomain {
                    local: Some(local),
                    slug: slug.into(),
                },
                Operation::AssignProperty {
                    object: Ref::Local(local),
                    kind: ObjectKind::Domain,
                    property: Ref::Id(owner),
                    values: vec![RawInput::Text(owner_of.to_string())],
                },
            ]
        };
        let mut domains = domain(1, "far", other);
        domains.extend(domain(2, "gone", acme));
        env.applied(domains).await;
        let (far, gone_domain) = (
            env.id("addressing_domain", "far").await,
            env.id("addressing_domain", "gone").await,
        );
        env.applied(vec![op("examples.install", json!({}))]).await;
        env.applied(vec![
            op("environment.create", json!({ "key": "prod" })),
            op("environment.create", json!({ "key": "dev" })),
        ])
        .await;
        let created = |name: &str, k: &str, extra: Value| {
            let mut body = json!({ "key": k });
            if let (Value::Object(body), Value::Object(extra)) = (&mut body, extra) {
                body.extend(extra);
            }
            op(&format!("{name}.create"), body)
        };
        let rule = |k: &str| {
            created(
                "rule",
                k,
                json!({ "labels": labels(k), "effect": "allow",
                        "flow_types": [{ "local": 1 }],
                        "source": { "policy.environment": ["prod"] },
                        "destination": { "policy.environment": ["dev"] } }),
            )
        };
        let inference = |k: &str, domain: Option<Uuid>| {
            let mut body = json!({ "key": k, "axis": "policy.environment", "values": ["prod"] });
            if let Some(domain) = domain {
                body["domain"] = json!({ "id": domain });
            }
            op("inference_rule.create", body)
        };
        for (suffix, first) in [("", true), ("-gone", false)] {
            let k = |name: &str| format!("{name}{suffix}");
            let mut batch = vec![
                created(
                    "flow_type",
                    &k("ft"),
                    json!({ "local": 1, "labels": labels("ft") }),
                ),
                created(
                    "service",
                    &k("svc"),
                    json!({ "labels": labels("svc"), "flow_type": { "local": 1 },
                            "entries": entries() }),
                ),
                created(
                    "pattern",
                    &k("pat"),
                    json!({ "labels": labels("pat"), "texts": texts() }),
                ),
                rule(&k("rl")),
                created(
                    "condition",
                    &k("cd"),
                    json!({ "kind": "max_width", "params": { "addresses": 256 },
                            "texts": texts() }),
                ),
                created(
                    "signal",
                    &k("sg"),
                    json!({ "kind": "axis_differs", "weight": 5,
                            "params": { "axis": "policy.environment" } }),
                ),
                inference(&k("ir"), None),
            ];
            if first {
                batch.push(inference("ir-far", Some(far)));
                batch.push(inference("ir-lab", Some(env.domain)));
            } else {
                batch.push(inference("ir-lab-gone", Some(env.domain)));
            }
            env.applied(batch).await;
        }
        let id = |table: &'static str, k: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, Uuid>(sqlx::AssertSqlSafe(format!(
                    "SELECT id FROM policy.{table} WHERE key = $1 AND valid_period @> now()"
                )))
                .bind(k)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        let axis_of = |property: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, Uuid>(
                    "SELECT id FROM policy.policy_axis
                      WHERE property_key = $1 AND valid_period @> now()",
                )
                .bind(property)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        let world = Self {
            other,
            far,
            gone_domain,
            axis: axis_of("policy.exposure").await,
            gone_axis: axis_of("policy.sensitivity").await,
            flow_type: id("policy_flow_type", "ft").await,
            gone_flow_type: id("policy_flow_type", "ft-gone").await,
            service: id("policy_service", "svc").await,
            gone_service: id("policy_service", "svc-gone").await,
            pattern: id("policy_pattern", "pat").await,
            gone_pattern: id("policy_pattern", "pat-gone").await,
            rule: id("policy_rule", "rl").await,
            gone_rule: id("policy_rule", "rl-gone").await,
            condition: id("policy_condition", "cd").await,
            gone_condition: id("policy_condition", "cd-gone").await,
            signal: id("policy_signal", "sg").await,
            gone_signal: id("policy_signal", "sg-gone").await,
            inference: id("policy_inference_rule", "ir").await,
            far_inference: id("policy_inference_rule", "ir-far").await,
            lab_inference: id("policy_inference_rule", "ir-lab").await,
            gone_inference: id("policy_inference_rule", "ir-lab-gone").await,
            env,
        };
        // The closed twins, their referrers first (D33).
        world
            .env
            .applied(vec![
                close("policy.rule", world.gone_rule),
                close("policy.service", world.gone_service),
                close("policy.condition", world.gone_condition),
                close("policy.signal", world.gone_signal),
                close("policy.pattern", world.gone_pattern),
                close("policy.inference_rule", world.gone_inference),
                close("policy.axis", world.gone_axis),
                close("domain", world.gone_domain),
            ])
            .await;
        world
            .env
            .applied(vec![
                close("policy.flow_type", world.gone_flow_type),
                close(
                    "policy.inference_rule",
                    id("policy_inference_rule", "ir-gone").await,
                ),
            ])
            .await;
        world
    }

    /// Every object of the world: identifiers a refusal may name as they are.
    fn fixtures(&self) -> BTreeSet<Uuid> {
        [
            self.env.domain,
            self.other,
            self.far,
            self.gone_domain,
            self.axis,
            self.gone_axis,
            self.flow_type,
            self.gone_flow_type,
            self.service,
            self.gone_service,
            self.pattern,
            self.gone_pattern,
            self.rule,
            self.gone_rule,
            self.condition,
            self.gone_condition,
            self.signal,
            self.gone_signal,
            self.inference,
            self.far_inference,
            self.lab_inference,
            self.gone_inference,
        ]
        .into_iter()
        .collect()
    }

    /// `operations` submitted by `identity` under the RBAC of the store.
    async fn submit_as(&self, identity: Uuid, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: identity,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        let authorizer = kartenn_store_pg::rbac::PgAuthorizer::new(self.env.pool.clone())
            .with_modules(self.env.registry.clone());
        submit(&self.env.store, &authorizer, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    /// A new person holding `on_acme` on `acme` and what it owns, `global` everywhere.
    async fn granted(&self, on_acme: &[&str], global: &[&str]) -> Uuid {
        let acme = self.env.id("entity", "acme").await;
        self.granted_on(("entity", acme), on_acme, global).await
    }

    /// A new person holding `scoped` on the scope `(kind, id)`, `global` everywhere.
    async fn granted_on(&self, scope: (&str, Uuid), scoped: &[&str], global: &[&str]) -> Uuid {
        let pool = &self.env.pool;
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name)
             VALUES (gen_random_uuid(), 'r-' || gen_random_uuid()) RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        for (permissions, scope) in [(scoped, Some(scope)), (global, None)] {
            if permissions.is_empty() {
                continue;
            }
            let role = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO role (id, key, labels, valid_period, changeset_id)
                 VALUES ($1, $2, jsonb_build_object('fr', 'r'),
                         tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
            )
            .bind(role)
            .bind(format!("r_{}", role.simple()))
            .execute(pool)
            .await
            .unwrap();
            for permission in permissions {
                sqlx::query(
                    "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
                     VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL),
                             gen_random_uuid())",
                )
                .bind(role)
                .bind(permission)
                .execute(pool)
                .await
                .unwrap();
            }
            sqlx::query(kartenn_testing::BIND_IDENTITY)
                .bind(identity)
                .bind(role)
                .bind(scope.map_or("global", |(kind, _)| kind))
                .bind(scope.map(|(_, id)| id))
                .execute(pool)
                .await
                .unwrap();
        }
        identity
    }

    /// The living rows of the tables of the module and of `module_object`, and the
    /// revisions of the policy.
    async fn written(&self) -> Vec<i64> {
        let mut counts = Vec::new();
        for table in TABLES {
            counts.push(
                sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                    "SELECT count(*) FROM {table} WHERE valid_period @> now()"
                )))
                .fetch_one(&self.env.pool)
                .await
                .unwrap(),
            );
        }
        let (policy, inference): (i64, i64) =
            sqlx::query_as("SELECT policy, inference FROM policy.policy_revision")
                .fetch_one(&self.env.pool)
                .await
                .unwrap();
        counts.extend([policy, inference]);
        counts
    }

    /// One case per field of each operation type that cites an object by identifier
    /// (`objects.rs` `Fields`, `handler.rs` `UpdateAxis`, `matrix.rs` `Set`, `infer.rs`
    /// `plan_rule`).
    #[allow(clippy::too_many_lines)] // the table of the cases
    fn fields(&self) -> Vec<Cited> {
        let ft = self.flow_type;
        let one = |x: Uuid| json!({ "id": x });
        let list = |x: Uuid| json!([{ "id": x }]);
        let rule = move |extra: Value| {
            let mut body = json!({ "labels": labels("r"), "effect": "allow",
                                   "flow_types": [{ "id": ft }],
                                   "source": { "policy.environment": ["prod"] },
                                   "destination": { "policy.environment": ["dev"] } });
            merge(&mut body, extra);
            body
        };
        let condition = |extra: Value| {
            let mut body = json!({ "kind": "max_width", "params": { "addresses": 256 },
                                   "texts": texts() });
            merge(&mut body, extra);
            body
        };
        let cell = move |extra: Value| {
            let mut body = json!({ "source": "prod", "destination": "dev",
                                   "flow_type": { "id": ft }, "state": "allow" });
            merge(&mut body, extra);
            body
        };
        let inference = |extra: Value| {
            let mut body = json!({ "axis": "policy.environment", "values": ["prod"] });
            merge(&mut body, extra);
            body
        };
        let (rl, svc, cd) = (self.rule, self.service, self.condition);
        let (axis, flow_type, service, pattern) =
            (self.axis, self.flow_type, self.service, self.pattern);
        let lab = self.env.domain;
        let gone = |kind: &str| match kind {
            "policy.axis" => self.gone_axis,
            "policy.flow_type" => self.gone_flow_type,
            "policy.service" => self.gone_service,
            "policy.pattern" => self.gone_pattern,
            "policy.rule" => self.gone_rule,
            "policy.condition" => self.gone_condition,
            "policy.signal" => self.gone_signal,
            "policy.inference_rule" => self.gone_inference,
            "domain" => self.gone_domain,
            other => panic!("{other}"),
        };
        // An object in force of another kind.
        let other = |kind: &str| match kind {
            "policy.flow_type" => self.pattern,
            _ => self.flow_type,
        };
        let case = |tag: &'static str,
                    field: &'static str,
                    kind: &'static str,
                    live: Uuid,
                    make: Box<dyn Fn(Uuid) -> Operation>| Cited {
            tag,
            field,
            kind,
            make,
            live,
            other: other(kind),
            closed: gone(kind),
        };
        let with = move |name: &'static str, body: Box<dyn Fn(Uuid) -> Value>| {
            Box::new(move |x: Uuid| op(name, body(x))) as Box<dyn Fn(Uuid) -> Operation>
        };
        let keyed = |mut body: Value| {
            body["key"] = key().into();
            body
        };
        vec![
            case(
                "policy.axis.update",
                "id",
                "policy.axis",
                axis,
                with(
                    "axis.update",
                    Box::new(move |x| json!({ "id": one(x), "inference": true })),
                ),
            ),
            case(
                "policy.flow_type.update",
                "id",
                "policy.flow_type",
                flow_type,
                with(
                    "flow_type.update",
                    Box::new(move |x| json!({ "id": one(x), "description": "d" })),
                ),
            ),
            case(
                "policy.service.create",
                "flow_type",
                "policy.flow_type",
                flow_type,
                with(
                    "service.create",
                    Box::new(move |x| {
                        keyed(json!({ "labels": labels("s"), "flow_type": one(x),
                                      "entries": entries() }))
                    }),
                ),
            ),
            case(
                "policy.service.update",
                "id",
                "policy.service",
                service,
                with(
                    "service.update",
                    Box::new(move |x| json!({ "id": one(x), "entries": entries() })),
                ),
            ),
            case(
                "policy.service.update",
                "flow_type",
                "policy.flow_type",
                flow_type,
                with(
                    "service.update",
                    Box::new(move |x| json!({ "id": one(svc), "flow_type": one(x) })),
                ),
            ),
            case(
                "policy.pattern.update",
                "id",
                "policy.pattern",
                pattern,
                with(
                    "pattern.update",
                    Box::new(move |x| json!({ "id": one(x), "texts": texts() })),
                ),
            ),
            case(
                "policy.rule.create",
                "flow_types[]",
                "policy.flow_type",
                flow_type,
                with(
                    "rule.create",
                    Box::new(move |x| keyed(rule(json!({ "flow_types": list(x) })))),
                ),
            ),
            case(
                "policy.rule.create",
                "services[]",
                "policy.service",
                service,
                with(
                    "rule.create",
                    Box::new(move |x| keyed(rule(json!({ "services": list(x) })))),
                ),
            ),
            case(
                "policy.rule.create",
                "pattern",
                "policy.pattern",
                pattern,
                with(
                    "rule.create",
                    Box::new(move |x| keyed(rule(json!({ "pattern": one(x) })))),
                ),
            ),
            case(
                "policy.rule.create",
                "required_patterns[]",
                "policy.pattern",
                pattern,
                with(
                    "rule.create",
                    Box::new(move |x| keyed(rule(json!({ "required_patterns": list(x) })))),
                ),
            ),
            case(
                "policy.rule.update",
                "id",
                "policy.rule",
                rl,
                with(
                    "rule.update",
                    Box::new(move |x| json!({ "id": one(x), "texts": texts() })),
                ),
            ),
            case(
                "policy.rule.update",
                "flow_types[]",
                "policy.flow_type",
                flow_type,
                with(
                    "rule.update",
                    Box::new(move |x| json!({ "id": one(rl), "flow_types": list(x) })),
                ),
            ),
            case(
                "policy.rule.update",
                "services[]",
                "policy.service",
                service,
                with(
                    "rule.update",
                    Box::new(move |x| json!({ "id": one(rl), "services": list(x) })),
                ),
            ),
            case(
                "policy.rule.update",
                "pattern",
                "policy.pattern",
                pattern,
                with(
                    "rule.update",
                    Box::new(move |x| json!({ "id": one(rl), "pattern": one(x) })),
                ),
            ),
            case(
                "policy.rule.update",
                "required_patterns[]",
                "policy.pattern",
                pattern,
                with(
                    "rule.update",
                    Box::new(move |x| json!({ "id": one(rl), "required_patterns": list(x) })),
                ),
            ),
            case(
                "policy.condition.create",
                "rules[]",
                "policy.rule",
                rl,
                with(
                    "condition.create",
                    Box::new(move |x| keyed(condition(json!({ "rules": list(x) })))),
                ),
            ),
            case(
                "policy.condition.create",
                "flow_types[]",
                "policy.flow_type",
                flow_type,
                with(
                    "condition.create",
                    Box::new(move |x| keyed(condition(json!({ "flow_types": list(x) })))),
                ),
            ),
            case(
                "policy.condition.update",
                "id",
                "policy.condition",
                cd,
                with(
                    "condition.update",
                    Box::new(move |x| json!({ "id": one(x), "texts": texts() })),
                ),
            ),
            case(
                "policy.condition.update",
                "rules[]",
                "policy.rule",
                rl,
                with(
                    "condition.update",
                    Box::new(move |x| json!({ "id": one(cd), "rules": list(x) })),
                ),
            ),
            case(
                "policy.condition.update",
                "flow_types[]",
                "policy.flow_type",
                flow_type,
                with(
                    "condition.update",
                    Box::new(move |x| json!({ "id": one(cd), "flow_types": list(x) })),
                ),
            ),
            case(
                "policy.signal.update",
                "id",
                "policy.signal",
                self.signal,
                with(
                    "signal.update",
                    Box::new(move |x| json!({ "id": one(x), "weight": 4 })),
                ),
            ),
            case(
                "policy.matrix.set",
                "flow_type",
                "policy.flow_type",
                flow_type,
                with(
                    "matrix.set",
                    Box::new(move |x| cell(json!({ "flow_type": one(x) }))),
                ),
            ),
            case(
                "policy.matrix.set",
                "patterns[]",
                "policy.pattern",
                pattern,
                with(
                    "matrix.set",
                    Box::new(move |x| cell(json!({ "state": "conditional", "patterns": list(x) }))),
                ),
            ),
            // The cell emptied: the rule of the cell of the flow type in force is closed (the
            // cases above wrote it), a flow type absent has nothing to close.
            case(
                "policy.matrix.set",
                "flow_type",
                "policy.flow_type",
                flow_type,
                with(
                    "matrix.set",
                    Box::new(move |x| cell(json!({ "flow_type": one(x), "state": "none" }))),
                ),
            ),
            case(
                "policy.inference_rule.create",
                "domain",
                "domain",
                lab,
                with(
                    "inference_rule.create",
                    Box::new(move |x| keyed(inference(json!({ "domain": one(x) })))),
                ),
            ),
            case(
                "policy.inference_rule.create",
                "parent",
                "policy.inference_rule",
                self.lab_inference,
                with(
                    "inference_rule.create",
                    Box::new(move |x| {
                        keyed(inference(json!({ "domain": one(lab), "parent": one(x) })))
                    }),
                ),
            ),
            case(
                "policy.inference_rule.update",
                "id",
                "policy.inference_rule",
                self.lab_inference,
                with(
                    "inference_rule.update",
                    Box::new(move |x| json!({ "id": one(x), "priority": 2 })),
                ),
            ),
        ]
    }
}

/// A reference of an operation type: the operation citing `x` in `field`, an object in
/// force of `kind`, one of another kind, one closed.
struct Cited {
    tag: &'static str,
    field: &'static str,
    kind: &'static str,
    make: Box<dyn Fn(Uuid) -> Operation>,
    live: Uuid,
    other: Uuid,
    closed: Uuid,
}

fn merge(into: &mut Value, extra: Value) {
    if let (Value::Object(into), Value::Object(extra)) = (into, extra) {
        into.extend(extra);
    }
}

/// `text` with `x` written `<x>` and every identifier but `kept` written `<new>`: what a
/// refusal says once the identifier it was given, and those it draws (the changeset, the
/// request, the object an operation creates), are set aside.
fn masked(text: &str, x: Uuid, kept: &BTreeSet<Uuid>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(candidate) = rest.get(..36)
            && candidate.as_bytes()[8] == b'-'
            && let Ok(id) = Uuid::try_parse(candidate)
        {
            out.push_str(if id == x {
                "<x>"
            } else if kept.contains(&id) {
                candidate
            } else {
                "<new>"
            });
            rest = &rest[36..];
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// The refusal of `outcome` as its author sees it, every violation whole (rule, message,
/// operation, detail, objects), `x` and the identifiers drawn by the request masked.
fn seen(outcome: &Outcome, x: Uuid, kept: &BTreeSet<Uuid>) -> String {
    let Outcome::Refused { violations, .. } = outcome else {
        panic!("expected a refusal, got {outcome:?}")
    };
    let shown: Vec<Value> = violations
        .iter()
        .map(|v| {
            json!({ "rule": v.rule.id, "message": v.message, "operation": v.operation,
                    "detail": v.detail, "subjects": v.subjects })
        })
        .collect();
    masked(&Value::from(shown).to_string(), x, kept)
}

/// REQ-002-12 (amendment of 2026-11-14), task 002-29d4: every field of every operation of
/// the module that cites an object by identifier is read before it is written; an
/// identifier that names nothing, an object of another kind or one closed is refused by
/// `core.reference.unknown` alone, its message and its detail naming the field and the
/// kind, and nothing is written. A new operation type fails here until it has a case or is
/// listed without a reference.
#[kartenn_testing::pg_test]
async fn every_reference_of_every_operation_is_checked(pool: PgPool) {
    let world = World::new(&pool).await;
    let fields = world.fields();
    let types: BTreeSet<String> = Policy
        .operations()
        .iter()
        .map(|o| format!("policy.{}", o.name))
        .collect();
    assert_eq!(types.len(), 22, "{types:?}");
    let covered: BTreeSet<String> = fields
        .iter()
        .map(|f| f.tag.to_owned())
        .chain(NO_REFERENCE.iter().map(|&t| t.to_owned()))
        .collect();
    assert_eq!(covered, types, "an operation of the module has no case");
    let before = world.written().await;
    let mut wrong = Vec::new();
    for f in &fields {
        let operation = (f.make)(Uuid::nil());
        let Operation::Module(m) = &operation else {
            panic!("not an operation of the module: {operation:?}")
        };
        assert_eq!(m.tag(), f.tag);
        for x in [Uuid::now_v7(), f.other, f.closed] {
            let outcome = world.env.submit(vec![(f.make)(x)]).await;
            let expected = json!([{
                "rule": UNKNOWN,
                "message": format!(
                    "operation 0, `{}`: no {} {x} in force at the start of its period",
                    f.field, f.kind
                ),
                "operation": 0,
                "detail": { "field": f.field, "expected": f.kind },
                "subjects": [x],
            }])
            .to_string();
            let kept = BTreeSet::from([x]);
            let got = match &outcome {
                Outcome::Refused { .. } => seen(&outcome, Uuid::nil(), &kept),
                other => format!("{other:?}"),
            };
            if got != expected {
                wrong.push(format!("{} `{}` ({x}): {got}", f.tag, f.field));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert_eq!(world.written().await, before, "nothing is written");
}

/// INV-63 on the references of the module (task 002-29d4), for each field of each
/// operation type: an author without any right gets the refusal of his rights, the same
/// whole (rules, messages, operations, details, objects, in order) whatever the identifier
/// names. An author with every right to write but none to read the policy, his rules of
/// inference in the domain `lab`: where the reference needs a right (the domain of a rule
/// of inference, the rule an update writes), the refusal of his rights for an object in
/// force out of his reach, the same for every hidden one; elsewhere (an oracle of existence
/// left to task 002-31), the object in force applied and every hidden one refused alike by
/// `core.reference.unknown`, as one who reads would be.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // two authors, each compared on every field
async fn no_right_reaches_through_a_hidden_reference(pool: PgPool) {
    let world = World::new(&pool).await;
    let kept = world.fixtures();
    let nobody = world.granted(&[], &[]).await;
    let blind = world
        .granted_on(
            ("domain", world.env.domain),
            &["policy.inference.admin", "core.domain.read"],
            &[
                "policy.settings.edit",
                "policy.settings.approval.self",
                "policy.rule.admin",
            ],
        )
        .await;
    let mut leaks = Vec::new();
    for f in world.fields() {
        let name = format!("{} `{}`", f.tag, f.field);
        let hidden = [Uuid::now_v7(), f.other, f.closed];
        // The object in force out of reach of `blind`, where the reference needs a right.
        let far = match (f.tag, f.field) {
            ("policy.inference_rule.create", "domain") => Some(world.far),
            ("policy.inference_rule.update", "id") => Some(world.far_inference),
            _ => None,
        };
        // Without any right: the refusal of the rights, whatever `x` names.
        let reach = world.submit_as(nobody, vec![(f.make)(f.live)]).await;
        let expected = seen(&reach, f.live, &kept);
        assert!(!expected.contains(UNKNOWN), "{name}: {expected}");
        for x in hidden {
            let answer = world.submit_as(nobody, vec![(f.make)(x)]).await;
            let got = seen(&answer, x, &kept);
            if got != expected {
                leaks.push(format!("nobody, {name} ({x}): {got} / expected {expected}"));
            }
        }
        // Every right to write, none to read the policy: hidden first, the object in force
        // applied changes what follows.
        let answers: Vec<(Uuid, String)> = {
            let mut answers = Vec::new();
            for x in hidden {
                let answer = world.submit_as(blind, vec![(f.make)(x)]).await;
                answers.push((x, seen(&answer, x, &kept)));
            }
            answers
        };
        let expected = if let Some(far) = far {
            let refused = world.submit_as(blind, vec![(f.make)(far)]).await;
            let refused = seen(&refused, far, &kept);
            assert!(!refused.contains(UNKNOWN), "{name}: {refused}");
            refused
        } else if f.field == "id" {
            // The object an update writes, hidden: its write refuses it before the hidden
            // designations are, alike for each (task 002-29d4, `reste`).
            json!([{
                "rule": "changeset.operation.no_version",
                "message": "<x> does not exist at the targeted time",
                "operation": 0,
                "detail": null,
                "subjects": ["<x>"],
            }])
            .to_string()
        } else {
            json!([{
                "rule": UNKNOWN,
                "message": format!(
                    "operation 0, `{}`: no {} <x> in force at the start of its period",
                    f.field, f.kind
                ),
                "operation": 0,
                "detail": { "field": f.field, "expected": f.kind },
                "subjects": ["<x>"],
            }])
            .to_string()
        };
        for (x, got) in answers {
            if got != expected {
                leaks.push(format!("blind, {name} ({x}): {got} / expected {expected}"));
            }
        }
        let applied = world.submit_as(blind, vec![(f.make)(f.live)]).await;
        assert!(
            matches!(applied, Outcome::Applied { .. }),
            "{name}: {applied:?}"
        );
    }
    assert!(leaks.is_empty(), "{leaks:#?}");
}

/// The status and the body of `operations` submitted over HTTP by `identity`.
async fn posted(world: &World, identity: Uuid, operations: Value) -> (StatusCode, Vec<u8>) {
    let body = json!({ "motive": "test", "operations": operations });
    let request = Request::builder()
        .method(Method::POST)
        .uri(kartenn_testing::applied_by_author("POST", "/v1/changesets"))
        .header(header::AUTHORIZATION, "Bearer t")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = world.env.app(identity).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

/// INV-63 over HTTP (task 002-29d4): the domain of a rule of inference, which needs the
/// right to write there, answered by the same status and the same bytes whether it names
/// nothing, a domain closed or a domain in force out of the author's reach, once the
/// identifiers that differ by construction (the one given, the changeset and the request,
/// the rule the operation would create) are set aside; within reach, the rule is created.
#[kartenn_testing::pg_test]
async fn a_hidden_domain_answers_the_same_bytes(pool: PgPool) {
    let world = World::new(&pool).await;
    let kept = world.fixtures();
    let lab = world.env.domain;
    let author = world
        .granted_on(
            ("domain", lab),
            &["policy.inference.admin", "core.domain.read"],
            &[],
        )
        .await;
    let create = |domain: Uuid| {
        json!([{ "op": "policy.inference_rule.create", "key": key(),
                 "axis": "policy.environment", "values": ["prod"],
                 "domain": { "id": domain } }])
    };
    let mut answers = Vec::new();
    for x in [Uuid::now_v7(), world.gone_domain, world.far] {
        let (status, bytes) = posted(&world, author, create(x)).await;
        let text = String::from_utf8(bytes).unwrap();
        answers.push((status, masked(&text, x, &kept)));
    }
    assert_eq!(
        answers[0].0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answers[0].1
    );
    assert!(
        answers[0].1.contains("core.object.forbidden"),
        "{}",
        answers[0].1
    );
    assert!(!answers[0].1.contains(UNKNOWN), "{}", answers[0].1);
    for (i, answer) in answers.iter().enumerate().skip(1) {
        assert_eq!(answer, &answers[0], "{i}");
    }
    let (status, bytes) = posted(&world, author, create(lab)).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
}
