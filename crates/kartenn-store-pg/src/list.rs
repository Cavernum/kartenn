//! Lists of objects by pages, read by key (spec 006, REQ-006-7, 8; tasks 006-10, 12a):
//! sorted by natural key then UUID, a page starts after the last key of the previous one,
//! so a write between two pages neither skips nor repeats an element; each page applies
//! the read scope of its caller (005, REQ-005-24). Each object comes with the summary of
//! the object it links to (its domain, or its parent), read by the same query.

use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{AssertSqlSafe, PgPool};

use crate::rbac::{ReadScope, readable};
use crate::temporal::{TemporalError, history_of, snapshot};

/// A kind of object of the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Entity.
    Entity,
    /// Site.
    Site,
    /// Addressing domain.
    Domain,
    /// Prefix.
    Prefix,
    /// Range.
    Range,
    /// Address.
    Address,
}

/// Where the natural key of a kind lies.
struct Shape {
    table: &'static str,
    columns: &'static [&'static str],
    /// Column of the value, for the objects of a domain; `None` keys by slug.
    value: Option<(&'static str, &'static str)>,
    /// The linked object: column and table of its slug.
    link: Option<(&'static str, &'static str)>,
}

impl Kind {
    /// Every kind.
    pub const ALL: [Self; 6] = [
        Self::Entity,
        Self::Site,
        Self::Domain,
        Self::Prefix,
        Self::Range,
        Self::Address,
    ];

    /// Name, as in the rights (`core.<kind>.read`) and in [`crate::read::object`].
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Site => "site",
            Self::Domain => "domain",
            Self::Prefix => "prefix",
            Self::Range => "range",
            Self::Address => "address",
        }
    }

    /// Whether objects of this kind belong to a domain and are keyed by their value.
    #[must_use]
    pub const fn in_domain(self) -> bool {
        matches!(self, Self::Prefix | Self::Range | Self::Address)
    }

    const fn shape(self) -> Shape {
        match self {
            Self::Entity => Shape {
                table: "entity",
                columns: &["id", "slug", "parent_id"],
                value: None,
                link: Some(("parent_id", "entity")),
            },
            Self::Site => Shape {
                table: "site",
                columns: &["id", "slug", "parent_id"],
                value: None,
                link: Some(("parent_id", "site")),
            },
            Self::Domain => Shape {
                table: "addressing_domain",
                columns: &["id", "slug"],
                value: None,
                link: None,
            },
            Self::Prefix => Shape {
                table: "prefix",
                columns: &[
                    "id",
                    "domain_id",
                    "prefix",
                    "nature",
                    "point_to_point",
                    "anycast",
                    "public",
                ],
                value: Some(("prefix", "cidr")),
                link: Some(("domain_id", "addressing_domain")),
            },
            Self::Range => Shape {
                table: "ip_range",
                columns: &["id", "domain_id", "range", "usage", "public"],
                value: Some(("range", "inetrange")),
                link: Some(("domain_id", "addressing_domain")),
            },
            Self::Address => Shape {
                table: "address",
                columns: &["id", "domain_id", "address", "usage", "public"],
                value: Some(("address", "inet")),
                link: Some(("domain_id", "addressing_domain")),
            },
        }
    }
}

impl Shape {
    /// The condition that the row `x` is readable under the scope `s`.
    fn readable(&self, kind: Kind) -> String {
        let name = format!("'{}'", kind.as_str());
        match (kind, self.value) {
            (Kind::Domain, _) => readable("x.id", &name, "x.id", "NULL::inet", "$1"),
            (Kind::Range, _) => readable("x.id", &name, "x.domain_id", "lower(x.range)", "$1"),
            (_, Some((value, _))) => {
                readable("x.id", &name, "x.domain_id", &format!("x.{value}"), "$1")
            }
            (_, None) => readable("x.id", &name, "NULL::uuid", "NULL::inet", "$1"),
        }
    }
}

/// The position of an object in its list: domain (for the objects of a domain), value
/// (CIDR, address or range text) or slug, identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// Addressing domain; `None` for a kind keyed by slug.
    pub domain: Option<Uuid>,
    /// Value or slug, as text.
    pub value: String,
    /// Identifier.
    pub id: Uuid,
}

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

/// An object of a list.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Position in the list.
    pub key: Key,
    /// Attributes, as in a read by identifier ([`crate::read::object`]).
    pub fields: serde_json::Value,
    /// Slug of the linked object (domain or parent) at the same dates; `None` without one.
    pub link: Option<String>,
}

