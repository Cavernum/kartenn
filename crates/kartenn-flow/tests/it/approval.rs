//! The approval policies and the anomalies of the registry (task 011-6; REQ-011-6, 12):
//! the policy `flow.breaks ≥ 1` installed with the module, and the anomaly
//! `flow.verdict.denied` opened on a flow a changeset breaks, closed when it heals.

use std::sync::Arc;

use kartenn_flow::Flows;
use kartenn_machines::Machines;
use kartenn_policy::Policy;
use kartenn_service::module::ModuleRegistry;
use kartenn_service::pipeline::PendingKind;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref};
use kartenn_vlan::Vlan;
use serde_json::json;
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::requests::{Env, flow, policy};

async fn flow_between(pool: &PgPool, source: Uuid, destination: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM flow.flow WHERE source_id = $1 AND destination_id = $2")
        .bind(source)
        .bind(destination)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The rule « prod to dev » the flows of the tests depend on, closed.
async fn close_the_rule(pool: &PgPool) -> Operation {
    let rule: Uuid = sqlx::query_scalar("SELECT id FROM policy.policy_rule WHERE key = 'prod-dev'")
        .fetch_one(pool)
        .await
        .unwrap();
    Operation::Close {
        kind: ObjectKind::from_name("policy.rule").unwrap(),
        id: Ref::Id(rule),
        expected: None,
        cascade: false,
    }
}

/// The open anomalies `flow.verdict.denied` of `flow`.
async fn open_anomalies(pool: &PgPool, flow: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM anomaly
          WHERE rule_id = 'flow.verdict.denied' AND object_id = $1 AND closed_at IS NULL",
    )
    .bind(flow)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A flow `prod → dev`, requested and applied (compliant).
async fn compliant_flow(env: &Env) -> Uuid {
    let source = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let destination = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    env.applied(vec![flow("request", env.fields(source, destination).await)])
        .await;
    flow_between(&env.pool, source, destination).await
}

/// REQ-011-6: the module installs the active policy `flow.breaks ≥ 1`; a changeset that
/// breaks a registered flow waits for an approval.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_6_breaking_a_flow_waits_for_an_approval(pool: PgPool) {
    let env = Env::new(&pool).await;
    let registry = ModuleRegistry::new(vec![
        Arc::new(Vlan),
        Arc::new(Machines),
        Arc::new(Policy),
        Arc::new(Flows),
    ])
    .unwrap();
    let installed = kartenn_store_pg::pending::install_default_policies(&pool, &registry)
        .await
        .unwrap();
    assert_eq!(installed, ["flow.breaks"]);
    let (enabled, triggers): (bool, serde_json::Value) = sqlx::query_as(
        "SELECT enabled, module_triggers FROM approval_policy WHERE name = 'flow.breaks'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(enabled);
    assert_eq!(triggers, json!([["flow.breaks", 1]]));
    compliant_flow(&env).await;
    let close = close_the_rule(&pool).await;
    let outcome = env
        .submit_as(
            &kartenn_service::authz::Unrestricted,
            env.author,
            vec![close],
        )
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
}

/// REQ-011-12: a changeset that makes a registered flow denied is accepted and opens the
/// anomaly `flow.verdict.denied` on it; one that makes it allowed again closes it, and a
/// changeset that touches no flow leaves it open.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_011_12_the_anomaly_of_a_broken_flow(pool: PgPool) {
    let env = Env::new(&pool).await;
    let id = compliant_flow(&env).await;
    assert_eq!(open_anomalies(&pool, id).await, 0);
    env.applied(vec![close_the_rule(&pool).await]).await;
    assert_eq!(open_anomalies(&pool, id).await, 1, "broken: noncompliant");
    env.applied(vec![Operation::CreateEntity {
        local: None,
        slug: "unrelated".into(),
        parent: None,
    }])
    .await;
    assert_eq!(open_anomalies(&pool, id).await, 1, "an unrelated changeset");
    env.applied(vec![policy(
        "rule.create",
        json!({ "key": "prod-dev-again", "labels": { "fr": "Prod vers dev" }, "effect": "allow",
                "flow_types": [{ "id": flow_type(&pool).await }],
                "source": { "policy.environment": ["prod"] },
                "destination": { "policy.environment": ["dev"] } }),
    )])
    .await;
    assert_eq!(open_anomalies(&pool, id).await, 0, "healed: closed");
}

async fn flow_type(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("SELECT id FROM policy.policy_flow_type WHERE key = 'admin'")
        .fetch_one(pool)
        .await
        .unwrap()
}
