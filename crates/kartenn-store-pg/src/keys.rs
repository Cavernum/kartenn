//! Natural keys and external identifiers (REQ-002-40, 41).
//!
//! Every object has an immutable UUID; humans, imports and connectors may designate it
//! by a natural key instead, resolved at a date: a slug for entities, sites and domains,
//! `(domain, prefix)`, `(domain, first–last)` and `(domain, address)` for addressing objects, or an external
//! identifier of a connector.

use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{AssertSqlSafe, PgExecutor, PgPool};

use crate::temporal::{AsOf, select_at, snapshot};

// Builder errors cannot happen with the static identifiers used here; they surface as a
// configuration error rather than a panic.

/// Object kinds designated by a slug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlugKind {
    /// Entity.
    Entity,
    /// Site.
    Site,
    /// Addressing domain.
    Domain,
}

impl SlugKind {
    const fn table(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Site => "site",
            Self::Domain => "addressing_domain",
        }
    }
}

/// UUID of the object of `kind` holding `slug` at `valid_at` (current assertions).
///
/// # Errors
///
/// Database errors.
pub async fn by_slug(
    pool: &PgPool,
    kind: SlugKind,
    slug: &str,
    valid_at: OffsetDateTime,
) -> sqlx::Result<Option<Uuid>> {
    let sql = select_at(kind.table(), &["id"], AsOf::Now, "slug = $2")
        .map_err(|e| sqlx::Error::Configuration(e.into()))?;
    sqlx::query_scalar(sql)
        .bind(valid_at)
        .bind(slug)
        .fetch_optional(pool)
        .await
}

/// UUID of the prefix `prefix` of `domain` at `valid_at`.
///
/// # Errors
///
/// Database errors.
pub async fn prefix_by_value(
    pool: &PgPool,
    domain: Uuid,
    prefix: &str,
    valid_at: OffsetDateTime,
) -> sqlx::Result<Option<Uuid>> {
    let sql = select_at(
        "prefix",
        &["id"],
        AsOf::Now,
        "domain_id = $2 AND prefix = $3::cidr",
    )
    .map_err(|e| sqlx::Error::Configuration(e.into()))?;
    sqlx::query_scalar(sql)
        .bind(valid_at)
        .bind(domain)
        .bind(prefix)
        .fetch_optional(pool)
        .await
}

/// UUID of the range `first`–`last` of `domain` at `valid_at` (both bounds must match).
///
/// # Errors
///
/// Database errors.
pub async fn range_by_bounds(
    pool: &PgPool,
    domain: Uuid,
    first: &str,
    last: &str,
    valid_at: OffsetDateTime,
) -> sqlx::Result<Option<Uuid>> {
    let sql = select_at(
        "ip_range",
        &["id"],
        AsOf::Now,
        "domain_id = $2 AND range = inetrange($3::inet, $4::inet, '[]')",
    )
    .map_err(|e| sqlx::Error::Configuration(e.into()))?;
    sqlx::query_scalar(sql)
        .bind(valid_at)
        .bind(domain)
        .bind(first)
        .bind(last)
        .fetch_optional(pool)
        .await
}

/// UUID of the address `address` of `domain` at `valid_at`.
///
/// # Errors
///
/// Database errors.
pub async fn address_by_value(
    pool: &PgPool,
    domain: Uuid,
    address: &str,
    valid_at: OffsetDateTime,
) -> sqlx::Result<Option<Uuid>> {
    let sql = select_at(
        "address",
        &["id"],
        AsOf::Now,
        "domain_id = $2 AND address = $3::inet",
    )
    .map_err(|e| sqlx::Error::Configuration(e.into()))?;
    sqlx::query_scalar(sql)
        .bind(valid_at)
        .bind(domain)
        .bind(address)
        .fetch_optional(pool)
        .await
}

/// The kind and the natural key, as stored, of each of `ids` that exists at `valid_at` as
/// asserted at `as_of` (spec 006, REQ-006-42), in one query: the slug of an entity, a
/// site, a domain or a group of VLANs; the text of the `cidr` of a prefix, of the
/// `inetrange` of a range and of the `inet` of an address; `<vid> · <name>` for a VLAN;
/// `None` for an object of a module, whose key its module gives. Kinds as
/// [`crate::property_read::kind_names`] names them; absent objects are left out.
///
/// # Errors
///
/// Database errors.
pub async fn natural_keys(
    pool: &PgPool,
    ids: &[Uuid],
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Vec<(Uuid, String, Option<String>)>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut sql = Vec::with_capacity(KEYED.len() + 1);
    for (kind, table, columns, key) in KEYED {
        let relation =
            snapshot(table, columns).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        sql.push(format!(
            "SELECT x.id, '{kind}'::text, {key} FROM {relation} x WHERE x.id = ANY($3)"
        ));
    }
    let modules = snapshot(MODULE_OBJECT.0, MODULE_OBJECT.1)
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    sql.push(format!(
        "SELECT x.id, x.kind, NULL::text FROM {modules} x WHERE x.id = ANY($3)"
    ));
    sqlx::query_as(AssertSqlSafe(sql.join(" UNION ALL ")))
        .bind(valid_at)
        .bind(as_of)
        .bind(ids)
        .fetch_all(pool)
        .await
}

