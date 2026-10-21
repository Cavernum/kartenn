//! The audit report of the policy (spec 010, REQ-010-22, 23; task 010-16): asked for a
//! date, produced by the background task `policy.report` by batches, one test per rubric,
//! then its pages, its export, one report at a time and its purge.

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_policy::report::{self, Requested};
use kartenn_service::module::run_job;
use kartenn_service::operations::RawInput;
use kartenn_service::{ObjectKind, Operation, Ref};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::support::env::{Env, op};

const TASK: &str = "policy.report";

/// The environments `prod`, `dev` and the transverse `global`, the flow type `admin` and
/// its service `ssh`.
async fn catalogue(env: &Env) {
    env.applied(vec![
        op("environment.create", json!({ "key": "prod" })),
        op("environment.create", json!({ "key": "dev" })),
        op("environment.create", json!({ "key": "global" })),
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
    .await;
    env.applied(vec![op(
        "value.describe",
        json!({ "property": "policy.environment", "value": "global", "transverse": true }),
    )])
    .await;
}

async fn id(pool: &PgPool, table: &str, key: &str) -> Uuid {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT id FROM policy.{table} WHERE key = $1 AND valid_period @> now()"
    )))
    .bind(key)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// An `allow` (or `deny`) rule of `admin` from `source` to `destination`.
async fn rule(env: &Env, key: &str, effect: &str, source: Value, destination: Value) {
    let admin = id(&env.pool, "policy_flow_type", "admin").await;
    let mut fields = json!({ "key": key, "label": key, "effect": effect,
                             "flow_types": [{ "id": admin }],
                             "source": source, "destination": destination });
    if effect == "deny" {
        fields["texts"] = json!({ "fr": { "body": "Interdit." } });
    }
    env.applied(vec![op("rule.create", fields)]).await;
}

/// A network `prefix` of the domain, with `environment` if given, and one address in it.
async fn network(env: &Env, prefix: &str, address: &str, environment: Option<&str>) {
    let property = env.property("policy.environment").await;
    let mut ops = vec![Operation::CreatePrefix {
        local: Some(1),
        domain: Ref::Id(env.domain),
        prefix: prefix.into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    }];
    if let Some(value) = environment {
        ops.push(Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Prefix,
            property: Ref::Id(property),
            values: vec![RawInput::Text(value.into())],
        });
    }
    ops.push(Operation::CreateAddress {
        local: None,
        domain: Ref::Id(env.domain),
        address: address.into(),
        usage: AddressUsage::Host,
    });
    env.applied(ops).await;
}

