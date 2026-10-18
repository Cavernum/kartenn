//! Navigation in the deduced addressing tree, occupancy and next free values
//! (REQ-002-14, 42 to 44).
//!
//! The parent of an object is the smallest prefix of its domain that strictly contains it
//! at the date considered (D34); nothing is stored. Occupancy and next free values reuse
//! the pure functions of `kartenn_core::occupancy` on the direct content read here.

use crate::rbac::{ReadScope, readable};
use crate::temporal::snapshot;
use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::objects::{IpRange, Nature, PrefixObject, RangeUsage};
use kartenn_core::occupancy::{self, Occupancy};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

/// A node of the addressing tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// A prefix.
    Prefix {
        /// Identifier.
        id: Uuid,
        /// Value.
        value: Prefix,
        /// Container or network.
        nature: Nature,
    },
    /// A range of addresses (always the child of a network).
    Range {
        /// Identifier.
        id: Uuid,
        /// First address.
        first: Addr,
        /// Last address, included.
        last: Addr,
        /// DHCP, reserved or static.
        usage: RangeUsage,
    },
    /// An address.
    Address {
        /// Identifier.
        id: Uuid,
        /// Value.
        value: Addr,
    },
}

impl Node {
    /// Its identifier.
    #[must_use]
    pub const fn id(&self) -> Uuid {
        match self {
            Self::Prefix { id, .. } | Self::Range { id, .. } | Self::Address { id, .. } => *id,
        }
    }
}

fn decode_err(e: impl std::fmt::Display) -> sqlx::Error {
    sqlx::Error::Decode(e.to_string().into())
}

fn nature(text: &str) -> Result<Nature, sqlx::Error> {
    match text {
        "container" => Ok(Nature::Container),
        "network" => Ok(Nature::Network),
        other => Err(decode_err(format!("unknown nature {other}"))),
    }
}

fn range_usage(text: &str) -> Result<RangeUsage, sqlx::Error> {
    match text {
        "dhcp" => Ok(RangeUsage::Dhcp),
        "reserved" => Ok(RangeUsage::Reserved),
        "static" => Ok(RangeUsage::Static),
        other => Err(decode_err(format!("unknown range usage {other}"))),
    }
}

fn addr(text: &str) -> Result<Addr, sqlx::Error> {
    Addr::parse_any(&ALL, text).map_err(decode_err)
}

/// The relations `pf`, `rg` and `ad` of the prefixes, ranges and addresses valid at `$1`,
/// as asserted at `$2` (now when null), for a `WITH` clause. Without `as_of`, the current
/// tables alone: a `UNION ALL` with the partitioned history is not flattened, which loses
/// the per-domain index of the joins (a whole scan per read).
fn relations(as_of: Option<OffsetDateTime>) -> sqlx::Result<String> {
    let relation = |table: &str, columns: &[&str]| match as_of {
        Some(_) => snapshot(table, columns).map_err(|e| sqlx::Error::Protocol(e.to_string())),
        None => Ok(format!(
            "(SELECT {} FROM {table} WHERE valid_period @> $1::timestamptz \
              AND $2::timestamptz IS NULL)",
            columns.join(", ")
        )),
    };
    Ok(format!(
        "pf AS NOT MATERIALIZED {},
         rg AS NOT MATERIALIZED {},
         ad AS NOT MATERIALIZED {}",
        relation(
            "prefix",
            &[
                "id",
                "domain_id",
                "prefix",
                "nature",
                "point_to_point",
                "anycast"
            ]
        )?,
        relation("ip_range", &["id", "domain_id", "range", "usage"])?,
        relation("address", &["id", "domain_id", "address"])?,
    ))
}

/// Ancestors of a prefix, a range or an address at `at`, nearest first (REQ-002-14, 42).
/// A range lies in a single network, so the prefixes holding its first address hold it all.
///
/// # Errors
///
/// Database or decoding errors.
pub async fn ancestors(pool: &PgPool, object: Uuid, at: OffsetDateTime) -> sqlx::Result<Vec<Node>> {
    ancestors_as_of(pool, object, at, None, &ReadScope::all()).await
}

