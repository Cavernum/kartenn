//! The preview of the inference in the differential of a changeset (spec 010, REQ-010-55,
//! S36; task 010-22): the nodes whose expectation changes, the deviations opened and
//! closed, in a simulation and without any anomaly; a rule outranked changes nothing; a
//! tie shows its candidates; a future rule is evaluated at its start; a scope too wide
//! gives its count alone; the simulation asks for the rights of the write.

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::authz::{Grants, Unrestricted};
use kartenn_service::module::DiffRow;
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref, submit};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::env::{Env, op};

const SECTION: &str = "policy.combinations";
pub(crate) const NETWORK: &str = "10.9.0.0/24";

/// The environments `prod`, `dev` and `rec`, a network of 11 addresses (12 nodes), 3 of
/// them declared `dev`; the identifiers of the network and of the addresses.
pub(crate) async fn tree(env: &Env) -> (Uuid, Vec<Uuid>) {
    env.applied(
        ["prod", "dev", "rec"]
            .iter()
            .map(|k| op("environment.create", json!({ "key": k })))
            .collect(),
    )
    .await;
    let domain = Ref::Id(env.domain);
    let mut operations = vec![Operation::CreatePrefix {
        local: Some(1),
        domain,
        prefix: NETWORK.into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    }];
    for i in 1..=11 {
        operations.push(Operation::CreateAddress {
            local: Some(1 + i),
            domain,
            address: format!("10.9.0.{i}"),
            usage: AddressUsage::Host,
        });
    }
    env.applied(operations).await;
    let network: Uuid = sqlx::query_scalar(
        "SELECT id FROM prefix WHERE prefix = $1::cidr AND valid_period @> now()",
    )
    .bind(NETWORK)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    let addresses: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM address WHERE address <<= $1::cidr AND valid_period @> now()
          ORDER BY address",
    )
    .bind(NETWORK)
    .fetch_all(&env.pool)
    .await
    .unwrap();
    let environment = env.property("policy.environment").await;
    env.applied(
        addresses[..3]
            .iter()
            .map(|a| Operation::AssignProperty {
                object: Ref::Id(*a),
                kind: ObjectKind::Address,
                property: Ref::Id(environment),
                values: vec![RawInput::Text("dev".into())],
            })
            .collect(),
    )
    .await;
    (network, addresses)
}

fn rule(key: &str, value: &str, priority: i64) -> Operation {
    op(
        "inference_rule.create",
        json!({ "key": key, "axis": "policy.environment", "priority": priority,
                "criteria": [{ "within": [NETWORK] }], "values": [value] }),
    )
}

/// The rows of the preview in a simulation of `operations` from `start`, if given.
async fn simulated(env: &Env, operations: Vec<Operation>, start: Option<i64>) -> Vec<DiffRow> {
    let mut request = env.request(operations, start);
    request.simulate = true;
    let outcome = submit(&env.store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    let Outcome::Simulated { sections, .. } = outcome else {
        panic!("not simulated: {outcome:?}");
    };
    sections
        .into_iter()
        .find(|s| s.name == SECTION)
        .map(|s| s.rows)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.kind.starts_with("policy.inference."))
        .collect()
}

fn summary(rows: &[DiffRow]) -> &Value {
    let found = rows
        .iter()
        .find(|r| r.kind == "policy.inference.summary")
        .unwrap_or_else(|| panic!("no summary: {rows:?}"));
    found.after.as_ref().unwrap()
}

fn nodes(rows: &[DiffRow]) -> Vec<&DiffRow> {
    rows.iter()
        .filter(|r| r.kind == "policy.inference.node")
        .collect()
}

