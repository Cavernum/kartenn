//! Lists of objects by pages, read by key (spec 006, REQ-006-7, 8; task 006-10): sorted
//! by natural key then UUID, a page starts after the last key of the previous one, so a
//! write between two pages neither skips nor repeats an element; each page applies the
//! read scope of its caller (005, REQ-005-24).

use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{AssertSqlSafe, PgPool};

use crate::rbac::{ReadScope, readable};
use crate::temporal::{TemporalError, snapshot};

/// The position of a prefix in its list: domain, value, identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixKey {
    /// Addressing domain.
    pub domain: Uuid,
    /// Value, CIDR text.
    pub prefix: String,
    /// Identifier.
    pub id: Uuid,
}

/// A prefix of a list: its key and its attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    /// Position in the list.
    pub key: PrefixKey,
    /// Attributes, as in a read by identifier.
    pub fields: serde_json::Value,
}

fn prefixes_sql() -> Result<String, TemporalError> {
    let relation = snapshot(
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
    )?;
    let visible = readable("x.id", "'prefix'", "x.domain_id", "x.prefix", "$1");
    Ok(format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT x.domain_id, x.prefix::text, x.id, to_jsonb(x) FROM {relation} x
         WHERE ($4::uuid IS NULL OR (x.domain_id, x.prefix, x.id) > ($4, $5::cidr, $6))
           AND ($3::jsonb IS NULL OR {visible})
         ORDER BY x.domain_id, x.prefix, x.id
         LIMIT $7"
    ))
}

/// At most `limit` prefixes valid at `valid_at`, as asserted at `as_of` (now when
/// `None`), after `after`, within what `scope` lets read.
///
/// # Errors
///
/// Database errors.
#[tracing::instrument(name = "SELECT prefix", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT prefix",
))]
pub async fn prefixes(
    pool: &PgPool,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    after: Option<&PrefixKey>,
    limit: i64,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Listed>> {
    let sql = prefixes_sql().map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    let rows: Vec<(Uuid, String, Uuid, serde_json::Value)> = sqlx::query_as(AssertSqlSafe(sql))
        .bind(valid_at)
        .bind(as_of)
        .bind(scope.to_json())
        .bind(after.map(|k| k.domain))
        .bind(after.map(|k| k.prefix.as_str()))
        .bind(after.map(|k| k.id))
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(domain, prefix, id, fields)| Listed {
            key: PrefixKey { domain, prefix, id },
            fields,
        })
        .collect())
}
