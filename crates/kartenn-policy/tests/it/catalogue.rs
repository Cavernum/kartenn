//! The objects of the catalogue of the policy (spec 010, REQ-010-4 to 9, 26, 33, 36, 37;
//! task 010-8): each rule of validation of the module, the undo of a changeset of rules,
//! and the event of a new revision.

use kartenn_service::{ObjectKind, Operation, Outcome, Ref};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::env::{Env, op, rules};

const TEXT: &str = "Interdit entre ces zones.";

/// The environments `keys` and a flow type `admin` with its service `ssh`.
async fn catalogue(env: &Env) {
    env.applied(
        ["prod", "dev", "global"]
            .iter()
            .map(|k| op("environment.create", json!({ "key": k })))
            .chain([
                op(
                    "flow_type.create",
                    json!({ "local": 1, "key": "admin", "label": "Administration" }),
                ),
                op(
                    "service.create",
                    json!({ "key": "ssh", "label": "SSH", "flow_type": { "local": 1 },
                            "entries": [{ "transport": "tcp", "ports": 22 }] }),
                ),
            ])
            .collect(),
    )
    .await;
}

async fn id_of(pool: &PgPool, table: &str, key: &str) -> Uuid {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT id FROM policy.{table} WHERE key = $1 AND valid_period @> now()"
    )))
    .bind(key)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A rule from `fields`, completed with its key, label and the flow type `admin`.
fn rule(admin: Uuid, fields: Value) -> Operation {
    let mut body = json!({ "key": "r1", "label": "R1", "effect": "allow",
                           "flow_types": [{ "id": admin }] });
    let Value::Object(fields) = fields else {
        panic!("fields of a rule")
    };
    for (k, v) in fields {
        body[k] = v;
    }
    op("rule.create", body)
}

/// Each rule of validation of the operations on the catalogue refuses its case, under its
/// own identifier (REQ-010-4 to 9, 33, 36, 37).
#[kartenn_testing::pg_test]
async fn each_rule_of_validation_refuses_its_case(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let admin = id_of(&pool, "policy_flow_type", "admin").await;
    let cases: Vec<(&str, Operation)> = vec![
        (
            "policy.key.syntax",
            op(
                "flow_type.create",
                json!({ "key": "Bad--Key", "label": "x" }),
            ),
        ),
        (
            "policy.label.invalid",
            op("flow_type.create", json!({ "key": "x", "label": "" })),
        ),
        (
            "policy.key.duplicate",
            op(
                "flow_type.create",
                json!({ "key": "admin", "label": "Again" }),
            ),
        ),
        (
            "policy.service.entry_invalid",
            op(
                "service.create",
                json!({ "key": "bad", "label": "Bad", "flow_type": { "id": admin },
                        "entries": [{ "transport": "tcp", "ports": "70000" }] }),
            ),
        ),
        (
            "policy.service.entries_empty",
            op(
                "service.create",
                json!({ "key": "bad", "label": "Bad", "flow_type": { "id": admin },
                        "entries": [] }),
            ),
        ),
        (
            "policy.service.flow_type",
            op(
                "service.create",
                json!({ "key": "bad", "label": "Bad",
                        "entries": [{ "transport": "udp", "ports": 53 }] }),
            ),
        ),
        (
            "policy.rule.flow_selector_empty",
            op(
                "rule.create",
                json!({ "key": "r1", "label": "R1", "effect": "allow" }),
            ),
        ),
        (
            "policy.rule.unknown_axis",
            rule(
                admin,
                json!({ "source": { "policy.exposure": ["internet"] } }),
            ),
        ),
        (
            "policy.rule.term_empty",
            rule(admin, json!({ "source": { "policy.environment": [] } })),
        ),
        (
            "policy.rule.conditional_deny",
            rule(
                admin,
                json!({ "effect": "deny", "required_patterns": [{ "id": admin }],
                        "texts": { "fr": { "body": TEXT } } }),
            ),
        ),
        (
            "policy.text.missing",
            rule(admin, json!({ "effect": "deny" })),
        ),
        (
            "policy.text.invalid",
            rule(
                admin,
                json!({ "effect": "deny", "texts": { "FR": { "body": TEXT } } }),
            ),
        ),
        (
            "policy.reference.closed",
            rule(
                admin,
                json!({ "destination": { "policy.environment": ["nowhere"] } }),
            ),
        ),
        (
            "policy.text.missing",
            op("pattern.create", json!({ "key": "p1", "label": "P1" })),
        ),
        (
            "policy.condition.params_invalid",
            op(
                "condition.create",
                json!({ "key": "c1", "kind": "dossier_rank_at_most",
                        "params": { "axis": "policy.sensitivity" },
                        "texts": { "fr": { "body": TEXT } } }),
            ),
        ),
        (
            "policy.signal.params_invalid",
            op(
                "signal.create",
                json!({ "key": "s1", "kind": "endpoint_value", "weight": 3,
                        "params": { "axis": "policy.environment", "values": [] } }),
            ),
        ),
        (
            "policy.operation.invalid",
            op(
                "flow_type.create",
                json!({ "key": "x", "label": "X", "colour": "red" }),
            ),
        ),
    ];
    for (expected, operation) in cases {
        let outcome = env.submit(vec![operation]).await;
        assert!(
            rules(&outcome).contains(&expected),
            "{expected}: {outcome:?}"
        );
    }
}