/// Which objects of a list: each non-empty list keeps the objects equal to one of its
/// elements. Values are canonical texts the caller has checked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Criteria<'a> {
    /// Slugs, for a kind keyed by slug.
    pub slugs: &'a [String],
    /// Slugs of the linked object: the domain, or the parent.
    pub links: &'a [String],
    /// Values (CIDR, address; first address of a range), for the objects of a domain.
    pub values: &'a [String],
    /// Identifiers.
    pub ids: &'a [Uuid],
    /// CIDR texts: the objects of a domain inside one of them (a prefix included in it,
    /// an address in it, a range whose bounds both are).
    pub within: &'a [String],
    /// The nature of a prefix, the usage of a range or of an address: one of them.
    pub usages: &'a [String],
}

fn sql_err(e: &TemporalError) -> sqlx::Error {
    sqlx::Error::Protocol(e.to_string())
}

/// The relation `l` of the linked objects at the dates of the read.
fn link_relation(table: &str) -> Result<String, TemporalError> {
    snapshot(table, &["id", "slug"])
}

fn objects_sql(kind: Kind) -> Result<String, TemporalError> {
    let shape = kind.shape();
    let relation = snapshot(shape.table, shape.columns)?;
    let visible = shape.readable(kind);
    let (key, after, order, value_filter) = match shape.value {
        Some((column, cast)) => {
            let first = if kind == Kind::Range {
                format!("lower(x.{column})")
            } else {
                format!("x.{column}")
            };
            let filter_cast = if kind == Kind::Range { "inet" } else { cast };
            (
                format!("x.domain_id, x.{column}::text, x.id"),
                format!("(x.domain_id, x.{column}, x.id) > ($4, $5::{cast}, $6)"),
                format!("x.domain_id, x.{column}, x.id"),
                format!("{first} = ANY($10::{filter_cast}[])"),
            )
        }
        None => (
            "NULL::uuid, x.slug, x.id".to_owned(),
            "(x.slug, x.id) > ($5, $6)".to_owned(),
            "x.slug, x.id".to_owned(),
            "false".to_owned(),
        ),
    };
    let (join, link) = match shape.link {
        Some((column, table)) => (
            format!("LEFT JOIN {} l ON l.id = x.{column}", link_relation(table)?),
            "l.slug",
        ),
        None => (String::new(), "NULL::text"),
    };
    let (within_filter, usage_filter) = match kind {
        Kind::Prefix => (
            "EXISTS (SELECT FROM unnest($12::cidr[]) w WHERE x.prefix <<= w)",
            "x.nature = ANY($13)",
        ),
        Kind::Range => (
            "EXISTS (SELECT FROM unnest($12::cidr[]) w
                     WHERE lower(x.range) <<= w AND upper(x.range) <<= w)",
            "x.usage = ANY($13)",
        ),
        Kind::Address => ("x.address <<= ANY($12::cidr[])", "x.usage = ANY($13)"),
        Kind::Entity | Kind::Site | Kind::Domain => ("false", "false"),
    };
    let slug_filter = if shape.value.is_none() {
        "x.slug = ANY($8)"
    } else {
        "false"
    };
    Ok(format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT {key}, to_jsonb(x), {link} FROM {relation} x {join}
         WHERE ($6::uuid IS NULL OR {after})
           AND (cardinality($8::text[]) = 0 OR {slug_filter})
           AND (cardinality($9::text[]) = 0 OR {link} = ANY($9))
           AND (cardinality($10::text[]) = 0 OR {value_filter})
           AND (cardinality($11::uuid[]) = 0 OR x.id = ANY($11))
           AND (cardinality($12::cidr[]) = 0 OR {within_filter})
           AND (cardinality($13::text[]) = 0 OR {usage_filter})
           AND ($3::jsonb IS NULL OR {visible})
         ORDER BY {order}
         LIMIT $7"
    ))
}

/// A row of a list: key, attributes, slug of the linked object.
type ListRow = (
    Option<Uuid>,
    String,
    Uuid,
    serde_json::Value,
    Option<String>,
);

