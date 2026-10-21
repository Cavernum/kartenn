//! The tables of the module (task 011-2, REQ-011-16, 23): their constraints, the rights
//! of the application role, and a read `as_of` in the past from the history.

use std::sync::Arc;

use kartenn_flow::Flows;
use kartenn_service::module::ModuleRegistry;
use sqlx::types::Uuid;
use sqlx::{PgPool, Postgres, Transaction};

async fn migrated(pool: &PgPool) {
    // With the modules it depends on (policy, machines and vlan).
    let registry = ModuleRegistry::new(vec![
        Arc::new(kartenn_vlan::Vlan),
        Arc::new(kartenn_machines::Machines),
        Arc::new(kartenn_policy::Policy),
        Arc::new(Flows),
    ])
    .unwrap();
    kartenn_store_pg::modules::migrate(pool, &registry)
        .await
        .unwrap();
}

/// A transaction under the application role (INV-11).
async fn as_app(pool: &PgPool) -> Transaction<'static, Postgres> {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE kartenn_app")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx
}

const REFS: &str = r#"[{"system": "00000000-0000-0000-0000-000000000001", "id": "CHG-1042"}]"#;

/// Inserts a flow with `refs` and `justification`; the error code, if any.
async fn insert_flow(
    tx: &mut Transaction<'static, Postgres>,
    id: Uuid,
    refs: &str,
    justification: Option<&str>,
    kind: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO flow.flow (id, kind, source_kind, source_id, destination_kind, \
         destination_id, service_id, external_refs, justification, requester_id, entity_id, \
         valid_period, changeset_id) \
         VALUES ($1, $5, 'machine', gen_random_uuid(), 'prefix', gen_random_uuid(), \
         gen_random_uuid(), $2::jsonb, $3, gen_random_uuid(), gen_random_uuid(), \
         tstzrange(now(), NULL), gen_random_uuid())",
    )
    .bind(id)
    .bind(refs)
    .bind(justification)
    .bind(id)
    .bind(kind)
    .execute(&mut **tx)
    .await
    .map(|_| ())
    .map_err(|e| {
        e.as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .map_or_else(|| e.to_string(), |c| c.to_string())
    })
}

