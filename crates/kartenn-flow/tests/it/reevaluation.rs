//! The re-evaluation of the flows a changeset touches, in its transaction, and the section
//! `flow.flows` of its differential (task 011-5; REQ-011-11, 12, 13, 23).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_flow::reevaluation::{SECTION, drift};
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{PeriodSpec, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref, submit};
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::requests::{Env, flow, policy};

fn prefix(local: u32, domain: Uuid, prefix: &str, nature: Nature) -> Operation {
    Operation::CreatePrefix {
        local: Some(local),
        domain: Ref::Id(domain),
        prefix: prefix.into(),
        nature,
        point_to_point: false,
        anycast: false,
    }
}

fn host(domain: Uuid, address: &str) -> Operation {
    Operation::CreateAddress {
        local: None,
        domain: Ref::Id(domain),
        address: address.into(),
        usage: AddressUsage::Host,
    }
}

fn classify(object: Ref, property: Uuid, environment: &str) -> Operation {
    Operation::AssignProperty {
        object,
        kind: ObjectKind::Prefix,
        property: Ref::Id(property),
        values: vec![RawInput::Text(environment.into())],
    }
}

async fn id_of(pool: &PgPool, sql: &str, key: &str) -> Uuid {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn address(pool: &PgPool, address: &str) -> Uuid {
    id_of(
        pool,
        "SELECT id FROM address WHERE address = $1::inet",
        address,
    )
    .await
}

/// The summary of `flow` now.
async fn summary(pool: &PgPool, flow: Uuid) -> String {
    sqlx::query_scalar(
        "SELECT summary FROM flow.flow_summary WHERE flow_id = $1 AND valid_period @> now()",
    )
    .bind(flow)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn flow_between(pool: &PgPool, source: Uuid, destination: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM flow.flow WHERE source_id = $1 AND destination_id = $2")
        .bind(source)
        .bind(destination)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The oracle: the flows whose stored evaluation differs from a full recompute.
async fn drifting(pool: &PgPool) -> Vec<Uuid> {
    let mut conn = pool.acquire().await.unwrap();
    drift(&mut conn).await.unwrap()
}

/// REQ-011-11: a container `{dev}` inserted above the source `{prod}` of an allowed flow
/// (prod to dev) makes it denied in the changeset that inserts it, which is accepted
/// (REQ-011-12).
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_11_a_container_above_the_source(pool: PgPool) {
    let env = Env::new(&pool).await;
    let environment = env.property("policy.environment").await;
    env.applied(vec![
        prefix(1, env.domain, "10.1.0.0/16", Nature::Container),
        classify(Ref::Local(1), environment, "prod"),
        prefix(2, env.domain, "10.1.1.0/24", Nature::Network),
        host(env.domain, "10.1.1.1"),
    ])
    .await;
    let source = address(&pool, "10.1.1.1").await;
    let destination = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    env.applied(vec![flow("request", env.fields(source, destination).await)])
        .await;
    let id = flow_between(&pool, source, destination).await;
    assert_eq!(summary(&pool, id).await, "allowed");
    env.applied(vec![
        prefix(1, env.domain, "10.1.0.0/20", Nature::Container),
        classify(Ref::Local(1), environment, "dev"),
    ])
    .await;
    assert_eq!(summary(&pool, id).await, "denied");
    let registered: String =
        sqlx::query_scalar("SELECT summary FROM flow.flow_registration WHERE flow_id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(registered, "allowed", "the registration stays frozen");
    assert!(drifting(&pool).await.is_empty());
}

/// REQ-011-13: removing the rule three flows depend on, simulated, announces three flows
/// `allowed → denied` with their counts, and writes nothing; applied, they are denied.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_13_the_differential_of_the_flows(pool: PgPool) {
    let env = Env::new(&pool).await;
    let source = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let mut flows = Vec::new();
    for n in 1..=3 {
        let destination = env
            .address(&format!("10.2.{n}.0/24"), &format!("10.2.{n}.1"), "dev")
            .await;
        env.applied(vec![flow("request", env.fields(source, destination).await)])
            .await;
        flows.push(flow_between(&pool, source, destination).await);
    }
    let rule = id_of(
        &pool,
        "SELECT id FROM policy.policy_rule WHERE key = $1",
        "prod-dev",
    )
    .await;
    let close = Operation::Close {
        kind: ObjectKind::from_name("policy.rule").unwrap(),
        id: Ref::Id(rule),
        expected: None,
        cascade: false,
    };
    let mut request = env.request(env.author, vec![close.clone()]);
    request.simulate = true;
    let outcome = submit(&env.store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    let Outcome::Simulated { sections, .. } = outcome else {
        panic!("not simulated: {outcome:?}");
    };
    let section = sections.iter().find(|s| s.name == SECTION).unwrap();
    assert_eq!(section.count, 4);
    assert_eq!(
        section.rows[0].after,
        Some(json!({ "flows": 3, "breaks": 3, "heals": 0, "outcome": 3 }))
    );
    for row in &section.rows[1..] {
        assert!(flows.contains(&row.object));
        assert_eq!(row.before.as_ref().unwrap()["summary"], "allowed");
        assert_eq!(row.after.as_ref().unwrap()["summary"], "denied");
        assert_eq!(row.after.as_ref().unwrap()["decisive"], "no_rule");
    }
    for id in &flows {
        assert_eq!(
            summary(&pool, *id).await,
            "allowed",
            "a simulation writes nothing"
        );
    }
    env.applied(vec![close]).await;
    for id in &flows {
        assert_eq!(summary(&pool, *id).await, "denied");
    }
    assert!(drifting(&pool).await.is_empty());
}

/// A write of the policy that changes no verdict re-evaluates the flows but rewrites none
/// of them, and adds no section: neither does a change the policy never reads.
#[kartenn_testing::pg_test]
async fn an_unchanged_evaluation_is_not_rewritten(pool: PgPool) {
    let env = Env::new(&pool).await;
    let source = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let destination = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    env.applied(vec![flow("request", env.fields(source, destination).await)])
        .await;
    let versions = || async {
        let n: i64 = sqlx::query_scalar(
            "SELECT (SELECT count(*) FROM flow.flow_evaluation_history)
                  + (SELECT count(*) FROM flow.flow_summary_history)
                  + (SELECT count(*) FROM flow.flow_endpoint_address_history)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        n
    };
    let before = versions().await;
    let mut request = env.request(
        env.author,
        vec![policy("environment.create", json!({ "key": "test" }))],
    );
    request.simulate = true;
    let outcome = submit(&env.store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    let Outcome::Simulated { sections, .. } = outcome else {
        panic!("not simulated: {outcome:?}");
    };
    assert!(sections.iter().all(|s| s.name != SECTION));
    env.applied(vec![policy("environment.create", json!({ "key": "test" }))])
        .await;
    assert_eq!(versions().await, before);
    assert!(drifting(&pool).await.is_empty());
}

/// A rule re-evaluates the flows of the services of its flow types only (task 011-10): a
/// stored evaluation made stale on purpose stays so after a rule of another flow type,
/// and is mended by a rule of its own.
#[kartenn_testing::pg_test]
async fn a_rule_reevaluates_the_flows_of_its_flow_type_only(pool: PgPool) {
    let env = Env::new(&pool).await;
    let source = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let destination = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    env.applied(vec![flow("request", env.fields(source, destination).await)])
        .await;
    let id = flow_between(&pool, source, destination).await;
    assert_eq!(summary(&pool, id).await, "allowed");
    let stale = || async {
        sqlx::query(
            "UPDATE flow.flow_summary SET summary = 'denied', allowed = 0, denied = 1
              WHERE flow_id = $1",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    };
    stale().await;
    let rule = |key: &str, flow_type: serde_json::Value| {
        policy(
            "rule.create",
            json!({ "key": key, "label": key, "effect": "allow", "flow_types": [flow_type],
                    "source": { "policy.environment": ["dev"] },
                    "destination": { "policy.environment": ["prod"] } }),
        )
    };
    env.applied(vec![
        policy(
            "flow_type.create",
            json!({ "local": 1, "key": "web", "label": "Web" }),
        ),
        policy(
            "service.create",
            json!({ "key": "http", "label": "HTTP", "flow_type": { "local": 1 },
                    "entries": [{ "transport": "tcp", "ports": 80 }] }),
        ),
        rule("dev-prod-web", json!({ "local": 1 })),
    ])
    .await;
    assert_eq!(
        summary(&pool, id).await,
        "denied",
        "another flow type: not read"
    );
    let admin = id_of(
        &pool,
        "SELECT id FROM policy.policy_flow_type WHERE key = $1",
        "admin",
    )
    .await;
    env.applied(vec![rule("dev-prod-admin", json!({ "id": admin }))])
        .await;
    assert_eq!(
        summary(&pool, id).await,
        "allowed",
        "its flow type: re-evaluated"
    );
    assert!(drifting(&pool).await.is_empty());
}

/// A generator of the suites, reproducible from its seed.
struct Suite(u64);

impl Suite {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).unwrap() % n
    }
}

/// The oracle (design 011 « Stratégie de test »): after each changeset of random suites
/// (classifications now and to come, rules created and closed, flows requested), the
/// stored evaluations equal a full recompute.
#[kartenn_testing::pg_test]
async fn the_reevaluation_agrees_with_a_full_recompute(pool: PgPool) {
    let env = Env::new(&pool).await;
    let environment = env.property("policy.environment").await;
    let admin = id_of(
        &pool,
        "SELECT id FROM policy.policy_flow_type WHERE key = $1",
        "admin",
    )
    .await;
    let mut networks = Vec::new();
    let mut addresses = Vec::new();
    for n in 1..=4 {
        let network = format!("10.{n}.0.0/24");
        let first = env
            .address(
                &network,
                &format!("10.{n}.0.1"),
                if n % 2 == 0 { "dev" } else { "prod" },
            )
            .await;
        env.applied(vec![host(env.domain, &format!("10.{n}.0.2"))])
            .await;
        addresses.push(first);
        addresses.push(address(&pool, &format!("10.{n}.0.2")).await);
        networks.push(
            id_of(
                &pool,
                "SELECT id FROM prefix WHERE prefix = $1::cidr",
                &network,
            )
            .await,
        );
    }
    let environments = ["prod", "dev"];
    for seed in [7_u64, 1_042, 31_337] {
        let mut suite = Suite(seed);
        let mut rules: Vec<Uuid> = Vec::new();
        for step in 0..12 {
            let operations = match suite.next(5) {
                0 => vec![classify(
                    Ref::Id(networks[suite.next(4)]),
                    environment,
                    environments[suite.next(2)],
                )],
                1 => {
                    let start: i64 = sqlx::query_scalar(
                        "SELECT (extract(epoch FROM now() + interval '1 day') * 1000000)::bigint",
                    )
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                    vec![Operation::Within {
                        period: PeriodSpec { start, end: None },
                        operation: Box::new(classify(
                            Ref::Id(networks[suite.next(4)]),
                            environment,
                            environments[suite.next(2)],
                        )),
                    }]
                }
                2 if !rules.is_empty() => {
                    let rule = rules.remove(suite.next(rules.len()));
                    vec![Operation::Close {
                        kind: ObjectKind::from_name("policy.rule").unwrap(),
                        id: Ref::Id(rule),
                        expected: None,
                        cascade: false,
                    }]
                }
                2 | 3 => {
                    let (from, to) = (environments[suite.next(2)], environments[suite.next(2)]);
                    let key = format!("r{seed}-{step}");
                    env.applied(vec![policy(
                        "rule.create",
                        json!({ "key": key, "label": key, "effect": "allow",
                                "flow_types": [{ "id": admin }],
                                "source": { "policy.environment": [from] },
                                "destination": { "policy.environment": [to] } }),
                    )])
                    .await;
                    rules.push(
                        id_of(
                            &pool,
                            "SELECT id FROM policy.policy_rule WHERE key = $1",
                            &key,
                        )
                        .await,
                    );
                    assert!(drifting(&pool).await.is_empty(), "seed {seed}, step {step}");
                    continue;
                }
                _ => {
                    let source = addresses[suite.next(addresses.len())];
                    let destination = addresses[suite.next(addresses.len())];
                    vec![flow("request", env.fields(source, destination).await)]
                }
            };
            // A request the policy denies waits for review: nothing to apply.
            let _ = env.submit_as(&Unrestricted, env.author, operations).await;
            assert!(drifting(&pool).await.is_empty(), "seed {seed}, step {step}");
        }
    }
    let flows: i64 = sqlx::query_scalar("SELECT count(DISTINCT id) FROM flow.flow")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(flows > 0);
}