/// At most `limit` objects of `kind` matching `criteria`, valid at `valid_at`, as
/// asserted at `as_of` (now when `None`), after `after`, within what `scope` lets read.
///
/// # Errors
///
/// Database errors.
#[allow(clippy::too_many_arguments)] // a list, its dates, its page and its reader
#[tracing::instrument(name = "SELECT objects", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT objects",
    kartenn.kind = kind.as_str(),
))]
pub async fn objects(
    pool: &PgPool,
    kind: Kind,
    criteria: &Criteria<'_>,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    after: Option<&Key>,
    limit: i64,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Row>> {
    let sql = objects_sql(kind).map_err(|e| sql_err(&e))?;
    let rows: Vec<ListRow> = sqlx::query_as(AssertSqlSafe(sql))
        .bind(valid_at)
        .bind(as_of)
        .bind(scope.to_json())
        .bind(after.and_then(|k| k.domain))
        .bind(after.map(|k| k.value.as_str()))
        .bind(after.map(|k| k.id))
        .bind(limit)
        .bind(criteria.slugs)
        .bind(criteria.links)
        .bind(criteria.values)
        .bind(criteria.ids)
        .bind(criteria.within)
        .bind(criteria.usages)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(domain, value, id, fields, link)| Row {
            key: Key { domain, value, id },
            fields,
            link,
        })
        .collect())
}

/// At most `limit` prefixes valid at `valid_at`, as asserted at `as_of` (now when
/// `None`), after `after`, within what `scope` lets read.
///
/// # Errors
///
/// Database errors.
pub async fn prefixes(
    pool: &PgPool,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    after: Option<&PrefixKey>,
    limit: i64,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Listed>> {
    let after = after.map(|k| Key {
        domain: Some(k.domain),
        value: k.prefix.clone(),
        id: k.id,
    });
    let rows = objects(
        pool,
        Kind::Prefix,
        &Criteria::default(),
        valid_at,
        as_of,
        after.as_ref(),
        limit,
        scope,
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let domain = row
                .key
                .domain
                .ok_or_else(|| sqlx::Error::Protocol("a prefix without domain".into()))?;
            Ok(Listed {
                key: PrefixKey {
                    domain,
                    prefix: row.key.value,
                    id: row.key.id,
                },
                fields: row.fields,
            })
        })
        .collect()
}

/// The slug of the object `id` of `kind` linked to (a domain, an entity or a site),
/// valid at `valid_at`, as asserted at `as_of` (now when `None`).
///
/// # Errors
///
/// Database errors.
pub async fn link(
    pool: &PgPool,
    kind: Kind,
    id: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<String>> {
    let Some((_, table)) = kind.shape().link else {
        return Ok(None);
    };
    let relation = link_relation(table).map_err(|e| sql_err(&e))?;
    sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT l.slug FROM {relation} l WHERE l.id = $3"
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// A version of an object, on both axes (REQ-004-18).
#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    /// Attributes, as in a read by identifier.
    pub fields: serde_json::Value,
    /// Slug of the linked object at the start of the validity, as asserted now.
    pub link: Option<String>,
    /// Start of the validity.
    pub valid_from: OffsetDateTime,
    /// End of the validity (excluded); `None` when open.
    pub valid_to: Option<OffsetDateTime>,
    /// When kartenn began to assert it.
    pub asserted_from: OffsetDateTime,
    /// When kartenn stopped asserting it; `None` while current.
    pub asserted_to: Option<OffsetDateTime>,
    /// The changeset that wrote it.
    pub changeset: Uuid,
}

/// The object of `kind` that `changeset` created: the one whose rows, current or past,
/// it wrote (a route by resource creates one object, 006-16). `None` if it wrote none.
///
/// # Errors
///
/// Database errors.
#[tracing::instrument(name = "SELECT created", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT created",
    kartenn.kind = kind.as_str(),
))]
pub async fn created(pool: &PgPool, kind: Kind, changeset: Uuid) -> sqlx::Result<Option<Uuid>> {
    let table = kind.shape().table;
    sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT id FROM {table} WHERE changeset_id = $1
         UNION ALL SELECT id FROM {table}_history WHERE changeset_id = $1 LIMIT 1"
    )))
    .bind(changeset)
    .fetch_optional(pool)
    .await
}

/// Every version of the object `id` of `kind`, ordered by start of validity then of
/// assertion; empty if it never existed.
///
/// # Errors
///
/// Database errors.
pub async fn history(pool: &PgPool, kind: Kind, id: Uuid) -> sqlx::Result<Vec<Version>> {
    history_page(pool, kind, id, None, None, None, None).await
}

