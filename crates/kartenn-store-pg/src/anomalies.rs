//! Anomalies: warnings recorded against objects (REQ-002-35, 36).

use std::collections::BTreeMap;

use kartenn_core::rules::{Severity, Violation};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, Postgres, Transaction};

use crate::list::Kind;
use crate::rbac::{ReadScope, readable};

/// Versioned tables whose current rows tell which changeset last wrote an object: every
/// object an anomaly can be about.
const VERSIONED: [&str; 6] = [
    "addressing_domain",
    "prefix",
    "ip_range",
    "address",
    "entity",
    "site",
];

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

/// Which open anomalies to list (REQ-002-36); every field narrows the list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnomalyFilter<'a> {
    /// One rule.
    pub rule: Option<&'a str>,
    /// One addressing domain: the domain and the objects it holds now.
    pub domain: Option<Uuid>,
    /// One entity: the entity, its descendants, and the objects whose resolved owner
    /// (`core.owner`) is one of them now.
    pub entity: Option<Uuid>,
}

/// Open anomalies matching `filter`: unacknowledged ones, and acknowledged ones whose object
/// changed since (REQ-002-36).
///
/// # Errors
///
/// Database errors.
pub async fn open(pool: &PgPool, filter: &AnomalyFilter<'_>) -> sqlx::Result<Vec<Anomaly>> {
    let found = open_within(pool, filter, &ReadScope::all()).await?;
    Ok(found.into_iter().map(|(anomaly, _)| anomaly).collect())
}

/// [`open`] restricted to the anomalies whose object `scope` lets read now (spec 006,
/// REQ-006-23): the anomaly of an object the caller may not read is not listed. Each comes
/// with the kind of its object now (`None` for an object that no longer exists).
///
/// # Errors
///
/// Database errors.
pub async fn open_within(
    pool: &PgPool,
    filter: &AnomalyFilter<'_>,
    scope: &ReadScope,
) -> sqlx::Result<Vec<(Anomaly, Option<Kind>)>> {
    let object_readable = readable("x.id", "x.kind", "x.domain_id", "x.value", "now()");
    let sql = format!(
        "WITH RECURSIVE s AS MATERIALIZED (SELECT * FROM kartenn_scopes($4)),
         sub(id, depth) AS (
             SELECT id, 0 FROM entity WHERE id = $3 AND valid_period @> now()
             UNION ALL
             SELECT c.id, sub.depth + 1 FROM sub
             JOIN entity c ON c.parent_id = sub.id AND c.valid_period @> now()
             WHERE sub.depth < 1000
         )
         SELECT a.id, a.rule_id, a.object_id, a.message, a.opened_at,
                (SELECT x.kind FROM carrier_object x
                 WHERE x.id = a.object_id AND x.valid_period @> now() LIMIT 1)
         FROM anomaly a
         WHERE a.closed_at IS NULL AND ($1::text IS NULL OR a.rule_id = $1)
           AND ($2::uuid IS NULL OR a.object_id = $2 OR EXISTS (
                   SELECT 1 FROM carrier_object x
                   WHERE x.id = a.object_id AND x.domain_id = $2 AND x.valid_period @> now()))
           AND ($3::uuid IS NULL OR a.object_id IN (SELECT id FROM sub) OR EXISTS (
                   SELECT 1 FROM property_origin p
                   JOIN property_assignment v ON v.object_id = p.origin_object_id
                       AND v.property_id = p.property_id AND v.kind = 'value'
                       AND v.valid_period @> now()
                   WHERE p.object_id = a.object_id AND p.valid_period @> now()
                     AND p.property_id IN (SELECT id FROM property_declaration WHERE key = 'core.owner')
                     AND v.v_uuid IN (SELECT id FROM sub)))
           AND (a.acknowledged_at IS NULL OR a.acked_changeset IS DISTINCT FROM ({} LIMIT 1))
           AND ($4::jsonb IS NULL OR EXISTS (
                   SELECT 1 FROM carrier_object x
                   WHERE x.id = a.object_id AND x.valid_period @> now() AND {object_readable}))
         ORDER BY a.opened_at, a.id",
        last_changeset_sql()
    );
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(filter.rule)
        .bind(filter.domain)
        .bind(filter.entity)
        .bind(scope.to_json())
        .fetch_all(pool)
        .await
        .map(|rows: Vec<Row>| rows.into_iter().map(listed).collect())
}

/// Row of [`open_within`] and [`find`]: an anomaly and the kind of its object.
type Row = (Uuid, String, Uuid, String, OffsetDateTime, Option<String>);

fn listed((id, rule_id, object_id, message, opened_at, kind): Row) -> (Anomaly, Option<Kind>) {
    let kind = kind.and_then(|kind| Kind::ALL.into_iter().find(|k| k.as_str() == kind));
    let anomaly = Anomaly {
        id,
        rule_id,
        object_id,
        message,
        opened_at,
    };
    (anomaly, kind)
}

/// The open anomaly `id` and the kind of its object now; `None` if it is closed, absent, or
/// its object no longer exists.
///
/// # Errors
///
/// Database errors.
pub async fn find(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<(Anomaly, Kind)>> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT a.id, a.rule_id, a.object_id, a.message, a.opened_at, x.kind FROM anomaly a
         JOIN carrier_object x ON x.id = a.object_id AND x.valid_period @> now()
         WHERE a.id = $1 AND a.closed_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|row| match listed(row) {
        (anomaly, Some(kind)) => Some((anomaly, kind)),
        (_, None) => None,
    }))
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