/// [`ancestors`] valid at `valid_at` as asserted at `as_of` (now when `None`), within what
/// `scope` lets read (REQ-005-24).
///
/// # Errors
///
/// As [`ancestors`].
#[tracing::instrument(name = "SELECT ancestors", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT ancestors",
))]
pub async fn ancestors_as_of(
    pool: &PgPool,
    object: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Node>> {
    let prefix_readable = readable("p.id", "'prefix'", "p.domain_id", "p.prefix", "$1");
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH {},
              o AS (SELECT domain_id, prefix::inet AS v FROM pf WHERE id = $3
                    UNION ALL SELECT domain_id, lower(range) FROM rg WHERE id = $3
                    UNION ALL SELECT domain_id, address FROM ad WHERE id = $3),
              s AS MATERIALIZED (SELECT * FROM kartenn_scopes($4))
         SELECT p.id, p.prefix::text, p.nature FROM pf p, o
         WHERE p.domain_id = o.domain_id AND p.prefix >> o.v
           AND ($4::jsonb IS NULL OR {prefix_readable})
         ORDER BY masklen(p.prefix) DESC",
        relations(as_of)?
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(object)
    .bind(scope.to_json())
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(id, value, n)| {
            Ok(Node::Prefix {
                id,
                value: Prefix::parse_any(&ALL, &value).map_err(decode_err)?,
                nature: nature(&n)?,
            })
        })
        .collect()
}

/// Row of [`children`]: kind, identifier and up to three text columns.
type ChildRow = (String, Uuid, String, Option<String>, Option<String>);

/// Direct children of a prefix at `at`: prefixes, ranges and addresses whose deduced parent
/// it is, in address order (REQ-002-42).
///
/// # Errors
///
/// Database or decoding errors.
pub async fn children(pool: &PgPool, prefix: Uuid, at: OffsetDateTime) -> sqlx::Result<Vec<Node>> {
    children_as_of(pool, prefix, at, None, &ReadScope::all()).await
}

