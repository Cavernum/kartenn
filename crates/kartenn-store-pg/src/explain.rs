//! Explained resolution of a property (spec 003, REQ-003-14, 15; task 003-14).
//!
//! At any `(valid_at, as_of)`: the resolved value, the object carrying the applicable
//! assignment (the materialised origin), the chain of objects walked from the object up
//! to it in the addressing tree of that instant, the provenance of the assignment and,
//! when it overrides one, the inherited value it replaces.

use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{AssertSqlSafe, PgPool};

use crate::diff::value_json;
use crate::temporal::{TemporalError, snapshot};

/// The resolution of one property for one object, explained (REQ-003-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    /// Property key.
    pub property: String,
    /// Resolved value as JSON (closed-list keys); `None` when absent or "no value".
    pub value: Option<String>,
    /// Object carrying the applicable assignment; `None` when nothing applies.
    pub origin: Option<Uuid>,
    /// Objects walked from the object to its origin, both included (the object alone
    /// when nothing applies).
    pub chain: Vec<Uuid>,
    /// Provenance of the applicable assignment: `input` or `source:<id>@<version>`.
    pub provenance: Option<String>,
    /// The inherited value the applicable assignment replaces: its origin and value.
    pub replaces: Option<(Uuid, Option<String>)>,
}

fn sql_err(e: &TemporalError) -> sqlx::Error {
    sqlx::Error::Protocol(e.to_string())
}

/// Ancestors of `$3` at `($1, $2)`, nearest first, then its domain: the prefixes of its
/// domain that contain it (strictly for a prefix; from its network up for a range or an
/// address). Empty for a domain.
fn ancestors_sql() -> Result<String, TemporalError> {
    let prefix = snapshot("prefix", &["id", "domain_id", "prefix"])?;
    let range = snapshot("ip_range", &["id", "domain_id", "range"])?;
    let address = snapshot("address", &["id", "domain_id", "address"])?;
    Ok(format!(
        "WITH o AS (
             SELECT id, domain_id, prefix::inet AS v FROM {prefix} x WHERE id = $3
             UNION ALL SELECT id, domain_id, lower(range) FROM {range} x WHERE id = $3
             UNION ALL SELECT id, domain_id, address FROM {address} x WHERE id = $3)
         SELECT id FROM (
             SELECT p.id, masklen(p.prefix) AS depth FROM {prefix} p, o
             WHERE p.domain_id = o.domain_id
               AND p.prefix >>= o.v AND p.id <> o.id
             UNION ALL SELECT o.domain_id, -1 FROM o) a
         ORDER BY depth DESC"
    ))
}

/// Explains the resolution of `property` for `object` at `valid_at`, as asserted at
/// `as_of` (now when `None`). `None` when the property is not declared then.
///
/// # Errors
///
/// Database errors.
pub async fn explain(
    pool: &PgPool,
    object: Uuid,
    property: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<Explanation>> {
    let declaration = snapshot(
        "property_declaration",
        &["id", "key", "value_type", "inherit_tree"],
    )
    .map_err(|e| sql_err(&e))?;
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT key, value_type, inherit_tree FROM {declaration} d WHERE id = $3"
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(property)
    .fetch_optional(pool)
    .await?;
    let Some((key, value_type, inherit)) = row else {
        return Ok(None);
    };
    let ancestors: Vec<Uuid> = if inherit.is_some() {
        sqlx::query_scalar(AssertSqlSafe(ancestors_sql().map_err(|e| sql_err(&e))?))
            .bind(valid_at)
            .bind(as_of)
            .bind(object)
            .fetch_all(pool)
            .await?
    } else {
        Vec::new()
    };
    let origin: Option<Uuid> = if inherit.is_some() {
        let origins = snapshot(
            "property_origin",
            &["object_id", "property_id", "origin_object_id"],
        )
        .map_err(|e| sql_err(&e))?;
        sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT origin_object_id FROM {origins} o WHERE object_id = $3 AND property_id = $4"
        )))
        .bind(valid_at)
        .bind(as_of)
        .bind(object)
        .bind(property)
        .fetch_optional(pool)
        .await?
    } else {
        Some(object)
    };
    let closed_list = value_type == "enum";
    let assignment =
        |candidate: Uuid| assignment_at(pool, candidate, property, closed_list, valid_at, as_of);
    let candidates: Vec<Uuid> = std::iter::once(object).chain(ancestors).collect();
    let Some(origin) = origin else {
        return Ok(Some(Explanation {
            property: key,
            value: None,
            origin: None,
            chain: vec![object],
            provenance: None,
            replaces: None,
        }));
    };
    let Some((value, provenance)) = assignment(origin).await? else {
        // Non-heritable without an assignment of its own.
        return Ok(Some(Explanation {
            property: key,
            value: None,
            origin: None,
            chain: vec![object],
            provenance: None,
            replaces: None,
        }));
    };
    let depth = candidates.iter().position(|c| *c == origin).unwrap_or(0);
    let mut replaces = None;
    for candidate in candidates.iter().skip(depth + 1) {
        if let Some((value, _)) = assignment(*candidate).await? {
            replaces = Some((*candidate, value));
            break;
        }
    }
    Ok(Some(Explanation {
        property: key,
        value,
        origin: Some(origin),
        chain: candidates[..=depth].to_vec(),
        provenance: Some(provenance),
        replaces,
    }))
}

/// The assignment of a property on an object at `(valid_at, as_of)`: its value as JSON
/// (`None` for "no value") and its provenance.
async fn assignment_at(
    pool: &PgPool,
    object: Uuid,
    property: Uuid,
    closed_list: bool,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<(Option<String>, String)>> {
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
            "provenance",
        ],
    )
    .map_err(|e| sql_err(&e))?;
    let value = value_json(if closed_list { "true" } else { "false" });
    sqlx::query_as(AssertSqlSafe(format!(
        "SELECT ({value})::text, a.provenance FROM {assignments} a
         WHERE a.object_id = $3 AND a.property_id = $4"
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(object)
    .bind(property)
    .fetch_optional(pool)
    .await
}