#[kartenn_testing::pg_test]
async fn a_flow_needs_a_reference_and_an_endpoint_kind(pool: PgPool) {
    migrated(&pool).await;
    let mut tx = as_app(&pool).await;
    sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
    insert_flow(&mut tx, Uuid::now_v7(), REFS, None, "requested")
        .await
        .unwrap();
    for (refs, justification, kind) in [
        ("[]", None, "requested"),
        (REFS, Some(""), "requested"),
        (REFS, None, "seen"),
    ] {
        sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
        let refused = insert_flow(&mut tx, Uuid::now_v7(), refs, justification, kind).await;
        assert_eq!(
            refused,
            Err("23514".into()),
            "{refs} {justification:?} {kind}"
        );
        sqlx::query("ROLLBACK TO SAVEPOINT s")
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    // A range is not an endpoint (F3).
    let range = sqlx::query(
        "INSERT INTO flow.flow (id, kind, source_kind, source_id, destination_kind, \
         destination_id, service_id, external_refs, requester_id, entity_id, valid_period, \
         changeset_id) VALUES (gen_random_uuid(), 'requested', 'range', gen_random_uuid(), \
         'address', gen_random_uuid(), gen_random_uuid(), $1::jsonb, gen_random_uuid(), \
         gen_random_uuid(), tstzrange(now(), NULL), gen_random_uuid())",
    )
    .bind(REFS)
    .execute(&mut *tx)
    .await;
    assert!(range.is_err());
}

#[kartenn_testing::pg_test]
async fn a_waiver_has_an_end_a_reason_and_a_risk(pool: PgPool) {
    migrated(&pool).await;
    let mut tx = as_app(&pool).await;
    let insert = |reason: &'static str, risk: &'static str, bounded: bool| {
        format!(
            "INSERT INTO flow.flow_waiver (flow_id, reason, accepted_risk, granted_by, \
             valid_period, changeset_id) VALUES (gen_random_uuid(), '{reason}', '{risk}', \
             gen_random_uuid(), tstzrange(now(), {}), gen_random_uuid())",
            if bounded {
                "now() + interval '30 days'"
            } else {
                "NULL"
            }
        )
    };
    sqlx::raw_sql(sqlx::AssertSqlSafe(insert("migration", "accès", true)))
        .execute(&mut *tx)
        .await
        .unwrap();
    for (reason, risk, bounded) in [
        ("migration", "accès", false),
        (" ", "accès", true),
        ("m", "", true),
    ] {
        sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
        assert!(
            sqlx::raw_sql(sqlx::AssertSqlSafe(insert(reason, risk, bounded)))
                .execute(&mut *tx)
                .await
                .is_err(),
            "{reason:?} {risk:?} {bounded}"
        );
        sqlx::query("ROLLBACK TO SAVEPOINT s")
            .execute(&mut *tx)
            .await
            .unwrap();
    }
}

#[kartenn_testing::pg_test]
async fn a_summary_agrees_with_its_counts(pool: PgPool) {
    migrated(&pool).await;
    let mut tx = as_app(&pool).await;
    let insert = |summary: &str, allowed: i32, denied: i32| {
        format!(
            "INSERT INTO flow.flow_summary (flow_id, summary, outcome, allowed, denied, \
             valid_period, changeset_id) VALUES (gen_random_uuid(), '{summary}', 'automatic', \
             {allowed}, {denied}, tstzrange(now(), NULL), gen_random_uuid())"
        )
    };
    for (summary, allowed, denied) in [
        ("allowed", 2, 0),
        ("partial", 1, 1),
        ("denied", 0, 3),
        ("denied", 0, 0),
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(insert(summary, allowed, denied)))
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    for (summary, allowed, denied) in [("allowed", 1, 1), ("partial", 2, 0), ("denied", 1, 0)] {
        sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
        assert!(
            sqlx::raw_sql(sqlx::AssertSqlSafe(insert(summary, allowed, denied)))
                .execute(&mut *tx)
                .await
                .is_err(),
            "{summary} {allowed} {denied}"
        );
        sqlx::query("ROLLBACK TO SAVEPOINT s")
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    // A pending verdict is an authorisation under reserve.
    sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
    let pending_denied = sqlx::query(
        "INSERT INTO flow.flow_evaluation (flow_id, source_address_id, destination_address_id, \
         verdict, pending, outcome, reason, policy_revision, valid_period, changeset_id) \
         VALUES (gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), 'denied', true, \
         'noncompliant', 'deny', 1, tstzrange(now(), NULL), gen_random_uuid())",
    )
    .execute(&mut *tx)
    .await;
    assert!(pending_denied.is_err());
}

#[kartenn_testing::pg_test]
async fn an_evaluation_as_of_the_past_is_read_from_the_history(pool: PgPool) {
    migrated(&pool).await;
    let flow = Uuid::now_v7();
    // Two transactions: the second rewrites the summary of the flow.
    let mut tx = as_app(&pool).await;
    sqlx::query(
        "INSERT INTO flow.flow_summary (flow_id, summary, outcome, allowed, denied, \
         valid_period, changeset_id) VALUES ($1, 'allowed', 'automatic', 1, 0, \
         tstzrange('2026-01-01', NULL), gen_random_uuid())",
    )
    .bind(flow)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let before: sqlx::types::time::OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut tx = as_app(&pool).await;
    sqlx::query(
        "UPDATE flow.flow_summary SET summary = 'denied', outcome = 'noncompliant', \
         allowed = 0, denied = 1, changeset_id = gen_random_uuid() WHERE flow_id = $1",
    )
    .bind(flow)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let now: String =
        sqlx::query_scalar("SELECT summary FROM flow.flow_summary WHERE flow_id = $1")
            .bind(flow)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(now, "denied");
    let mut tx = as_app(&pool).await;
    let then: String = sqlx::query_scalar(
        "SELECT summary FROM flow.flow_summary_history \
         WHERE flow_id = $1 AND sys_start <= $2 AND sys_end > $2",
    )
    .bind(flow)
    .bind(before)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(then, "allowed");
}

#[kartenn_testing::pg_test]
async fn the_application_appends_registrations_but_never_rewrites_them(pool: PgPool) {
    migrated(&pool).await;
    let mut tx = as_app(&pool).await;
    sqlx::query(
        "INSERT INTO flow.flow_registration (flow_id, summary, outcome, policy_revision, \
         changeset_id) VALUES (gen_random_uuid(), 'allowed', 'automatic', 1, gen_random_uuid())",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
    let rewrite = sqlx::query("UPDATE flow.flow_registration SET outcome = 'review'")
        .execute(&mut *tx)
        .await;
    assert_eq!(
        rewrite
            .unwrap_err()
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("42501")
    );
}
