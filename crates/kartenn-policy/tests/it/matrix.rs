//! The matrix view of the rules (spec 010, REQ-010-34, S28; task 010-11): a cell written
//! by `policy.matrix.set` is a rule, read back in its state; a multi-axis rule is counted
//! out of the view; a view written, read and written again keeps one rule per cell.

use std::collections::BTreeSet;

use kartenn_policy::cache::SnapshotCache;
use kartenn_policy::matrix::{CellState, MatrixView, fingerprint, view};
use kartenn_policy::model::Key;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::support::env::{Env, op, rules};

const TEXT: &str = "Interdit entre ces zones.";

/// The environments `prod`, `dev` and `global`, a flow type `file` and a pattern `relay`.
async fn catalogue(env: &Env) -> (Uuid, Uuid) {
    env.applied(
        ["prod", "dev", "global"]
            .iter()
            .map(|k| op("environment.create", json!({ "key": k })))
            .chain([
                op(
                    "flow_type.create",
                    json!({ "key": "file", "label": "Fichiers" }),
                ),
                op(
                    "pattern.create",
                    json!({ "key": "relay", "label": "Relais",
                            "texts": { "fr": { "body": "Passer par le relais." } } }),
                ),
            ])
            .collect(),
    )
    .await;
    (
        id_of(&env.pool, "policy_flow_type", "file").await,
        id_of(&env.pool, "policy_pattern", "relay").await,
    )
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

fn set(
    source: &str,
    file: Uuid,
    destination: &str,
    state: &str,
    extra: Value,
) -> kartenn_service::Operation {
    let mut body = json!({ "source": source, "destination": destination,
                           "flow_type": { "id": file }, "state": state });
    if let Value::Object(extra) = extra {
        for (k, v) in extra {
            body[k] = v;
        }
    }
    op("matrix.set", body)
}

async fn read(env: &Env) -> MatrixView {
    let mut conn = env.pool.acquire().await.unwrap();
    let snapshot = SnapshotCache::new()
        .snapshot(&mut conn, OffsetDateTime::now_utc(), None)
        .await
        .unwrap();
    view(&snapshot, &Key::new("file").unwrap())
}

async fn living_rules(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM policy.policy_rule WHERE valid_period @> now()")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Acceptance of REQ-010-34: the cell `(prod, file, global)` written `conditional` is an
/// allow that requires the chosen patterns, read back `conditional`; a multi-axis rule is
/// out of the view and counted.
#[kartenn_testing::pg_test]
async fn a_conditional_cell_is_an_allow_that_requires_its_patterns(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (file, relay) = catalogue(&env).await;
    env.applied(vec![set(
        "prod",
        file,
        "global",
        "conditional",
        json!({ "patterns": [{ "id": relay }] }),
    )])
    .await;
    let (effect, patterns): (String, Vec<Uuid>) = sqlx::query_as(
        "SELECT effect, required_pattern_ids FROM policy.policy_rule WHERE valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((effect.as_str(), patterns), ("allow", vec![relay]));
    // A rule of two environments on its source: out of the view.
    env.applied(vec![op(
        "rule.create",
        json!({ "key": "wide", "label": "Large", "effect": "allow",
                "source": { "policy.environment": ["prod", "dev"] },
                "destination": { "policy.environment": ["global"] },
                "flow_types": [{ "id": file }] }),
    )])
    .await;
    let matrix = read(&env).await;
    assert_eq!(matrix.outside, 1);
    assert_eq!(matrix.cells.len(), 1);
    let cell = &matrix.cells[0];
    assert_eq!(
        (cell.source.as_str(), cell.destination.as_str(), cell.state),
        ("prod", "global", CellState::Conditional)
    );
    assert_eq!(cell.patterns, BTreeSet::from([Key::new("relay").unwrap()]));
}

/// A view written, read, then written again: one rule per cell, updated in place, and a
/// cell emptied closes its rule (REQ-010-34, verify « aller-retour »).
#[kartenn_testing::pg_test]
async fn a_view_written_and_read_back_keeps_one_rule_per_cell(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (file, _) = catalogue(&env).await;
    let text = json!({ "texts": { "fr": { "body": TEXT } } });
    env.applied(vec![
        set("dev", file, "prod", "allow", json!({})),
        set("prod", file, "dev", "deny", text.clone()),
    ])
    .await;
    let first = read(&env).await;
    let states: Vec<_> = first
        .cells
        .iter()
        .map(|c| (c.source.as_str(), c.destination.as_str(), c.state))
        .collect();
    assert_eq!(
        states,
        [
            ("dev", "prod", CellState::Allow),
            ("prod", "dev", CellState::Deny)
        ]
    );
    // The view read back, written as it is: nothing new.
    env.applied(
        first
            .cells
            .iter()
            .map(|c| {
                let extra = if c.state == CellState::Deny {
                    text.clone()
                } else {
                    json!({})
                };
                set(
                    c.source.as_str(),
                    file,
                    c.destination.as_str(),
                    c.state.as_str(),
                    extra,
                )
            })
            .collect(),
    )
    .await;
    assert_eq!(read(&env).await, first);
    assert_eq!(living_rules(&pool).await, 2);
    // A cell changed: the same rule, updated.
    env.applied(vec![set("dev", file, "prod", "deny", text.clone())])
        .await;
    assert_eq!(living_rules(&pool).await, 2);
    assert_eq!(read(&env).await.cells[0].state, CellState::Deny);
    // A cell emptied: its rule closed.
    env.applied(vec![set("dev", file, "prod", "none", json!({}))])
        .await;
    assert_eq!(living_rules(&pool).await, 1);
    assert_eq!(read(&env).await.cells.len(), 1);
}

/// A deny cell without its text, a conditional one without patterns, and an empty cell
/// emptied are refused.
#[kartenn_testing::pg_test]
async fn a_cell_is_refused_without_what_its_state_requires(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (file, _) = catalogue(&env).await;
    for (operation, rule) in [
        (
            set("prod", file, "dev", "deny", json!({})),
            "policy.text.missing",
        ),
        (
            set("prod", file, "dev", "conditional", json!({})),
            "policy.operation.invalid",
        ),
        (
            set("prod", file, "dev", "none", json!({})),
            "policy.operation.invalid",
        ),
    ] {
        let outcome = env.submit(vec![operation]).await;
        assert!(rules(&outcome).contains(&rule), "{outcome:?}");
    }
}

/// Under `if_match` (S35): two cells of a changeset compared with the matrix before it are
/// applied; the same tag once the matrix changed is `policy.matrix.stale`, nothing
/// written; the tag read again is applied; without `if_match`, no comparison.
#[kartenn_testing::pg_test]
async fn a_cell_under_if_match_is_refused_once_the_matrix_changed(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (file, _) = catalogue(&env).await;
    let tag = fingerprint(&read(&env).await);
    let tagged = json!({ "if_match": tag });
    env.applied(vec![
        set("prod", file, "dev", "allow", tagged.clone()),
        set("dev", file, "prod", "allow", tagged.clone()),
    ])
    .await;
    let rules_before = living_rules(&env.pool).await;
    let outcome = env
        .submit(vec![set("prod", file, "global", "allow", tagged)])
        .await;
    assert!(
        rules(&outcome).contains(&"policy.matrix.stale"),
        "{outcome:?}"
    );
    assert_eq!(living_rules(&env.pool).await, rules_before);
    let quoted = format!("\"{}\"", fingerprint(&read(&env).await));
    env.applied(vec![set(
        "prod",
        file,
        "global",
        "allow",
        json!({ "if_match": quoted }),
    )])
    .await;
    env.applied(vec![set("global", file, "prod", "allow", json!({}))])
        .await;
    assert_eq!(living_rules(&env.pool).await, rules_before + 2);
}
