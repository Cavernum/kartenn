//! Reading one object at `(valid_at, as_of)`: its attributes and the resolved values of
//! its properties (spec 002, REQ-002-45; spec 004, REQ-004-16; task 002-27). The read the
//! budgets of D14 measure, until the API (006) serves it.

use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{AssertSqlSafe, PgPool};

use kartenn_service::StoreError;
use kartenn_service::port::{AuditQuery, Reader};

use crate::diff::value_json;
use crate::temporal::{TemporalError, snapshot};

pub use kartenn_service::port::ObjectRead;

fn sql_err(e: &TemporalError) -> sqlx::Error {
    sqlx::Error::Protocol(e.to_string())
}

/// Objects of every kind, those of the modules by their record, with their attributes,
/// as a relation `(kind, fields)`.
fn objects_sql() -> Result<String, TemporalError> {
    let tables: [(&str, &str, &[&str]); 6] = [
        ("domain", "addressing_domain", &["id", "slug"]),
        (
            "prefix",
            "prefix",
            &[
                "id",
                "domain_id",
                "prefix",
                "nature",
                "point_to_point",
                "anycast",
                "public",
            ],
        ),
        (
            "range",
            "ip_range",
            &["id", "domain_id", "range", "usage", "public"],
        ),
        (
            "address",
            "address",
            &["id", "domain_id", "address", "usage", "public"],
        ),
        ("entity", "entity", &["id", "slug", "parent_id"]),
        ("site", "site", &["id", "slug", "parent_id"]),
    ];
    let mut parts = Vec::with_capacity(tables.len() + 1);
    for (kind, table, columns) in tables {
        let relation = snapshot(table, columns)?;
        parts.push(format!(
            "SELECT '{kind}' AS kind, to_jsonb(x) AS fields FROM {relation} x WHERE x.id = $3"
        ));
    }
    // An object of a module, by its record (ADR 0026, clause 4): its qualified type.
    let modules = snapshot("module_object", &["id", "kind", "parent_id", "domain_id"])?;
    parts.push(format!(
        "SELECT x.kind, to_jsonb(x) - 'kind' AS fields FROM {modules} x WHERE x.id = $3"
    ));
    Ok(parts.join(" UNION ALL "))
}

/// Resolved values of the properties of `$3`: through its origin for a heritable
/// property, its own assignment otherwise.
fn properties_sql() -> Result<String, TemporalError> {
    let declarations = snapshot(
        "property_declaration",
        &["id", "key", "value_type", "inherit_tree", "sensitive"],
    )?;
    let origins = snapshot(
        "property_origin",
        &["object_id", "property_id", "origin_object_id"],
    )?;
    let assignments = snapshot(
        "property_assignment",
        &[
            "object_id",
            "property_id",
            "kind",
            "v_uuid",
            "v_uuids",
            "v_text",
            "v_texts",
            "v_int",
            "v_ints",
            "v_bool",
            "v_date",
            "v_dates",
        ],
    )?;
    let value = value_json("s.value_type = 'enum'");
    Ok(format!(
        "WITH d AS (SELECT * FROM {declarations} x),
              s AS (SELECT d.id, d.key, d.value_type, d.sensitive, o.origin_object_id AS holder
                    FROM d JOIN {origins} o ON o.property_id = d.id AND o.object_id = $3
                    WHERE d.inherit_tree IS NOT NULL
                    UNION ALL
                    SELECT d.id, d.key, d.value_type, d.sensitive, $3 FROM d WHERE d.inherit_tree IS NULL)
         SELECT s.key, {value}, s.sensitive FROM s
         -- per holder behind a fence, so that the lookup goes down the union to the
         -- indexes (a join hashed the whole table of assignments: 30 ms on the reference
         -- dataset)
         CROSS JOIN LATERAL (SELECT * FROM {assignments} x
                             WHERE x.object_id = s.holder AND x.property_id = s.id
                             OFFSET 0) a
         ORDER BY s.key"
    ))
}

