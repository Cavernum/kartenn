//! Reading one object at `(valid_at, as_of)`: its attributes and the resolved values of
//! its properties (spec 002, REQ-002-45; spec 004, REQ-004-16; task 002-27). The read the
//! budgets of D14 measure, until the API (006) serves it.

use std::collections::BTreeMap;

use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{AssertSqlSafe, PgPool};

use crate::diff::value_json;
use crate::temporal::{TemporalError, snapshot};

/// An object as it was valid at a date and known at another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectView {
    /// `domain`, `prefix`, `range`, `address`, `entity` or `site`.
    pub kind: String,
    /// Attributes, by column.
    pub fields: serde_json::Value,
    /// Resolved value of each property the object carries or inherits, by key (closed-list
    /// keys); `None` for an explicit "no value".
    pub properties: BTreeMap<String, Option<serde_json::Value>>,
}

fn sql_err(e: &TemporalError) -> sqlx::Error {
    sqlx::Error::Protocol(e.to_string())
}

/// Objects of every kind with their attributes, as a relation `(kind, id, fields)`.
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
    let mut parts = Vec::with_capacity(tables.len());
    for (kind, table, columns) in tables {
        let relation = snapshot(table, columns)?;
        parts.push(format!(
            "SELECT '{kind}' AS kind, to_jsonb(x) AS fields FROM {relation} x WHERE x.id = $3"
        ));
    }
    Ok(parts.join(" UNION ALL "))
}

/// Resolved values of the properties of `$3`: through its origin for a heritable
/// property, its own assignment otherwise.
fn properties_sql() -> Result<String, TemporalError> {
    let declarations = snapshot(
        "property_declaration",
        &["id", "key", "value_type", "inherit_tree"],
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
              s AS (SELECT d.id, d.key, d.value_type, o.origin_object_id AS holder
                    FROM d JOIN {origins} o ON o.property_id = d.id AND o.object_id = $3
                    WHERE d.inherit_tree IS NOT NULL
                    UNION ALL
                    SELECT d.id, d.key, d.value_type, $3 FROM d WHERE d.inherit_tree IS NULL)
         SELECT s.key, {value} FROM s
         JOIN {assignments} a ON a.object_id = s.holder AND a.property_id = s.id
         ORDER BY s.key"
    ))
}

/// The object `id` valid at `valid_at`, as asserted at `as_of` (now when `None`); `None`
/// if it does not exist then.
///
/// # Errors
///
/// Database errors.
pub async fn object(
    pool: &PgPool,
    id: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<ObjectView>> {
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
    let properties: Vec<(String, Option<serde_json::Value>)> =
        sqlx::query_as(AssertSqlSafe(properties_sql().map_err(|e| sql_err(&e))?))
            .bind(valid_at)
            .bind(as_of)
            .bind(id)
            .fetch_all(pool)
            .await?;
    Ok(Some(ObjectView {
        kind,
        fields,
        properties: properties.into_iter().collect(),
    }))
}
