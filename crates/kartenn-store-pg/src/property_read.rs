//! Reads of the schema of the properties for the API (spec 006, REQ-006-4; task 006-13a):
//! the declarations and the values of closed lists at `(valid_at, as_of)`, by pages keyed
//! by key; the kinds of the objects an explanation walks.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{AssertSqlSafe, PgPool};

use crate::list::Kind;
use crate::temporal::{TemporalError, snapshot};

fn sql_err(e: &TemporalError) -> sqlx::Error {
    sqlx::Error::Protocol(e.to_string())
}

/// A property declaration as the API serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationRead {
    /// Identifier.
    pub id: Uuid,
    /// Key.
    pub key: String,
    /// Type: `enum`, `text`, `int`, `bool`, `date`, `fqdn`, `ref`, or a module's.
    pub value_type: String,
    /// Parameters of the type.
    pub params: serde_json::Value,
    /// `single` or `set`.
    pub cardinality: String,
    /// Kinds of objects that may carry it.
    pub carriers: Vec<String>,
    /// Tree it is inherited along; `None` when not heritable.
    pub inherit: Option<String>,
    /// Whether its value is sensitive (REQ-003-27).
    pub sensitive: bool,
    /// Source and mode of its expected value.
    pub expected: Option<(String, String)>,
}

type Row = (
    Uuid,
    String,
    String,
    serde_json::Value,
    String,
    Vec<String>,
    Option<String>,
    bool,
    Option<String>,
    Option<String>,
);

fn decoded(
    (id, key, value_type, params, cardinality, carriers, inherit, sensitive, source, mode): Row,
) -> DeclarationRead {
    DeclarationRead {
        id,
        key,
        value_type,
        params,
        cardinality,
        carriers,
        inherit,
        sensitive,
        expected: source.zip(mode),
    }
}

/// The declarations at `(valid_at, as_of)` whose key is `$3` (when not null) or after
/// `$4` (when not null), by key, at most `$5`.
fn declarations_sql() -> Result<String, TemporalError> {
    let declaration = snapshot(
        "property_declaration",
        &[
            "id",
            "key",
            "value_type",
            "type_params",
            "cardinality",
            "carriers",
            "inherit_tree",
            "sensitive",
            "expected_source",
            "expected_mode",
        ],
    )?;
    Ok(format!(
        "SELECT id, key, value_type, type_params, cardinality, carriers, inherit_tree,
                sensitive, expected_source, expected_mode
         FROM {declaration} d
         WHERE ($3::text IS NULL OR key = $3) AND ($4::text IS NULL OR key > $4)
         ORDER BY key LIMIT $5"
    ))
}

/// The declarations valid at `valid_at` as asserted at `as_of` (now when `None`), by
/// key, after `after`, at most `limit`.
///
/// # Errors
///
/// Database errors.
pub async fn declarations(
    pool: &PgPool,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    after: Option<&str>,
    limit: i64,
) -> sqlx::Result<Vec<DeclarationRead>> {
    let rows: Vec<Row> =
        sqlx::query_as(AssertSqlSafe(declarations_sql().map_err(|e| sql_err(&e))?))
            .bind(valid_at)
            .bind(as_of)
            .bind(None::<String>)
            .bind(after)
            .bind(limit)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(decoded).collect())
}

/// The declaration of `key` valid at `valid_at` as asserted at `as_of`, if any.
///
/// # Errors
///
/// Database errors.
pub async fn declaration(
    pool: &PgPool,
    key: &str,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<DeclarationRead>> {
    let row: Option<Row> =
        sqlx::query_as(AssertSqlSafe(declarations_sql().map_err(|e| sql_err(&e))?))
            .bind(valid_at)
            .bind(as_of)
            .bind(key)
            .bind(None::<String>)
            .bind(1_i64)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(decoded))
}

/// The keys of the closed list of `property` valid at `valid_at` as asserted at `as_of`,
/// sorted, after `after`, at most `limit`.
///
/// # Errors
///
/// Database errors.
pub async fn enum_values(
    pool: &PgPool,
    property: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    after: Option<&str>,
    limit: i64,
) -> sqlx::Result<Vec<String>> {
    let values =
        snapshot("property_enum_value", &["property_id", "key"]).map_err(|e| sql_err(&e))?;
    sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT key FROM {values} v
         WHERE property_id = public.kartenn_values_owner($3) AND ($4::text IS NULL OR key > $4)
         ORDER BY key LIMIT $5"
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(property)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// The keys of the properties declared sensitive now.
///
/// # Errors
///
/// Database errors.
pub async fn sensitive_keys(pool: &PgPool) -> sqlx::Result<BTreeSet<String>> {
    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT key FROM property_declaration WHERE sensitive AND valid_period @> now()",
    )
    .fetch_all(pool)
    .await?;
    Ok(keys.into_iter().collect())
}

/// The properties declared sensitive now: key by identifier.
///
/// # Errors
///
/// Database errors.
pub async fn sensitive_ids(pool: &PgPool) -> sqlx::Result<BTreeMap<Uuid, String>> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, key FROM property_declaration WHERE sensitive AND valid_period @> now()",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// The kind of each of `ids` at the dates, by name: those of the core as in [`kinds`],
/// those of the modules by their qualified type (`vlan.vlan`, ADR 0026); absent ones are
/// left out.
///
/// # Errors
///
/// Database errors.
pub async fn kind_names(
    pool: &PgPool,
    ids: &[Uuid],
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<HashMap<Uuid, String>> {
    let mut found: HashMap<Uuid, String> = kinds(pool, ids, valid_at, as_of)
        .await?
        .into_iter()
        .map(|(id, kind)| (id, kind.as_str().to_owned()))
        .collect();
    let relation = snapshot("module_object", &["id", "kind"]).map_err(|e| sql_err(&e))?;
    let rows: Vec<(Uuid, String)> = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT x.id, x.kind FROM {relation} x WHERE x.id = ANY($3)"
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(ids)
    .fetch_all(pool)
    .await?;
    found.extend(rows);
    Ok(found)
}

/// The kind of each of `ids` that exists at `valid_at` as asserted at `as_of`.
///
/// # Errors
///
/// Database errors.
pub async fn kinds(
    pool: &PgPool,
    ids: &[Uuid],
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<HashMap<Uuid, Kind>> {
    let tables = [
        (Kind::Entity, "entity"),
        (Kind::Site, "site"),
        (Kind::Domain, "addressing_domain"),
        (Kind::Prefix, "prefix"),
        (Kind::Range, "ip_range"),
        (Kind::Address, "address"),
    ];
    let mut parts = Vec::with_capacity(tables.len());
    for (kind, table) in tables {
        let relation = snapshot(table, &["id"]).map_err(|e| sql_err(&e))?;
        parts.push(format!(
            "SELECT '{}' AS kind, id FROM {relation} x WHERE id = ANY($3)",
            kind.as_str()
        ));
    }
    let rows: Vec<(String, Uuid)> = sqlx::query_as(AssertSqlSafe(parts.join(" UNION ALL ")))
        .bind(valid_at)
        .bind(as_of)
        .bind(ids)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(kind, id)| {
            Kind::ALL
                .into_iter()
                .find(|k| k.as_str() == kind)
                .map(|k| (id, k))
        })
        .collect())
}