/// The kinds of the core that have a natural key ([`natural_keys`]): kind, table, columns
/// read, key as an SQL expression of the row `x`.
const KEYED: [(&str, &str, &[&str], &str); 8] = [
    ("entity", "entity", &["id", "slug"], "x.slug"),
    ("site", "site", &["id", "slug"], "x.slug"),
    ("domain", "addressing_domain", &["id", "slug"], "x.slug"),
    ("prefix", "prefix", &["id", "prefix"], "x.prefix::text"),
    ("range", "ip_range", &["id", "range"], "x.range::text"),
    ("address", "address", &["id", "address"], "x.address::text"),
    ("vlan_group", "vlan_group", &["id", "slug"], "x.slug"),
    (
        "vlan",
        "vlan",
        &["id", "vid", "name"],
        "x.vid::text || ' · ' || x.name",
    ),
];

/// The objects of the modules, whose keys their modules give.
const MODULE_OBJECT: (&str, &[&str]) = ("module_object", &["id", "kind"]);

/// [`natural_keys`] of objects each read at its own date of validity, as asserted at
/// `as_of`, in one query whatever the number of pairs (spec 006, task 006-33i): for each
/// pair `(id, valid_at)` of `pairs` whose object existed then, the pair, its kind and its
/// key as stored (`None` for an object of a module); the others left out.
///
/// # Errors
///
/// Database errors.
pub async fn natural_keys_at(
    pool: &PgPool,
    pairs: &[(Uuid, OffsetDateTime)],
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Vec<(Uuid, OffsetDateTime, String, Option<String>)>> {
    type Row = (i64, String, Option<String>);
    if pairs.is_empty() {
        return Ok(Vec::new());
    }
    let (ids, dates): (Vec<Uuid>, Vec<OffsetDateTime>) = pairs.iter().copied().unzip();
    let relation = |table: &str, columns: &[&str]| {
        crate::temporal::snapshot_at(table, columns, "p.at")
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))
    };
    let mut sql = Vec::with_capacity(KEYED.len() + 1);
    for (kind, table, columns, key) in KEYED {
        let relation = relation(table, columns)?;
        sql.push(format!(
            "SELECT p.n, '{kind}'::text, {key} FROM p CROSS JOIN LATERAL {relation} x
              WHERE x.id = p.id"
        ));
    }
    let modules = relation(MODULE_OBJECT.0, MODULE_OBJECT.1)?;
    sql.push(format!(
        "SELECT p.n, x.kind, NULL::text FROM p CROSS JOIN LATERAL {modules} x
          WHERE x.id = p.id"
    ));
    // `$2` is the `as_of` of the relations; the pairs by their rank, not by a date that
    // the round trip would have to give back alike.
    let sql = format!(
        "WITH p AS MATERIALIZED (
             SELECT * FROM unnest($1::uuid[], $3::timestamptz[]) WITH ORDINALITY AS p(id, at, n))
         {}",
        sql.join(" UNION ALL ")
    );
    let rows: Vec<Row> = sqlx::query_as(AssertSqlSafe(sql))
        .bind(&ids)
        .bind(as_of)
        .bind(&dates)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(n, kind, key)| {
            let (id, at) = *pairs.get(usize::try_from(n).ok()?.checked_sub(1)?)?;
            Some((id, at, kind, key))
        })
        .collect())
}

/// Records that `object` is known as `external_id` by `source`.
///
/// # Errors
///
/// Database errors, including a unique violation when the external identifier already
/// designates another object for this source (`core.external_id.duplicate`).
pub async fn bind_external<'e>(
    executor: impl PgExecutor<'e>,
    source: &str,
    external_id: &str,
    object: Uuid,
    object_type: &str,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO external_id (source, external_id, object_id, object_type) VALUES ($1, $2, $3, $4)")
        .bind(source)
        .bind(external_id)
        .bind(object)
        .bind(object_type)
        .execute(executor)
        .await
        .map(|_| ())
}

/// Object designated by `external_id` for `source`, with its type.
///
/// # Errors
///
/// Database errors.
pub async fn by_external(
    pool: &PgPool,
    source: &str,
    external_id: &str,
) -> sqlx::Result<Option<(Uuid, String)>> {
    sqlx::query_as(
        "SELECT object_id, object_type FROM external_id WHERE source = $1 AND external_id = $2",
    )
    .bind(source)
    .bind(external_id)
    .fetch_optional(pool)
    .await
}
