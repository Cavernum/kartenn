//! The deviations of the inference by origin node (spec 010, REQ-010-48; task 010-14):
//! the background task `policy.inference_check` counts, once the rules of inference
//! change, the heirs of a declared environment that the inference expects otherwise, and
//! opens or closes `policy.inference.mismatch` on the origin (ADR 0026, amendment 4).

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::module::{Findings, run_job};
use kartenn_service::operations::RawInput;
use kartenn_service::port::{Store as _, WriteTx as _};
use kartenn_service::{ObjectKind, Operation, Ref};
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::support::env::{Env, op};

const TASK: &str = "policy.inference_check";
const MISMATCH: &str = "policy.inference.mismatch";

/// The open anomaly of the inference on `object`: its message.
async fn open(env: &Env, object: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT message FROM anomaly
          WHERE object_id = $1 AND rule_id = $2 AND closed_at IS NULL",
    )
    .bind(object)
    .bind(MISMATCH)
    .fetch_optional(&env.pool)
    .await
    .unwrap()
}

async fn prefix_id(pool: &PgPool, prefix: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = $1::cidr AND valid_period @> now()")
        .bind(prefix)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Runs the task until its pass is over.
async fn checked(env: &Env) {
    for _ in 0..10 {
        assert!(
            run_job(&env.store, TASK).await.unwrap(),
            "a task of the module"
        );
        let pass: Option<i64> =
            sqlx::query_scalar("SELECT pass FROM policy.policy_inference_check")
                .fetch_one(&env.pool)
                .await
                .unwrap();
        if pass.is_none() {
            return;
        }
    }
    panic!("the pass does not end");
}

/// Acceptance of REQ-010-48: `10.0.0.0/8` declared `rec`, a network and two addresses
/// inheriting it. No rule: nothing. A rule expects `prod` within `10.0.0.0/24`: the task
/// opens one anomaly on the origin `/8`, counting the three heirs in deviation (network
/// and addresses). The rule changed to expect `rec`: the task closes it.
#[kartenn_testing::pg_test]
async fn a_deviation_is_counted_on_its_origin_and_closes_once_corrected(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.applied(vec![
        op("environment.create", json!({ "key": "prod" })),
        op("environment.create", json!({ "key": "rec" })),
    ])
    .await;
    let environment = env.property("policy.environment").await;
    env.applied(vec![
        Operation::CreatePrefix {
            local: Some(1),
            domain: Ref::Id(env.domain),
            prefix: "10.0.0.0/8".into(),
            nature: Nature::Container,
            point_to_point: false,
            anycast: false,
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Prefix,
            property: Ref::Id(environment),
            values: vec![RawInput::Text("rec".into())],
        },
        Operation::CreatePrefix {
            local: Some(2),
            domain: Ref::Id(env.domain),
            prefix: "10.0.0.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
        Operation::CreateAddress {
            local: None,
            domain: Ref::Id(env.domain),
            address: "10.0.0.9".into(),
            usage: AddressUsage::Host,
        },
        Operation::CreateAddress {
            local: None,
            domain: Ref::Id(env.domain),
            address: "10.0.0.10".into(),
            usage: AddressUsage::Host,
        },
    ])
    .await;
    let origin = prefix_id(&pool, "10.0.0.0/8").await;
    // No rule written yet: the revision has not moved, nothing to check.
    checked(&env).await;
    assert_eq!(open(&env, origin).await, None);

    env.applied(vec![op(
        "inference_rule.create",
        json!({ "key": "r-prod", "axis": "policy.environment", "priority": 0,
                "criteria": [{ "within": ["10.0.0.0/24"] }], "values": ["prod"] }),
    )])
    .await;
    checked(&env).await;
    let message = open(&env, origin).await.expect("an anomaly on the origin");
    assert!(message.starts_with("3 "), "{message}");
    // One anomaly, on the origin only, not on each heir.
    let heirs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM anomaly WHERE rule_id = $1 AND closed_at IS NULL")
            .bind(MISMATCH)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(heirs, 1);
    // A second run on the same revision: nothing moves.
    checked(&env).await;
    assert!(open(&env, origin).await.is_some());

    let rule: Uuid = sqlx::query_scalar(
        "SELECT id FROM policy.policy_inference_rule WHERE key = 'r-prod'
            AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    env.applied(vec![op(
        "inference_rule.update",
        json!({ "id": { "id": rule }, "values": ["rec"] }),
    )])
    .await;
    checked(&env).await;
    assert_eq!(open(&env, origin).await, None);
}

/// A pass goes by batches: with a batch of one heir, the count of an origin spans several
/// runs and its anomaly appears once every heir was read.
#[kartenn_testing::pg_test]
async fn a_pass_goes_by_batches(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.applied(vec![
        op("environment.create", json!({ "key": "prod" })),
        op("environment.create", json!({ "key": "rec" })),
    ])
    .await;
    let environment = env.property("policy.environment").await;
    env.applied(vec![
        Operation::CreatePrefix {
            local: Some(1),
            domain: Ref::Id(env.domain),
            prefix: "10.1.0.0/24".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Prefix,
            property: Ref::Id(environment),
            values: vec![RawInput::Text("rec".into())],
        },
        Operation::CreateAddress {
            local: None,
            domain: Ref::Id(env.domain),
            address: "10.1.0.5".into(),
            usage: AddressUsage::Host,
        },
    ])
    .await;
    env.applied(vec![op(
        "inference_rule.create",
        json!({ "key": "r-prod", "axis": "policy.environment", "priority": 0,
                "criteria": [{ "within": ["10.1.0.0/24"] }], "values": ["prod"] }),
    )])
    .await;
    let origin = prefix_id(&pool, "10.1.0.0/24").await;
    // One run with a batch of one heir: the origin is not complete, nothing found yet.
    let mut tx = env.store.begin().await.unwrap();
    let first = kartenn_policy::mismatch::run_with(tx.module_tx(), 1)
        .await
        .unwrap();
    assert_eq!(first, Findings::default());
    tx.commit().await.unwrap();
    // The scheduled runs go on from where it stopped: the count spans the runs.
    checked(&env).await;
    let message = open(&env, origin).await.expect("an anomaly once complete");
    assert!(message.starts_with("2 "), "{message}");
}
