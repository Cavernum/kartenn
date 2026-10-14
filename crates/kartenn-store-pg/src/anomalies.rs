//! Anomalies: warnings recorded against objects (REQ-002-35, 36).

use std::collections::BTreeMap;

use kartenn_core::rules::{Severity, Violation};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, Postgres, Transaction};

/// Versioned tables whose current rows tell which changeset last wrote an object.
const VERSIONED: [&str; 3] = ["addressing_domain", "prefix", "address"];

/// Records the warnings of an applied changeset: anomalies that fire are opened or
/// refreshed; on the `(object, rule-id prefix)` pairs it evaluated anew, those that no
/// longer fire are closed. An anomaly of a rule the changeset did not evaluate stays open.
///
/// # Errors
///
/// Database errors.
pub async fn record(
    tx: &mut Transaction<'static, Postgres>,
    changeset: Uuid,
    reevaluated: &[(Uuid, &str)],
    violations: &[Violation],
) -> sqlx::Result<()> {
    let (scope_objects, scope_rules): (Vec<Uuid>, Vec<String>) = reevaluated
        .iter()
        .map(|(o, prefix)| (*o, (*prefix).to_owned()))
        .unzip();
    let mut firing: BTreeMap<(Uuid, &str), &str> = BTreeMap::new();
    for v in violations
        .iter()
        .filter(|v| v.rule.severity == Severity::Warning)
    {
        if let Some(&object) = v.subjects.first() {
            firing.insert((object, v.rule.id), &v.message);
        }
    }
    let rules: Vec<String> = firing.keys().map(|(_, r)| (*r).to_owned()).collect();
    let objects: Vec<Uuid> = firing.keys().map(|(o, _)| *o).collect();
    // Only the rules evaluated anew on an object can tell that its anomaly is gone.
    sqlx::query(
        "UPDATE anomaly a SET closed_at = now()
         FROM unnest($1::uuid[], $2::text[]) AS s(object_id, prefix)
         WHERE a.closed_at IS NULL AND a.object_id = s.object_id
           AND starts_with(a.rule_id, s.prefix)
           AND (a.object_id, a.rule_id) NOT IN (SELECT * FROM unnest($3::uuid[], $4::text[]))",
    )
    .bind(&scope_objects)
    .bind(&scope_rules)
    .bind(&objects)
    .bind(&rules)
    .execute(&mut **tx)
    .await?;
    for ((object, rule), message) in firing {
        sqlx::query(
            "INSERT INTO anomaly (rule_id, object_id, message, opened_by) VALUES ($1, $2, $3, $4)
             ON CONFLICT (rule_id, object_id) WHERE closed_at IS NULL DO UPDATE SET message = EXCLUDED.message",
        )
        .bind(rule)
        .bind(object)
        .bind(message)
        .bind(changeset)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// An anomaly as listed.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Anomaly {
    /// Identifier.
    pub id: Uuid,
    /// Rule.
    pub rule_id: String,
    /// Object.
    pub object_id: Uuid,
    /// Message.
    pub message: String,
    /// Opening time.
    pub opened_at: OffsetDateTime,
}

fn last_changeset_sql() -> String {
    VERSIONED
        .iter()
        .map(|t| {
            format!("SELECT changeset_id FROM {t} WHERE id = a.object_id AND valid_period @> now()")
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ")
}

/// Open anomalies, optionally for one rule: unacknowledged ones, and acknowledged ones whose
/// object changed since (REQ-002-36).
///
/// # Errors
///
/// Database errors.
pub async fn open(pool: &PgPool, rule: Option<&str>) -> sqlx::Result<Vec<Anomaly>> {
    let sql = format!(
        "SELECT a.id, a.rule_id, a.object_id, a.message, a.opened_at FROM anomaly a
         WHERE a.closed_at IS NULL AND ($1::text IS NULL OR a.rule_id = $1)
           AND (a.acknowledged_at IS NULL OR a.acked_changeset IS DISTINCT FROM ({} LIMIT 1))
         ORDER BY a.opened_at, a.id",
        last_changeset_sql()
    );
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(rule)
        .fetch_all(pool)
        .await
}

/// Acknowledges an anomaly with a motive, against the current version of its object.
///
/// # Errors
///
/// Database errors (an empty motive is refused by a constraint).
pub async fn acknowledge(
    pool: &PgPool,
    anomaly: Uuid,
    person: Uuid,
    motive: &str,
) -> sqlx::Result<bool> {
    let sql = format!(
        "UPDATE anomaly a SET acknowledged_by = $2, acknowledged_at = now(), motive = $3,
             acked_changeset = ({} LIMIT 1)
         WHERE a.id = $1 AND a.closed_at IS NULL",
        last_changeset_sql()
    );
    let done = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(anomaly)
        .bind(person)
        .bind(motive)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}