/// [`history`] as known at `as_of` (now when `None`): the versions asserted then, a
/// version retracted later shown as current; only those valid at `valid_at` when given
/// (REQ-006-5); at most `limit` of them (all when `None`)
/// after the key `after`, the start of validity then of assertion of the last one read
/// (REQ-006-7).
///
/// # Errors
///
/// Database errors.
#[tracing::instrument(name = "SELECT history", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT history",
    kartenn.kind = kind.as_str(),
))]
pub async fn history_page(
    pool: &PgPool,
    kind: Kind,
    id: Uuid,
    valid_at: Option<OffsetDateTime>,
    as_of: Option<OffsetDateTime>,
    after: Option<(OffsetDateTime, OffsetDateTime)>,
    limit: Option<i64>,
) -> sqlx::Result<Vec<Version>> {
    let shape = kind.shape();
    versions_page(
        pool,
        shape.table,
        shape.columns,
        shape.link,
        None,
        id,
        valid_at,
        as_of,
        after,
        limit,
    )
    .await
}

/// The versions of the object `id` of the module type `kind` (`vlan.vlan`), by its record
/// in `module_object` (ADR 0026, clause 4), as [`history_page`] reads those of the core:
/// each changeset that writes the object leaves a version there (REQ-008-3). None for an
/// object of another type.
///
/// # Errors
///
/// Database errors.
#[allow(clippy::too_many_arguments)] // the history of the core, with the type
#[tracing::instrument(name = "SELECT history", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT history",
    kartenn.kind = kind,
))]
pub async fn module_history_page(
    pool: &PgPool,
    kind: &str,
    id: Uuid,
    valid_at: Option<OffsetDateTime>,
    as_of: Option<OffsetDateTime>,
    after: Option<(OffsetDateTime, OffsetDateTime)>,
    limit: Option<i64>,
) -> sqlx::Result<Vec<Version>> {
    versions_page(
        pool,
        "module_object",
        &["id", "kind", "parent_id", "domain_id"],
        None,
        Some(kind),
        id,
        valid_at,
        as_of,
        after,
        limit,
    )
    .await
}

/// The versions of `id` in `table` (its `columns`), with the slug of the object `link`
/// names, only those of `kind` when given.
#[allow(clippy::too_many_arguments)] // the parts of a page of versions
async fn versions_page(
    pool: &PgPool,
    table: &str,
    columns: &[&str],
    link: Option<(&str, &str)>,
    kind: Option<&str>,
    id: Uuid,
    valid_at: Option<OffsetDateTime>,
    as_of: Option<OffsetDateTime>,
    after: Option<(OffsetDateTime, OffsetDateTime)>,
    limit: Option<i64>,
) -> sqlx::Result<Vec<Version>> {
    type VersionRow = (
        serde_json::Value,
        Option<String>,
        OffsetDateTime,
        Option<OffsetDateTime>,
        OffsetDateTime,
        Option<OffsetDateTime>,
        Uuid,
    );
    let versions = history_of(table, columns).map_err(|e| sql_err(&e))?;
    let link = match link {
        Some((column, table)) => format!(
            "(SELECT l.slug FROM {table} l
              WHERE l.id = h.{column} AND l.valid_period @> lower(h.valid_period))"
        ),
        None => "NULL::text".to_owned(),
    };
    let sql = format!(
        "SELECT to_jsonb(h) - 'valid_period' - 'sys_start' - 'sys_end' - 'changeset_id',
                {link}, lower(h.valid_period), upper(h.valid_period), h.sys_start,
                CASE WHEN $2::timestamptz IS NULL OR h.sys_end <= $2 THEN h.sys_end END,
                h.changeset_id
         FROM ({}) h
         WHERE ($2::timestamptz IS NULL OR h.sys_start <= $2)
           AND ($6::timestamptz IS NULL OR h.valid_period @> $6::timestamptz)
           AND ($7::text IS NULL OR to_jsonb(h) ->> 'kind' = $7)
           AND ($3::timestamptz IS NULL
                OR (lower(h.valid_period), h.sys_start) > ($3::timestamptz, $4::timestamptz))
         ORDER BY lower(h.valid_period), h.sys_start
         LIMIT $5",
        versions.0
    );
    let rows: Vec<VersionRow> = sqlx::query_as(AssertSqlSafe(sql))
        .bind(id)
        .bind(as_of)
        .bind(after.map(|(valid_from, _)| valid_from))
        .bind(after.map(|(_, asserted_from)| asserted_from))
        .bind(limit)
        .bind(valid_at)
        .bind(kind)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(
            |(fields, link, valid_from, valid_to, asserted_from, asserted_to, changeset)| Version {
                fields,
                link,
                valid_from,
                valid_to,
                asserted_from,
                asserted_to,
                changeset,
            },
        )
        .collect())
}