async fn node(pool: &PgPool, table: &str, column: &str, value: &str) -> Uuid {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT id FROM {table} WHERE {column} = $1::inet AND valid_period @> now()"
    )))
    .bind(value)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn now(pool: &PgPool) -> OffsetDateTime {
    sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Asks for a report now and runs the task until it is over: its identifier.
async fn produced(env: &Env) -> Uuid {
    let mut conn = env.pool.acquire().await.unwrap();
    let at = now(&env.pool).await;
    let Requested::Started(id) = report::request(&mut conn, at, Some(env.author))
        .await
        .unwrap()
    else {
        panic!("no report being produced")
    };
    drop(conn);
    for _ in 0..50 {
        assert!(
            run_job(&env.store, TASK).await.unwrap(),
            "a task of the module"
        );
        let mut conn = env.pool.acquire().await.unwrap();
        if report::report(&mut conn, id).await.unwrap().unwrap().state == "done" {
            return id;
        }
    }
    panic!("the report is not over");
}

/// The items of `rubric`, all pages.
async fn listed(env: &Env, report_id: Uuid, rubric: &str) -> Vec<(Option<Uuid>, Value)> {
    let mut conn = env.pool.acquire().await.unwrap();
    report::items(&mut conn, report_id, rubric, 0, 10_000)
        .await
        .unwrap()
        .into_iter()
        .map(|i| (i.subject, i.detail))
        .collect()
}

fn subjects(items: &[(Option<Uuid>, Value)]) -> Vec<Uuid> {
    items.iter().filter_map(|(s, _)| *s).collect()
}

/// Nodes with no environment where a rule cites the axis, counted by axis and domain.
#[kartenn_testing::pg_test]
async fn unclassified_nodes_are_listed_by_axis_and_domain(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    rule(
        &env,
        "r1",
        "allow",
        json!({ "policy.environment": ["prod"] }),
        json!({ "policy.environment": ["dev"] }),
    )
    .await;
    network(&env, "10.8.0.0/24", "10.8.0.1", Some("prod")).await;
    network(&env, "10.9.0.0/24", "10.9.0.1", None).await;
    let report_id = produced(&env).await;
    let items = listed(&env, report_id, "unclassified").await;
    let found = subjects(&items);
    let bare = node(&pool, "prefix", "prefix", "10.9.0.0/24").await;
    let bare_host = node(&pool, "address", "address", "10.9.0.1").await;
    let classified = node(&pool, "prefix", "prefix", "10.8.0.0/24").await;
    assert!(
        found.contains(&bare) && found.contains(&bare_host),
        "{items:?}"
    );
    assert!(!found.contains(&classified), "{items:?}");
    let detail = &items.iter().find(|(s, _)| *s == Some(bare)).unwrap().1;
    assert_eq!(detail["axis"], "policy.environment");
    assert_eq!(detail["domain"], json!(env.domain));
    assert_eq!(detail["kind"], "prefix");
}

/// Without a rule citing the axis, no node is unclassified on it.
#[kartenn_testing::pg_test]
async fn an_axis_no_rule_cites_classifies_nothing(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    network(&env, "10.9.0.0/24", "10.9.0.1", None).await;
    let report_id = produced(&env).await;
    assert!(listed(&env, report_id, "unclassified").await.is_empty());
}

/// Nodes whose resolved set holds the transverse `global`, with the node of origin.
#[kartenn_testing::pg_test]
async fn transverse_values_are_listed_with_their_origin(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    network(&env, "10.7.0.0/24", "10.7.0.5", Some("global")).await;
    network(&env, "10.8.0.0/24", "10.8.0.1", Some("prod")).await;
    let report_id = produced(&env).await;
    let items = listed(&env, report_id, "transverse").await;
    let origin = node(&pool, "prefix", "prefix", "10.7.0.0/24").await;
    let host = node(&pool, "address", "address", "10.7.0.5").await;
    assert_eq!(subjects(&items).len(), 2, "{items:?}");
    for (subject, detail) in &items {
        assert!([Some(origin), Some(host)].contains(subject));
        assert_eq!(detail["origin"], json!(origin));
        assert_eq!(detail["values"], json!(["global"]));
    }
}

/// An authorisation whose term lists the transverse value only.
#[kartenn_testing::pg_test]
async fn authorisations_holding_by_a_transverse_value_are_listed(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    rule(
        &env,
        "r-global",
        "allow",
        json!({ "policy.environment": ["global"] }),
        json!({}),
    )
    .await;
    rule(
        &env,
        "r-prod",
        "allow",
        json!({ "policy.environment": ["prod", "global"] }),
        json!({}),
    )
    .await;
    let report_id = produced(&env).await;
    let items = listed(&env, report_id, "transverse_allow").await;
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].1["rule"], "r-global");
    assert_eq!(items[0].1["side"], "source");
}

/// A value of the axis no rule cites.
#[kartenn_testing::pg_test]
async fn values_no_rule_cites_are_listed(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    rule(
        &env,
        "r1",
        "allow",
        json!({ "policy.environment": ["prod"] }),
        json!({ "policy.environment": ["dev"] }),
    )
    .await;
    let report_id = produced(&env).await;
    let values: Vec<Value> = listed(&env, report_id, "uncited_values")
        .await
        .into_iter()
        .filter(|(_, d)| d["axis"] == "policy.environment")
        .map(|(_, d)| d["value"].clone())
        .collect();
    assert_eq!(values, [json!("global")]);
}

/// A value a rule cites and no object carries.
#[kartenn_testing::pg_test]
async fn values_no_node_carries_are_listed(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    rule(
        &env,
        "r1",
        "allow",
        json!({ "policy.environment": ["prod"] }),
        json!({ "policy.environment": ["dev"] }),
    )
    .await;
    network(&env, "10.8.0.0/24", "10.8.0.1", Some("prod")).await;
    let report_id = produced(&env).await;
    let items = listed(&env, report_id, "unborne_values").await;
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(
        (
            &items[0].1["rule"],
            &items[0].1["side"],
            &items[0].1["value"]
        ),
        (&json!("r1"), &json!("destination"), &json!("dev"))
    );
}