async fn anomalies(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM anomaly WHERE rule_id LIKE 'policy.inference.%'")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// REQ-010-55: a rule `prod` on 12 nodes without expectation, 3 declared `dev`: 12 nodes
/// changed, 3 deviations opened on 3 origins, nothing written and no anomaly; applied,
/// its closure closes the 3 deviations.
#[kartenn_testing::pg_test]
async fn a_rule_opens_and_its_closure_closes_the_deviations(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (network, addresses) = tree(&env).await;

    let rows = simulated(&env, vec![rule("r-prod", "prod", 0)], None).await;
    let after = summary(&rows);
    assert_eq!(after["axis"], "policy.environment");
    assert_eq!(after["scope"], 12, "{after}");
    assert_eq!(after["scope_exact"], true);
    assert_eq!(after["truncated"], false);
    assert_eq!(after["changed"], 12, "{after}");
    assert_eq!(after["opens"], 3);
    assert_eq!(after["closes"], 0);
    assert_eq!(after["origins_opened"], 3);
    assert_eq!(after["origins_closed"], 0);
    let changed = nodes(&rows);
    assert_eq!(changed.len(), 12);
    // The deviations opened first, in the order of the tree; the network before its
    // addresses among the others.
    let first: Vec<Uuid> = changed[..3].iter().map(|r| r.object).collect();
    assert_eq!(first, addresses[..3]);
    assert_eq!(changed[3].object, network);
    let opened = changed[0];
    let (before, after) = (
        opened.before.as_ref().unwrap(),
        opened.after.as_ref().unwrap(),
    );
    assert_eq!(before["expected"], Value::Null);
    assert_eq!(before["deviates"], false);
    assert_eq!(after["expected"], json!(["prod"]));
    assert_eq!(after["rule"], "r-prod");
    assert_eq!(after["declared"], json!(["dev"]));
    assert_eq!(after["origin"], addresses[0].to_string());
    assert_eq!(after["deviates"], true);
    assert_eq!(
        after["revision"].as_i64().unwrap(),
        before["revision"].as_i64().unwrap() + 1
    );
    let unclassified = changed[4].after.as_ref().unwrap();
    assert_eq!(unclassified["declared"], Value::Null, "{unclassified}");
    assert_eq!(unclassified["deviates"], false);
    let written: i64 = sqlx::query_scalar("SELECT count(*) FROM policy.policy_inference_rule")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(written, 0, "a simulation writes nothing");
    assert_eq!(anomalies(&pool).await, 0);

    env.applied(vec![rule("r-prod", "prod", 0)]).await;
    let id: Uuid = sqlx::query_scalar(
        "SELECT id FROM policy.policy_inference_rule WHERE key = 'r-prod'
          AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let close = Operation::Close {
        kind: ObjectKind::from_name("policy.inference_rule").unwrap(),
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    };
    let rows = simulated(&env, vec![close], None).await;
    let after = summary(&rows);
    assert_eq!(after["changed"], 12, "{after}");
    assert_eq!(after["closes"], 3);
    assert_eq!(after["opens"], 0);
    // No open anomaly to close: the background task has not run.
    assert_eq!(after["origins_closed"], 0);
    assert_eq!(anomalies(&pool).await, 0);
}

/// A rule of lower priority than one that covers the network already changes nothing;
/// one of the same priority at the same depth gives a tie: its candidates, no deviation.
#[kartenn_testing::pg_test]
async fn an_outranked_rule_changes_nothing_and_a_tie_shows_its_candidates(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (_, addresses) = tree(&env).await;
    env.applied(vec![rule("r-prod", "prod", 5)]).await;

    let rows = simulated(&env, vec![rule("r-rec", "rec", 1)], None).await;
    let after = summary(&rows);
    assert_eq!(after["changed"], 0, "{after}");
    assert_eq!(after["scope"], 12);
    assert!(nodes(&rows).is_empty(), "{rows:?}");

    let rows = simulated(&env, vec![rule("r-rec", "rec", 5)], None).await;
    let after = summary(&rows);
    assert_eq!(after["changed"], 12, "{after}");
    assert_eq!(after["closes"], 3, "the dev nodes deviate no more");
    let row = nodes(&rows)
        .into_iter()
        .find(|r| r.object == addresses[0])
        .unwrap();
    let after = row.after.as_ref().unwrap();
    assert_eq!(after["expected"], Value::Null);
    let mut candidates: Vec<Value> = after["candidates"].as_array().unwrap().clone();
    candidates.sort_by_key(ToString::to_string);
    assert_eq!(candidates, [json!(["prod"]), json!(["rec"])]);
    assert_eq!(after["deviates"], false);
}

/// A rule that starts tomorrow is evaluated at its start, not now.
#[kartenn_testing::pg_test]
async fn a_future_rule_is_evaluated_at_its_start(pool: PgPool) {
    let env = Env::new(&pool).await;
    tree(&env).await;
    let tomorrow: i64 = sqlx::query_scalar(
        "SELECT (extract(epoch FROM date_trunc('second', now() + interval '1 day'))
                 * 1000000)::bigint",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let rows = simulated(&env, vec![rule("r-prod", "prod", 0)], Some(tomorrow)).await;
    let after = summary(&rows);
    let date: String = sqlx::query_scalar(
        "SELECT to_char(to_timestamp($1 / 1000000.0) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS')",
    )
    .bind(tomorrow)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        after["valid_at"].as_str().unwrap().starts_with(&date),
        "{after} at {date}"
    );
    assert_eq!(after["changed"], 12);
}

/// Beyond the bound of the scope (lowered here), the summary gives the count alone.
#[kartenn_testing::pg_test]
async fn a_scope_too_wide_gives_its_count_alone(pool: PgPool) {
    let env = Env::new(&pool).await;
    tree(&env).await;
    kartenn_policy::preview::set_scope_limit(5);
    let rows = simulated(&env, vec![rule("r-prod", "prod", 0)], None).await;
    let after = summary(&rows);
    assert_eq!(after["truncated"], true, "{after}");
    assert_eq!(after["scope_exact"], false);
    assert_eq!(after["scope"], 6, "the count stops past the bound");
    assert_eq!(after["changed"], Value::Null);
    assert!(nodes(&rows).is_empty());
}

/// The simulation asks for the rights of the write: without `policy.inference.admin`,
/// refused as the write.
#[kartenn_testing::pg_test]
async fn the_preview_needs_the_administration_of_the_inference(pool: PgPool) {
    let env = Env::new(&pool).await;
    tree(&env).await;
    let mut request = env.request(vec![rule("r-prod", "prod", 0)], None);
    request.simulate = true;
    let without = Grants::new().everywhere(env.author, "policy.rule.admin");
    let refused = submit(&env.store, &without, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    // A simulation answers its violations (REQ-004-9): the refusal of the write, and no
    // section, so no preview.
    let Outcome::Simulated {
        violations,
        sections,
        ..
    } = &refused
    else {
        panic!("not simulated: {refused:?}");
    };
    let found: Vec<&str> = violations.iter().map(|v| v.rule.id).collect();
    assert_eq!(found, ["core.object.forbidden"]);
    assert!(sections.is_empty(), "{sections:?}");
    let with = Grants::new().everywhere(env.author, "policy.inference.admin");
    let simulated = submit(&env.store, &with, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(
        matches!(simulated, Outcome::Simulated { .. }),
        "{simulated:?}"
    );
}

/// A small deterministic generator, for the oracle.
struct Draw(u64);

impl Draw {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % n as u64).unwrap()
    }
}

/// What the source expects for `node`, as sets of values: the oracle's view.
async fn expectation(env: &Env, property: Uuid, node: Uuid) -> Vec<Vec<String>> {
    use kartenn_service::expected::{Expected, Sources, suggest};
    let sources = Sources::none().with(std::sync::Arc::new(kartenn_policy::infer::InferenceSource));
    let sets = |s: &kartenn_service::expected::Suggestion| {
        let mut values: Vec<String> = s
            .values
            .iter()
            .map(|v| match v {
                RawInput::Text(t) => t.clone(),
                other => format!("{other:?}"),
            })
            .collect();
        values.sort();
        values
    };
    let mut found = match suggest(&env.store, &sources, property, node, None)
        .await
        .unwrap()
    {
        Expected::None => Vec::new(),
        Expected::One(s) => vec![sets(&s)],
        Expected::Candidates(all) => all.iter().map(sets).collect(),
    };
    found.sort();
    found
}

/// The oracle (design « Stratégie de test ») on small random trees: the nodes a
/// simulation shows changed are those whose expectation, read node by node over the
/// whole domain before and after the rule is applied, differs.
#[kartenn_testing::pg_test]
async fn the_changed_nodes_are_those_whose_expectation_differs(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.applied(
        ["prod", "dev", "rec"]
            .iter()
            .map(|k| op("environment.create", json!({ "key": k })))
            .collect(),
    )
    .await;
    let property = env.property("policy.environment").await;
    let (owner, acme) = (
        env.property("core.owner").await,
        env.id("entity", "acme").await,
    );
    for seed in 1..=4_u8 {
        let mut draw = Draw(0x9e37_79b9_7f4a_7c15 ^ u64::from(seed));
        let slug = format!("oracle{seed}");
        env.applied(vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: slug.clone(),
            },
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(acme.to_string())],
            },
        ])
        .await;
        let domain = env.id("addressing_domain", &slug).await;
        let mut prefixes = Vec::new();
        let mut operations = Vec::new();
        let mut local = 0;
        for c in 0..2_u8 {
            let container = format!("10.{seed}.{}.0/22", c * 4);
            prefixes.push(container.clone());
            local += 1;
            operations.push(Operation::CreatePrefix {
                local: Some(local),
                domain: Ref::Id(domain),
                prefix: container,
                nature: Nature::Container,
                point_to_point: false,
                anycast: false,
            });
            for n in 0..2_u8 {
                let network = format!("10.{seed}.{}.0/24", c * 4 + n);
                prefixes.push(network.clone());
                local += 1;
                operations.push(Operation::CreatePrefix {
                    local: Some(local),
                    domain: Ref::Id(domain),
                    prefix: network,
                    nature: Nature::Network,
                    point_to_point: false,
                    anycast: false,
                });
                for a in 1..=3_u8 {
                    local += 1;
                    operations.push(Operation::CreateAddress {
                        local: Some(local),
                        domain: Ref::Id(domain),
                        address: format!("10.{seed}.{}.{a}", c * 4 + n),
                        usage: AddressUsage::Host,
                    });
                }
            }
        }
        env.applied(operations).await;
        let values = ["prod", "dev", "rec"];
        let rule = |key: String, draw: &mut Draw| {
            op(
                "inference_rule.create",
                json!({ "key": key, "axis": "policy.environment",
                        "priority": draw.below(3), "domain": { "id": domain },
                        "criteria": [{ "within": [prefixes[draw.below(prefixes.len())]] }],
                        "values": [values[draw.below(values.len())]] }),
            )
        };
        let existing: Vec<Operation> = (0..3)
            .map(|i| rule(format!("s{seed}-r{i}"), &mut draw))
            .collect();
        env.applied(existing).await;
        let new = rule(format!("s{seed}-new"), &mut draw);
        let tree: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()
             UNION ALL SELECT id FROM address WHERE domain_id = $1 AND valid_period @> now()
             UNION ALL SELECT $1",
        )
        .bind(domain)
        .fetch_all(&pool)
        .await
        .unwrap();
        let mut before = Vec::new();
        for node in &tree {
            before.push(expectation(&env, property, *node).await);
        }
        let rows = simulated(&env, vec![new.clone()], None).await;
        let mut shown: Vec<Uuid> = nodes(&rows).iter().map(|r| r.object).collect();
        shown.sort();
        env.applied(vec![new]).await;
        let mut oracle = Vec::new();
        for (node, was) in tree.iter().zip(&before) {
            if &expectation(&env, property, *node).await != was {
                oracle.push(*node);
            }
        }
        oracle.sort();
        assert_eq!(shown, oracle, "seed {seed}: {rows:?}");
    }
}