/// REQ-010-7: a service of more than 1 024 ports is accepted with a warning.
#[kartenn_testing::pg_test]
async fn a_wide_service_warns(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let admin = id_of(&pool, "policy_flow_type", "admin").await;
    let outcome = env
        .applied(vec![op(
            "service.create",
            json!({ "key": "wide", "label": "Wide", "flow_type": { "id": admin },
                    "entries": [{ "transport": "tcp", "ports": "1-2000" }] }),
        )])
        .await;
    let Outcome::Applied { warnings, .. } = outcome else {
        unreachable!()
    };
    assert!(
        warnings
            .iter()
            .any(|w| w.rule.id == "policy.service.wide_ports"),
        "{warnings:?}"
    );
}

/// A rule cites flow types, services, patterns and environments; what it cites cannot be
/// closed under it; modified, then closed, the policy rises a revision each time.
#[kartenn_testing::pg_test]
async fn a_rule_is_written_modified_and_closed(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let (admin, ssh) = (
        id_of(&pool, "policy_flow_type", "admin").await,
        id_of(&pool, "policy_service", "ssh").await,
    );
    env.applied(vec![
        op(
            "pattern.create",
            json!({ "local": 1, "key": "bastion", "label": "Bastion",
                    "texts": { "fr": { "body": "Passer par le bastion." } } }),
        ),
        rule(
            admin,
            json!({ "source": { "policy.environment": ["prod"] },
                    "destination": { "policy.environment": ["dev", "global"] },
                    "services": [{ "id": ssh }], "pattern": { "local": 1 } }),
        ),
        op(
            "condition.create",
            json!({ "key": "c1", "kind": "max_width", "params": { "addresses": 256 },
                    "flow_types": [{ "id": admin }],
                    "texts": { "fr": { "body": "Une extrémité de 256 adresses au plus." } } }),
        ),
        op(
            "signal.create",
            json!({ "key": "s1", "kind": "axis_differs", "weight": 5,
                    "params": { "axis": "policy.environment" } }),
        ),
        op("exempt_role.create", json!({ "role": "administration" })),
    ])
    .await;
    let r1 = id_of(&pool, "policy_rule", "r1").await;
    let services: Vec<Uuid> =
        sqlx::query_scalar("SELECT service_ids FROM policy.policy_rule WHERE id = $1")
            .bind(r1)
            .fetch_one(&pool)
            .await
            .map(|ids: Vec<Uuid>| ids)
            .unwrap();
    assert_eq!(services, [ssh]);
    // What the rule cites is not closed under it (D33).
    let flow_type = env
        .submit(vec![Operation::Close {
            kind: ObjectKind::from_name("policy.flow_type").unwrap(),
            id: Ref::Id(admin),
            expected: None,
            cascade: false,
        }])
        .await;
    assert!(
        matches!(flow_type, Outcome::Refused { .. }),
        "{flow_type:?}"
    );
    let before = env.revision().await;
    env.applied(vec![op(
        "rule.update",
        json!({ "id": { "id": r1 }, "effect": "deny",
                "texts": { "fr": { "body": TEXT }, "en": { "body": "Denied." } } }),
    )])
    .await;
    assert_eq!(env.revision().await, before + 1);
    let deny_without_text = env
        .submit(vec![op(
            "rule.update",
            json!({ "id": { "id": r1 }, "texts": { "en": { "body": "Denied." } } }),
        )])
        .await;
    assert_eq!(rules(&deny_without_text), ["policy.text.missing"]);
    // A key taken by another rule is refused before the write.
    env.applied(vec![rule(admin, json!({ "key": "r2" }))]).await;
    let taken = env
        .submit(vec![op(
            "rule.update",
            json!({ "id": { "id": r1 }, "key": "r2" }),
        )])
        .await;
    assert_eq!(rules(&taken), ["policy.key.duplicate"]);
    assert_eq!(env.revision().await, before + 2, "r2 written");
    env.applied(vec![Operation::Close {
        kind: ObjectKind::from_name("policy.rule").unwrap(),
        id: Ref::Id(r1),
        expected: None,
        cascade: false,
    }])
    .await;
    assert_eq!(env.revision().await, before + 3, "r1 closed");
}