/// An `allow` rule a looser `deny` covers entirely; a deny on another source masks nothing.
#[kartenn_testing::pg_test]
async fn authorisations_a_deny_masks_are_listed(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    rule(
        &env,
        "r-allow",
        "allow",
        json!({ "policy.environment": ["prod"] }),
        json!({ "policy.environment": ["dev"] }),
    )
    .await;
    rule(
        &env,
        "r-open",
        "allow",
        json!({ "policy.environment": ["dev"] }),
        json!({ "policy.environment": ["prod"] }),
    )
    .await;
    rule(
        &env,
        "r-deny",
        "deny",
        json!({ "policy.environment": ["prod", "global"] }),
        json!({}),
    )
    .await;
    let report_id = produced(&env).await;
    let items = listed(&env, report_id, "masked_allow").await;
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].1["rule"], "r-allow");
    assert_eq!(items[0].1["masked_by"], "r-deny");
}

/// Without the flows of the 011, the two rubrics over them are not answered.
#[kartenn_testing::pg_test]
async fn without_the_flows_their_rubrics_are_not_answered(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let report_id = produced(&env).await;
    let mut conn = pool.acquire().await.unwrap();
    let done = report::report(&mut conn, report_id).await.unwrap().unwrap();
    assert_eq!(done.counts.get("rules_never_applied"), Some(&None));
    assert_eq!(done.counts.get("combinations_without_flow"), Some(&None));
    assert_eq!(done.counts.get("unclassified"), Some(&Some(0)));
    let exported = report::export(&mut conn, report_id).await.unwrap().unwrap();
    assert_eq!(
        exported["rubrics"]["rules_never_applied"]["answered"],
        false
    );
}

/// The evaluations of the 011, as its tables hold them (the columns the report reads).
async fn flows(pool: &PgPool) {
    for sql in [
        "CREATE SCHEMA flow",
        "CREATE TABLE flow.flow (id uuid, service_id uuid, valid_period tstzrange)",
        "CREATE TABLE flow.flow_evaluation (flow_id uuid, verdict text, decisive_id uuid,
                                            valid_period tstzrange)",
        "GRANT USAGE ON SCHEMA flow TO kartenn_app",
        "GRANT SELECT ON ALL TABLES IN SCHEMA flow TO kartenn_app",
    ] {
        sqlx::query(sql).execute(pool).await.unwrap();
    }
}

/// A declared flow of `ssh` allowed by `rule`.
async fn flow_allowed_by(pool: &PgPool, rule: Uuid, service: Uuid) {
    let flow = Uuid::now_v7();
    sqlx::query("INSERT INTO flow.flow VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL))")
        .bind(flow)
        .bind(service)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO flow.flow_evaluation
         VALUES ($1, 'allowed', $2, tstzrange(now() - interval '1 day', NULL))",
    )
    .bind(flow)
    .bind(rule)
    .execute(pool)
    .await
    .unwrap();
}

/// Rules never decisive for a declared flow.
#[kartenn_testing::pg_test]
async fn rules_never_applied_to_a_flow_are_listed(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    rule(
        &env,
        "r-used",
        "allow",
        json!({ "policy.environment": ["prod"] }),
        json!({ "policy.environment": ["dev"] }),
    )
    .await;
    rule(
        &env,
        "r-idle",
        "allow",
        json!({ "policy.environment": ["dev"] }),
        json!({ "policy.environment": ["prod"] }),
    )
    .await;
    flows(&pool).await;
    flow_allowed_by(
        &pool,
        id(&pool, "policy_rule", "r-used").await,
        id(&pool, "policy_service", "ssh").await,
    )
    .await;
    let report_id = produced(&env).await;
    let rules: Vec<Value> = listed(&env, report_id, "rules_never_applied")
        .await
        .into_iter()
        .map(|(_, d)| d["rule"].clone())
        .collect();
    assert_eq!(rules, [json!("r-idle")]);
}

/// Authorised combinations (rule, service) without any declared flow.
#[kartenn_testing::pg_test]
async fn combinations_without_a_flow_are_listed(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    rule(
        &env,
        "r-used",
        "allow",
        json!({ "policy.environment": ["prod"] }),
        json!({ "policy.environment": ["dev"] }),
    )
    .await;
    rule(
        &env,
        "r-idle",
        "allow",
        json!({ "policy.environment": ["dev"] }),
        json!({ "policy.environment": ["prod"] }),
    )
    .await;
    flows(&pool).await;
    flow_allowed_by(
        &pool,
        id(&pool, "policy_rule", "r-used").await,
        id(&pool, "policy_service", "ssh").await,
    )
    .await;
    let report_id = produced(&env).await;
    let items = listed(&env, report_id, "combinations_without_flow").await;
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(
        (&items[0].1["rule"], &items[0].1["service"]),
        (&json!("r-idle"), &json!("ssh"))
    );
}