/// [`children`] within what `scope` lets read (REQ-005-24).
///
/// # Errors
///
/// As [`children`].
pub async fn children_in(
    pool: &PgPool,
    prefix: Uuid,
    at: OffsetDateTime,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Node>> {
    children_as_of(pool, prefix, at, None, scope).await
}

/// [`children_in`] valid at `valid_at` as asserted at `as_of` (now when `None`).
///
/// # Errors
///
/// As [`children`].
#[tracing::instrument(name = "SELECT children", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT children",
))]
pub async fn children_as_of(
    pool: &PgPool,
    prefix: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Node>> {
    // kind: 'p' prefix (a = value, b = nature), 'r' range (a, b = bounds, c = usage),
    // 'a' address (a = value).
    let (prefix_readable, range_readable, address_readable) = (
        readable("q.id", "'prefix'", "q.domain_id", "q.prefix", "$1"),
        readable("g.id", "'range'", "g.domain_id", "lower(g.range)", "$1"),
        readable("a.id", "'address'", "a.domain_id", "a.address", "$1"),
    );
    let rows: Vec<ChildRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH {},
              p AS (SELECT domain_id, prefix FROM pf WHERE id = $3),
              s AS MATERIALIZED (SELECT * FROM kartenn_scopes($4))
         SELECT 'p', q.id, q.prefix::text, q.nature, NULL FROM pf q, p
         WHERE q.domain_id = p.domain_id AND q.prefix << p.prefix
           AND NOT EXISTS (SELECT 1 FROM pf r WHERE r.domain_id = p.domain_id
                           AND r.prefix << p.prefix AND q.prefix << r.prefix)
           AND ($4::jsonb IS NULL OR {prefix_readable})
         UNION ALL
         SELECT 'r', g.id, host(lower(g.range)), host(upper(g.range)), g.usage FROM rg g, p
         WHERE g.domain_id = p.domain_id AND lower(g.range) <<= p.prefix AND upper(g.range) <<= p.prefix
           AND NOT EXISTS (SELECT 1 FROM pf r WHERE r.domain_id = p.domain_id
                           AND r.prefix << p.prefix AND lower(g.range) <<= r.prefix)
           AND ($4::jsonb IS NULL OR {range_readable})
         UNION ALL
         SELECT 'a', a.id, host(a.address), NULL, NULL FROM ad a, p
         WHERE a.domain_id = p.domain_id AND a.address <<= p.prefix
           AND NOT EXISTS (SELECT 1 FROM pf r WHERE r.domain_id = p.domain_id
                           AND r.prefix << p.prefix AND a.address <<= r.prefix)
           AND ($4::jsonb IS NULL OR {address_readable})",
        relations(as_of)?
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(prefix)
    .bind(scope.to_json())
    .fetch_all(pool)
    .await?;
    let mut nodes: Vec<Node> = rows
        .into_iter()
        .map(|(kind, id, a, b, c)| match (kind.as_str(), b, c) {
            ("p", Some(n), _) => Ok(Node::Prefix {
                id,
                value: Prefix::parse_any(&ALL, &a).map_err(decode_err)?,
                nature: nature(&n)?,
            }),
            ("r", Some(last), Some(usage)) => Ok(Node::Range {
                id,
                first: addr(&a)?,
                last: addr(&last)?,
                usage: range_usage(&usage)?,
            }),
            ("a", _, _) => Ok(Node::Address {
                id,
                value: addr(&a)?,
            }),
            _ => Err(decode_err(format!("malformed child row of kind {kind}"))),
        })
        .collect::<Result<_, sqlx::Error>>()?;
    // Address order; at the same start, the wider node first (prefix, range, address).
    nodes.sort_by_key(|n| match n {
        Node::Prefix { value, .. } => (value.network(), 0, value.prefix_len()),
        Node::Range { first, .. } => (*first, 1, 0),
        Node::Address { value, .. } => (*value, 2, 0),
    });
    Ok(nodes)
}

/// The prefix `id` valid at `valid_at` as asserted at `as_of` (now when `None`); `None`
/// if it is not a prefix or does not exist then.
///
/// # Errors
///
/// Database or decoding errors.
pub async fn prefix_as_of(
    pool: &PgPool,
    id: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<PrefixObject>> {
    let row: Option<(Uuid, String, String, bool, bool)> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "WITH {} SELECT domain_id, prefix::text, nature, point_to_point, anycast FROM pf WHERE id = $3",
            relations(as_of)?
        )))
        .bind(valid_at)
        .bind(as_of)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    row.map(|(domain, value, n, point_to_point, anycast)| {
        Ok(PrefixObject {
            id,
            domain,
            value: Prefix::parse_any(&ALL, &value).map_err(decode_err)?,
            nature: nature(&n)?,
            point_to_point,
            anycast,
        })
    })
    .transpose()
}

fn addresses_of(children: &[Node]) -> Vec<Addr> {
    children
        .iter()
        .filter_map(|n| {
            if let Node::Address { value, .. } = n {
                Some(*value)
            } else {
                None
            }
        })
        .collect()
}

fn ranges_of(children: &[Node], domain: Uuid) -> Vec<IpRange> {
    children
        .iter()
        .filter_map(|n| {
            if let Node::Range {
                id,
                first,
                last,
                usage,
            } = n
            {
                Some(IpRange {
                    id: *id,
                    domain,
                    first: *first,
                    last: *last,
                    usage: *usage,
                })
            } else {
                None
            }
        })
        .collect()
}

fn prefixes_of(children: &[Node]) -> Vec<Prefix> {
    children
        .iter()
        .filter_map(|n| {
            if let Node::Prefix { value, .. } = n {
                Some(*value)
            } else {
                None
            }
        })
        .collect()
}