/// REQ-010-26: the undo of a changeset of rules gives the policy back as it was.
#[kartenn_testing::pg_test]
async fn the_undo_of_a_changeset_of_rules(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let admin = id_of(&pool, "policy_flow_type", "admin").await;
    let Outcome::Applied { changeset, .. } = env
        .applied(vec![
            rule(admin, json!({ "key": "r1" })),
            rule(admin, json!({ "key": "r2" })),
        ])
        .await
    else {
        unreachable!()
    };
    let undo = kartenn_service::undo::inverse(&env.store, changeset, env.author)
        .await
        .unwrap()
        .unwrap();
    assert!(
        undo.conflicts.is_empty() && undo.unsupported.is_empty(),
        "{undo:?}"
    );
    env.applied(undo.request.operations).await;
    let living: i64 =
        sqlx::query_scalar("SELECT count(*) FROM policy.policy_rule WHERE valid_period @> now()")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(living, 0);
}

/// N15: a changeset that writes the policy emits `org.kartenn.policy.revised` once, with
/// the new revision; a refused one emits nothing.
#[kartenn_testing::pg_test]
async fn a_new_revision_is_announced(pool: PgPool) {
    let env = Env::new(&pool).await;
    let Outcome::Applied { changeset, .. } = env
        .applied(vec![
            op("flow_type.create", json!({ "key": "a", "label": "A" })),
            op("flow_type.create", json!({ "key": "b", "label": "B" })),
        ])
        .await
    else {
        unreachable!()
    };
    let events: Vec<Value> = sqlx::query_scalar(
        "SELECT payload FROM outbox WHERE changeset_id = $1
            AND payload->>'type' = 'org.kartenn.policy.revised'",
    )
    .bind(changeset)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["data"]["revision"], env.revision().await);
    let refused = env
        .submit(vec![op(
            "flow_type.create",
            json!({ "key": "a", "label": "A" }),
        )])
        .await;
    let Outcome::Refused { changeset, .. } = refused else {
        panic!("{refused:?}")
    };
    let none: i64 = sqlx::query_scalar("SELECT count(*) FROM outbox WHERE changeset_id = $1")
        .bind(changeset)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(none, 0);
}