/// An open anomaly of `rule` on a fresh object.
async fn anomaly(pool: &PgPool, rule: &str) -> Uuid {
    let object = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO anomaly (rule_id, object_id, message, opened_by)
         SELECT $1, $2, 'écart', id FROM changeset LIMIT 1",
    )
    .bind(rule)
    .bind(object)
    .execute(pool)
    .await
    .unwrap();
    object
}

/// The deviations of the inference, open; a closed one is not listed.
#[kartenn_testing::pg_test]
async fn deviations_of_the_inference_are_listed(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let open = anomaly(&pool, "policy.inference.mismatch").await;
    let closed = anomaly(&pool, "policy.inference.mismatch").await;
    sqlx::query("UPDATE anomaly SET closed_at = now() WHERE object_id = $1")
        .bind(closed)
        .execute(&pool)
        .await
        .unwrap();
    let report_id = produced(&env).await;
    assert_eq!(
        subjects(&listed(&env, report_id, "inference_mismatch").await),
        [open]
    );
}

/// The anomalies of machines, open.
#[kartenn_testing::pg_test]
async fn anomalies_of_machines_are_listed(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    let machine = anomaly(&pool, "policy.machine.environment_mismatch").await;
    let report_id = produced(&env).await;
    let items = listed(&env, report_id, "machine_anomalies").await;
    assert_eq!(subjects(&items), [machine]);
    assert_eq!(items[0].1["message"], "écart");
}

/// One report at a time; pages by rank; the windows of one object resume where they
/// stopped and count as one pass; a report is purged 30 days after it was asked for.
#[kartenn_testing::pg_test]
async fn reports_go_one_at_a_time_by_pages_and_are_purged(pool: PgPool) {
    let env = Env::new(&pool).await;
    catalogue(&env).await;
    rule(
        &env,
        "r1",
        "allow",
        json!({ "policy.environment": ["prod"] }),
        json!({}),
    )
    .await;
    for i in 1..=3 {
        network(
            &env,
            &format!("10.{i}.0.0/24"),
            &format!("10.{i}.0.1"),
            None,
        )
        .await;
    }
    let mut conn = pool.acquire().await.unwrap();
    let at = now(&pool).await;
    let Requested::Started(first) = report::request(&mut conn, at, None).await.unwrap() else {
        panic!("a report started");
    };
    assert_eq!(
        report::request(&mut conn, at, None).await.unwrap(),
        Requested::Busy(first)
    );
    // Windows of one object, two steps per run: the rubric spans several runs.
    for _ in 0..200 {
        report::run_with(&mut conn, 1, 2).await.unwrap();
        if report::report(&mut conn, first)
            .await
            .unwrap()
            .unwrap()
            .state
            == "done"
        {
            break;
        }
    }
    let done = report::report(&mut conn, first).await.unwrap().unwrap();
    assert_eq!(done.state, "done");
    // The domain and the three networks and addresses: seven nodes without environment.
    assert_eq!(done.counts["unclassified"], Some(7));
    let page = report::items(&mut conn, first, "unclassified", 0, 3)
        .await
        .unwrap();
    assert_eq!(page.iter().map(|i| i.seq).collect::<Vec<_>>(), [1, 2, 3]);
    let next = report::items(&mut conn, first, "unclassified", 3, 10)
        .await
        .unwrap();
    assert_eq!(next.len(), 4);
    let exported = report::export(&mut conn, first).await.unwrap().unwrap();
    assert_eq!(exported["state"], "done");
    assert_eq!(exported["rubrics"]["unclassified"]["count"], 7);
    assert_eq!(
        exported["rubrics"]["unclassified"]["items"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    // Over: another may be asked for.
    let Requested::Started(second) = report::request(&mut conn, at, None).await.unwrap() else {
        panic!("a second report started");
    };
    sqlx::query(
        "UPDATE policy.policy_report SET requested_at = now() - interval '31 days'
          WHERE id = $1",
    )
    .bind(first)
    .execute(&mut *conn)
    .await
    .unwrap();
    report::run_with(&mut conn, 1_000, 1).await.unwrap();
    assert!(report::report(&mut conn, first).await.unwrap().is_none());
    let items: i64 =
        sqlx::query_scalar("SELECT count(*) FROM policy.policy_report_item WHERE report_id = $1")
            .bind(first)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(items, 0);
    assert!(report::report(&mut conn, second).await.unwrap().is_some());
}