/// Occupancy of a prefix at `at` (REQ-002-43); `None` if it does not exist then. A
/// network counts its addresses plus its DHCP and reserved ranges in full.
///
/// # Errors
///
/// Database or decoding errors.
pub async fn occupancy(
    pool: &PgPool,
    prefix: Uuid,
    at: OffsetDateTime,
) -> sqlx::Result<Option<Occupancy>> {
    occupancy_as_of(pool, prefix, at, None).await
}

/// [`occupancy`] valid at `valid_at` as asserted at `as_of` (now when `None`). The whole
/// content counts, readable or not: the occupancy is a measure of the prefix.
///
/// # Errors
///
/// As [`occupancy`].
#[tracing::instrument(name = "SELECT occupancy", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT occupancy",
))]
pub async fn occupancy_as_of(
    pool: &PgPool,
    prefix: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<Occupancy>> {
    let Some(p) = prefix_as_of(pool, prefix, valid_at, as_of).await? else {
        return Ok(None);
    };
    let content = children_as_of(pool, prefix, valid_at, as_of, &ReadScope::all()).await?;
    Ok(Some(match p.nature {
        Nature::Network => occupancy::network_occupancy(
            &p,
            &ranges_of(&content, p.domain),
            &addresses_of(&content),
        ),
        Nature::Container => occupancy::container_occupancy(p.value, &prefixes_of(&content)),
    }))
}

/// First free address of a network at `at`, outside DHCP and reserved ranges
/// (REQ-002-44).
///
/// # Errors
///
/// Database or decoding errors.
pub async fn next_free_address(
    pool: &PgPool,
    network: Uuid,
    at: OffsetDateTime,
) -> sqlx::Result<Option<Addr>> {
    next_free_address_as_of(pool, network, at, None).await
}

/// [`next_free_address`] valid at `valid_at` as asserted at `as_of` (now when `None`):
/// every address taken counts, readable or not.
///
/// # Errors
///
/// As [`next_free_address`].
#[tracing::instrument(name = "SELECT next_free_address", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT next_free_address",
))]
pub async fn next_free_address_as_of(
    pool: &PgPool,
    network: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<Addr>> {
    let Some(p) = prefix_as_of(pool, network, valid_at, as_of).await? else {
        return Ok(None);
    };
    if p.nature != Nature::Network {
        return Ok(None);
    }
    let content = children_as_of(pool, network, valid_at, as_of, &ReadScope::all()).await?;
    Ok(occupancy::next_free_address(
        &p,
        &ranges_of(&content, p.domain),
        &addresses_of(&content),
    ))
}

/// First free prefix of length `len` in a container at `at` (REQ-002-44).
///
/// # Errors
///
/// Database or decoding errors.
pub async fn next_free_prefix(
    pool: &PgPool,
    container: Uuid,
    len: u8,
    at: OffsetDateTime,
) -> sqlx::Result<Option<Prefix>> {
    next_free_prefix_as_of(pool, container, len, at, None).await
}

/// [`next_free_prefix`] valid at `valid_at` as asserted at `as_of` (now when `None`):
/// every child prefix counts, readable or not.
///
/// # Errors
///
/// As [`next_free_prefix`].
#[tracing::instrument(name = "SELECT next_free_prefix", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT next_free_prefix",
))]
pub async fn next_free_prefix_as_of(
    pool: &PgPool,
    container: Uuid,
    len: u8,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<Option<Prefix>> {
    let Some(p) = prefix_as_of(pool, container, valid_at, as_of).await? else {
        return Ok(None);
    };
    if p.nature != Nature::Container {
        return Ok(None);
    }
    let content = children_as_of(pool, container, valid_at, as_of, &ReadScope::all()).await?;
    Ok(occupancy::next_free_prefix(
        p.value,
        &prefixes_of(&content),
        len,
    ))
}