/// The object `id` valid at `valid_at`, as asserted at `as_of` (now when `None`); `None`
/// if it does not exist then.
///
/// # Errors
///
/// Database errors.
#[tracing::instrument(name = "SELECT object", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT object",
))]
pub async fn object(
    pool: &PgPool,
    id: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<ObjectRead>> {
    let row: Option<(String, serde_json::Value)> =
        sqlx::query_as(AssertSqlSafe(objects_sql().map_err(|e| sql_err(&e))?))
            .bind(valid_at)
            .bind(as_of)
            .bind(id)
            .fetch_optional(pool)
            .await?;
    let Some((kind, fields)) = row else {
        return Ok(None);
    };
    let properties: Vec<(String, Option<serde_json::Value>, bool)> =
        sqlx::query_as(AssertSqlSafe(properties_sql().map_err(|e| sql_err(&e))?))
            .bind(valid_at)
            .bind(as_of)
            .bind(id)
            .fetch_all(pool)
            .await?;
    let sensitive = properties
        .iter()
        .filter(|(_, _, sensitive)| *sensitive)
        .map(|(key, _, _)| key.clone())
        .collect();
    Ok(Some(ObjectRead {
        kind,
        fields,
        properties: properties.into_iter().map(|(k, v, _)| (k, v)).collect(),
        sensitive,
        masked: std::collections::BTreeSet::new(),
    }))
}

/// The tables of the six kinds of object a read by UUID looks in.
const OBJECT_TABLES: [&str; 7] = [
    "addressing_domain",
    "prefix",
    "ip_range",
    "address",
    "entity",
    "site",
    "module_object",
];

/// The version of the object `id` valid at `valid_at`, as asserted at `as_of` (now when
/// `None`), the strong `ETag` of its representation (REQ-006-6): without `as_of`, the
/// version a write expects (REQ-004-11), the last changeset that wrote the object or its
/// properties (`object_version`, 006-16); at a past `as_of`, the changeset of the version
/// read. `None` if it does not exist then.
///
/// # Errors
///
/// Database errors.
#[tracing::instrument(name = "SELECT version", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT version",
))]
pub async fn version(
    pool: &PgPool,
    id: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<Uuid>> {
    let mut parts = Vec::with_capacity(OBJECT_TABLES.len());
    for table in OBJECT_TABLES {
        let relation = snapshot(table, &["id", "changeset_id"]).map_err(|e| sql_err(&e))?;
        parts.push(format!(
            "SELECT x.changeset_id FROM {relation} x WHERE x.id = $3"
        ));
    }
    let current = if as_of.is_none() {
        "(SELECT changeset_id FROM object_version WHERE object_id = $3)"
    } else {
        "NULL::uuid"
    };
    let sql = format!(
        "SELECT coalesce(v.version, r.changeset_id) FROM ({}) r(changeset_id) \
         CROSS JOIN (SELECT {}) v(version)",
        parts.join(" UNION ALL "),
        current,
    );
    sqlx::query_scalar(AssertSqlSafe(sql))
        .bind(valid_at)
        .bind(as_of)
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// The time of the database (`now()`), against which the dates of a read are checked.
///
/// # Errors
///
/// Database errors.
pub async fn now(pool: &PgPool) -> sqlx::Result<OffsetDateTime> {
    sqlx::query_scalar("SELECT now()").fetch_one(pool).await
}

fn instant_time(at: kartenn_core::period::Instant) -> Result<OffsetDateTime, StoreError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(at.0) * 1000)
        .map_err(|e| StoreError(e.to_string()))
}

/// The `WHERE` clause of an audit consultation, on `audit.event e`.
fn audit_criteria(
    q: &mut sqlx::QueryBuilder<sqlx::Postgres>,
    query: &AuditQuery,
) -> Result<(), StoreError> {
    q.push(" WHERE true");
    if let Some(from) = query.from.map(instant_time).transpose()? {
        q.push(" AND e.recorded_at >= ").push_bind(from);
    }
    if let Some(to) = query.to.map(instant_time).transpose()? {
        q.push(" AND e.recorded_at < ").push_bind(to);
    }
    if let Some(name) = &query.name {
        q.push(" AND e.name = ").push_bind(name.clone());
    }
    if let Some(actor) = query.actor {
        q.push(" AND e.actor_id = ").push_bind(actor);
    }
    if let Some(object) = query.object {
        q.push(" AND e.target_ids @> ARRAY[")
            .push_bind(object)
            .push("]");
    }
    if let Some(before) = query.before {
        q.push(" AND e.seq < ").push_bind(before);
    }
    Ok(())
}

impl Reader for crate::write::PgStore {
    async fn object(
        &self,
        id: Uuid,
        valid_at: kartenn_core::period::Instant,
        as_of: Option<kartenn_core::period::Instant>,
    ) -> Result<Option<ObjectRead>, StoreError> {
        let as_of = as_of.map(instant_time).transpose()?;
        object(self.pool(), id, instant_time(valid_at)?, as_of)
            .await
            .map_err(|e| StoreError(e.to_string()))
    }

    async fn audit_events(&self, query: &AuditQuery) -> Result<Vec<serde_json::Value>, StoreError> {
        let mut q = sqlx::QueryBuilder::new(
            "SELECT to_jsonb(e) - 'seq' || jsonb_build_object('seq', e.seq) FROM audit.event e",
        );
        audit_criteria(&mut q, query)?;
        q.push(" ORDER BY e.seq DESC LIMIT ")
            .push_bind(i64::from(query.limit));
        q.build_query_scalar()
            .fetch_all(self.pool())
            .await
            .map_err(|e| StoreError(e.to_string()))
    }

    async fn audit_extent(&self, query: &AuditQuery) -> Result<(u64, Option<i64>), StoreError> {
        let mut q = sqlx::QueryBuilder::new("SELECT count(*), max(e.seq) FROM audit.event e");
        audit_criteria(&mut q, query)?;
        let (count, last): (i64, Option<i64>) = q
            .build_query_as()
            .fetch_one(self.pool())
            .await
            .map_err(|e| StoreError(e.to_string()))?;
        Ok((u64::try_from(count).unwrap_or_default(), last))
    }
}
