//! Deadlines and events of the registry (task 011-8; REQ-011-7): the task `flow.expiry`
//! announces a dated flow at J-30, J-7 and its term, once per end, never a flow without
//! end; a changeset that flips the summary of a flow emits `outcome_changed`, and a
//! closing emits `closed`.

use kartenn_service::authz::Unrestricted;
use kartenn_service::module::run_job;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref, submit};
use serde_json::Value;
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::requests::{Env, flow};

const TASK: &str = "flow.expiry";
const DAY: i64 = 24 * 3600;

/// The events of `kind` about `flow` in the outbox.
async fn events(pool: &PgPool, kind: &str, flow: Uuid) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT payload FROM outbox WHERE payload->>'type' = $1 AND payload->>'subject' = $2
          ORDER BY seq",
    )
    .bind(format!("org.kartenn.flow.{kind}"))
    .bind(flow.to_string())
    .fetch_all(pool)
    .await
    .unwrap()
}

/// A compliant flow `prod → dev` from the addresses `n.1`, ending `seconds` from now (none:
/// without end), requested with its `end` in a changeset of open period.
async fn dated(env: &Env, n: u8, seconds: Option<i64>) -> Uuid {
    let source = env
        .address(&format!("10.{n}.0.0/24"), &format!("10.{n}.0.1"), "prod")
        .await;
    let destination = env
        .address(
            &format!("10.{}.0.0/24", n + 100),
            &format!("10.{}.0.1", n + 100),
            "dev",
        )
        .await;
    let mut fields = env.fields(source, destination).await;
    if let Some(seconds) = seconds {
        let end: String = sqlx::query_scalar(
            "SELECT to_char((now() + $1 * interval '1 second') AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
        )
        .bind(seconds)
        .fetch_one(&env.pool)
        .await
        .unwrap();
        fields["end"] = Value::String(end);
    }
    let request = env.request(env.author, vec![flow("request", fields)]);
    let outcome = submit(&env.store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    sqlx::query_scalar("SELECT id FROM flow.flow WHERE source_id = $1")
        .bind(source)
        .fetch_one(&env.pool)
        .await
        .unwrap()
}

async fn run(env: &Env) {
    assert!(run_job(&env.store, TASK).await.unwrap());
}

/// REQ-011-7: a flow 20 days from its end is announced at J-30, one 5 days from it at J-7
/// only; a task run again announces neither twice; a flow without end never.
#[kartenn_testing::pg_test]
async fn a_dated_flow_is_announced_once_per_window_and_end(pool: PgPool) {
    let env = Env::new(&pool).await;
    let far = dated(&env, 1, Some(20 * DAY)).await;
    let near = dated(&env, 2, Some(5 * DAY)).await;
    let forever = dated(&env, 3, None).await;
    run(&env).await;
    run(&env).await;
    let expiring = events(&pool, "expiring", far).await;
    assert_eq!(expiring.len(), 1, "{expiring:?}");
    assert_eq!(expiring[0]["data"]["days"], 30);
    assert_eq!(expiring[0]["data"]["of"], "flow");
    let expiring = events(&pool, "expiring", near).await;
    assert_eq!(expiring.len(), 1, "J-7 only: {expiring:?}");
    assert_eq!(expiring[0]["data"]["days"], 7);
    assert!(events(&pool, "expiring", forever).await.is_empty());
    assert!(events(&pool, "expired", forever).await.is_empty());
}

/// REQ-011-7: a flow at its term is announced, once.
#[kartenn_testing::pg_test]
async fn a_flow_at_its_term_is_announced_once(pool: PgPool) {
    let env = Env::new(&pool).await;
    let soon = dated(&env, 4, Some(2)).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    run(&env).await;
    run(&env).await;
    assert_eq!(events(&pool, "expired", soon).await.len(), 1);
}

/// REQ-011-7: a changeset that makes a flow denied emits `outcome_changed`; closing one
/// emits `closed`.
#[kartenn_testing::pg_test]
async fn outcome_changed_and_closed(pool: PgPool) {
    let env = Env::new(&pool).await;
    let id = dated(&env, 5, None).await;
    let rule: Uuid = sqlx::query_scalar("SELECT id FROM policy.policy_rule WHERE key = 'prod-dev'")
        .fetch_one(&pool)
        .await
        .unwrap();
    env.applied(vec![Operation::Close {
        kind: ObjectKind::from_name("policy.rule").unwrap(),
        id: Ref::Id(rule),
        expected: None,
        cascade: false,
    }])
    .await;
    let changed = events(&pool, "outcome_changed", id).await;
    assert_eq!(changed.len(), 1, "{changed:?}");
    assert_eq!(changed[0]["data"]["after"]["summary"], "denied");
    env.applied(vec![Operation::Close {
        kind: ObjectKind::from_name("flow.flow").unwrap(),
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    }])
    .await;
    assert_eq!(events(&pool, "closed", id).await.len(), 1);
}
