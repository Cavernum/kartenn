//! Reads of the dictionary of labels (spec 018, R19 to R24; ADR 0040): the revision of
//! the labels, the labels stored with their owning objects at the dates of a read, and
//! the entries a subject holds on any scope, which filter the spaces.

use std::fmt::Write as _;

use kartenn_service::authz::Subject;
use serde_json::Value;
use sqlx::types::time::OffsetDateTime;
use sqlx::{AssertSqlSafe, PgPool};

use crate::temporal::snapshot;

/// Labels stored with an owning object.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    /// The space of the dictionary: `value`, `property`, `role`, or a module's.
    pub space: String,
    /// For a value, its property.
    pub property: Option<String>,
    /// The key of the object.
    pub key: String,
    /// `{<tag>: <text>}`.
    pub labels: Value,
    /// For a value or a property: the property is sensitive (REQ-003-27).
    pub sensitive: bool,
}

/// The revision of the labels stored (R22).
///
/// # Errors
///
/// Database errors.
pub async fn revision(pool: &PgPool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT revision FROM label_revision WHERE id = 1")
        .fetch_one(pool)
        .await
}

fn sql_err(e: &crate::temporal::TemporalError) -> sqlx::Error {
    sqlx::Error::Protocol(e.to_string())
}

/// The labels stored at `valid_at` as asserted at `as_of` (now when `None`): the values
/// of the closed lists, the declarations, the roles, and the rows of `tables`
/// (`(space, table)`, versioned, with `key` and `labels`). Rows without labels are left
/// out.
///
/// # Errors
///
/// Database errors.
pub async fn stored(
    pool: &PgPool,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    tables: &[(&str, &str)],
) -> sqlx::Result<Vec<Stored>> {
    type Row = (String, Option<String>, String, Value, bool);
    let declarations = snapshot(
        "property_declaration",
        &["id", "key", "labels", "sensitive"],
    )
    .map_err(|e| sql_err(&e))?;
    let values = snapshot("property_enum_value", &["property_id", "key", "labels"])
        .map_err(|e| sql_err(&e))?;
    let roles = snapshot("role", &["key", "labels"]).map_err(|e| sql_err(&e))?;
    let mut sql = format!(
        "SELECT 'value' AS space, d.key AS property, v.key, v.labels, d.sensitive
           FROM {values} v JOIN {declarations} d ON d.id = v.property_id
          WHERE v.labels <> '{{}}'
         UNION ALL
         SELECT 'property', NULL, d.key, d.labels, d.sensitive FROM {declarations} d
          WHERE d.labels <> '{{}}'
         UNION ALL
         SELECT 'role', NULL, r.key, r.labels, false FROM {roles} r WHERE r.labels <> '{{}}'"
    );
    for (space, table) in tables {
        // The space is a label of the code of a module (`[a-z_.]`); the table is checked.
        if !space
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'_' || b == b'.')
        {
            return Err(sqlx::Error::Protocol(format!("{space} is not a space")));
        }
        let rows = snapshot(table, &["key", "labels"]).map_err(|e| sql_err(&e))?;
        let _ = write!(
            sql,
            " UNION ALL SELECT '{space}', NULL, t.key, t.labels, false FROM {rows} t
               WHERE t.labels <> '{{}}'"
        );
    }
    let rows: Vec<Row> = sqlx::query_as(AssertSqlSafe(sql))
        .bind(valid_at)
        .bind(as_of)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(space, property, key, labels, sensitive)| Stored {
            space,
            property,
            key,
            labels,
            sensitive,
        })
        .collect())
}

/// The entries of the roles `subject` holds now, on any scope, and the keys of those
/// roles.
///
/// # Errors
///
/// Database errors.
pub async fn held(pool: &PgPool, subject: &Subject) -> sqlx::Result<(Vec<String>, Vec<String>)> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT DISTINCT r.key, p.permission FROM binding b
           JOIN role r ON r.id = b.role_id AND r.valid_period @> now()
           LEFT JOIN role_permission p ON p.role_id = r.id AND p.valid_period @> now()
          WHERE b.valid_period @> now()
            AND b.group_id IN (SELECT kartenn_member_of($1, $2, $3, now()))",
    )
    .bind(subject.id)
    .bind(subject.issuer.as_deref())
    .bind(&subject.groups)
    .fetch_all(pool)
    .await?;
    let mut roles: Vec<String> = rows.iter().map(|(r, _)| r.clone()).collect();
    roles.sort();
    roles.dedup();
    let mut entries: Vec<String> = rows.into_iter().filter_map(|(_, p)| p).collect();
    entries.sort();
    entries.dedup();
    Ok((entries, roles))
}
