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
pub async fn ancestors_as_of(
    pool: &PgPool,
    object: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Node>> {
    let found: Option<(Uuid, String)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH {} SELECT domain_id, prefix::text FROM pf WHERE id = $3
         UNION ALL SELECT domain_id, host(lower(range)) FROM rg WHERE id = $3
         UNION ALL SELECT domain_id, host(address) FROM ad WHERE id = $3",
        relations(as_of)?
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(object)
    .fetch_optional(pool)
    .await?;
    match found {
        Some((domain, value)) => ancestors_at(pool, domain, &value, valid_at, as_of, scope).await,
        None => Ok(Vec::new()),
    }
}

/// [`ancestors_as_of`] of the prefix `parent`, read before: its domain and value are
/// parameters of the query, so that a past read keeps the per-domain index of the
/// history (task 006-12d).
///
/// # Errors
///
/// As [`ancestors`].
pub async fn ancestors_of(
    pool: &PgPool,
    parent: &PrefixObject,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Node>> {
    let value = parent.value.to_string();
    ancestors_at(pool, parent.domain, &value, valid_at, as_of, scope).await
}

/// The prefixes of `domain` strictly holding `value` (a prefix or an address), nearest
/// first, within what `scope` lets read.
#[tracing::instrument(name = "SELECT ancestors", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT ancestors",
))]
async fn ancestors_at(
    pool: &PgPool,
    domain: Uuid,
    value: &str,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Node>> {
    let prefix_readable = readable("p.id", "'prefix'", "p.domain_id", "p.prefix", "$1");
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH {},
              s AS MATERIALIZED (SELECT * FROM kartenn_scopes($5))
         SELECT p.id, p.prefix::text, p.nature FROM pf p
         WHERE p.domain_id = $3 AND p.prefix >> $4::inet
           AND ($5::jsonb IS NULL OR {prefix_readable})
         ORDER BY masklen(p.prefix) DESC",
        relations(as_of)?
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(domain)
    .bind(value)
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

/// [`children_in`] valid at `valid_at` as asserted at `as_of` (now when `None`); empty if
/// `prefix` is not a prefix then.
///
/// # Errors
///
/// As [`children`].
pub async fn children_as_of(
    pool: &PgPool,
    prefix: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    scope: &ReadScope,
) -> sqlx::Result<Vec<Node>> {
    match prefix_as_of(pool, prefix, valid_at, as_of).await? {
        Some(parent) => children_of(pool, &parent, valid_at, as_of, scope, None, None).await,
        None => Ok(Vec::new()),
    }
}

/// Position of a child in address order: its first address, then its rank (prefix,
/// range, address: the wider first at the same start), then its identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChildKey {
    /// First address.
    pub start: Addr,
    /// 0 prefix, 1 range, 2 address.
    pub rank: i16,
    /// Identifier.
    pub id: Uuid,
}

impl Node {
    /// Its position among the children of its parent.
    #[must_use]
    pub const fn key(&self) -> ChildKey {
        match *self {
            Self::Prefix { id, value, .. } => ChildKey {
                start: value.network(),
                rank: 0,
                id,
            },
            Self::Range { id, first, .. } => ChildKey {
                start: first,
                rank: 1,
                id,
            },
            Self::Address { id, value } => ChildKey {
                start: value,
                rank: 2,
                id,
            },
        }
    }
}

/// The direct children of the prefix `parent`, read before, valid at `valid_at` as
/// asserted at `as_of`, within what `scope` lets read: at most `limit` of them (all when
/// `None`) after the key `after`, in [`ChildKey`] order (REQ-002-42, REQ-006-7). The
/// domain and the value of `parent` are parameters of the query, so that a past read
/// keeps the per-domain index of the history (task 006-12d).
///
/// # Errors
///
/// As [`children`].
#[tracing::instrument(name = "SELECT children", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT children",
))]
pub async fn children_of(
    pool: &PgPool,
    parent: &PrefixObject,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    scope: &ReadScope,
    after: Option<&ChildKey>,
    limit: Option<i64>,
) -> sqlx::Result<Vec<Node>> {
    // kind: 'p' prefix (a = value, b = nature), 'r' range (a, b = bounds, c = usage),
    // 'a' address (a = value); start: the first address, without mask, for the order.
    let (prefix_readable, range_readable, address_readable) = (
        readable("q.id", "'prefix'", "q.domain_id", "q.prefix", "$1"),
        readable("g.id", "'range'", "g.domain_id", "lower(g.range)", "$1"),
        readable("a.id", "'address'", "a.domain_id", "a.address", "$1"),
    );
    let rows: Vec<ChildRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH {},
              s AS MATERIALIZED (SELECT * FROM kartenn_scopes($5))
         SELECT kind, id, a, b, c FROM (
           SELECT 'p' AS kind, q.id, q.prefix::text AS a, q.nature AS b, NULL AS c,
                  host(q.prefix)::inet AS start, 0 AS rank FROM pf q
           WHERE q.domain_id = $3 AND q.prefix << $4::cidr
             AND NOT EXISTS (SELECT 1 FROM pf r WHERE r.domain_id = $3
                             AND r.prefix << $4::cidr AND q.prefix << r.prefix)
             AND ($5::jsonb IS NULL OR {prefix_readable})
           UNION ALL
           SELECT 'r', g.id, host(lower(g.range)), host(upper(g.range)), g.usage,
                  host(lower(g.range))::inet, 1 FROM rg g
           WHERE g.domain_id = $3 AND lower(g.range) <<= $4::cidr AND upper(g.range) <<= $4::cidr
             AND NOT EXISTS (SELECT 1 FROM pf r WHERE r.domain_id = $3
                             AND r.prefix << $4::cidr AND lower(g.range) <<= r.prefix)
             AND ($5::jsonb IS NULL OR {range_readable})
           UNION ALL
           SELECT 'a', a.id, host(a.address), NULL, NULL, host(a.address)::inet, 2 FROM ad a
           WHERE a.domain_id = $3 AND a.address <<= $4::cidr
             AND NOT EXISTS (SELECT 1 FROM pf r WHERE r.domain_id = $3
                             AND r.prefix << $4::cidr AND a.address <<= r.prefix)
             AND ($5::jsonb IS NULL OR {address_readable})
         ) c
         WHERE $6::inet IS NULL OR (c.start, c.rank, c.id) > ($6::inet, $7::int, $8::uuid)
         ORDER BY c.start, c.rank, c.id
         LIMIT $9",
        relations(as_of)?
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(parent.domain)
    .bind(parent.value.to_string())
    .bind(scope.to_json())
    .bind(after.map(|k| k.start.to_string()))
    .bind(after.map(|k| i32::from(k.rank)))
    .bind(after.map(|k| k.id))
    .bind(limit)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(node_of).collect()
}

/// The node of a row read by [`children_of`] or [`roots_of`].
fn node_of((kind, id, a, b, c): ChildRow) -> sqlx::Result<Node> {
    match (kind.as_str(), b, c) {
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
    }
}

/// An `inet` below every address of every family, the lower bound of an open gap.
const LOWEST: &str = "'0.0.0.0/0'::inet";

/// The scan of a gap `p` of [`roots_of`]: past `p.after` and the cursor, before `p.before`
/// if the gap is bounded, in order. Each bound is a plain comparison so that the keyset
/// index of the table serves it: one branch for a bounded gap, one for the last one.
/// `select` reads the table as `t`, ordered by `t.{value}`; `bound(x)` is the address `x`
/// as a value of that column; `exact` what the index leaves to check, readability included.
fn gap(select: &str, value: &str, bound: &dyn Fn(&str) -> String, exact: &str) -> String {
    let common = format!(
        "{select} WHERE t.domain_id = $3 AND t.{value} > {after} AND t.{value} >= {from} {exact}",
        after = bound("p.after"),
        from = bound(&format!("coalesce($5::inet, {LOWEST})")),
    );
    let order = format!("ORDER BY t.{value}, t.id LIMIT $8 + 1");
    format!(
        "({common} AND p.before IS NOT NULL AND t.{value} < {before} {order})
         UNION ALL
         ({common} AND p.before IS NULL {order})",
        before = bound("p.before"),
    )
}

/// The roots of the tree of the domain `domain`: its prefixes, ranges and addresses that
/// no prefix of the domain contains, valid at `valid_at` as asserted at `as_of`, within
/// what `scope` lets read: at most `limit` of them (all when `None`) after the key `after`,
/// in [`ChildKey`] order (REQ-007-39).
///
/// The roots are disjoint and the order of `cidr` is that of a depth-first walk, so the
/// root prefixes are walked one index probe each, from the first prefix at or after
/// `after` (or the outermost one holding it) to the first prefix past the last address of
/// the previous one; the walk stops once it holds one readable root more than `limit`,
/// enough to fill the page. The ranges and addresses that are roots lie in the gaps
/// between two root prefixes, each read in address order on its own.
///
/// # Errors
///
/// As [`children`].
#[tracing::instrument(name = "SELECT roots", skip_all, fields(
    db.system.name = "postgresql",
    db.operation.name = "SELECT",
    db.query.summary = "SELECT roots",
))]
pub async fn roots_of(
    pool: &PgPool,
    domain: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    scope: &ReadScope,
    after: Option<&ChildKey>,
    limit: Option<i64>,
) -> sqlx::Result<Vec<Node>> {
    let max = |x: &str| format!("CASE family({x}) WHEN 4 THEN 32 ELSE 128 END");
    let (first, next, root) = (
        readable("f.id", "'prefix'", "$3", "f.prefix", "$1"),
        readable("x.id", "'prefix'", "$3", "x.prefix", "$1"),
        readable("r.id", "'prefix'", "$3", "r.prefix", "$1"),
    );
    let (max_w, max_p) = (max("w.prefix"), max("prefix"));
    let ranges = gap(
        "SELECT t.id, t.range, t.usage FROM rg t",
        "range",
        &|x| format!("inetrange({x}, {x}, '[]')"),
        &format!(
            "AND lower(t.range) > p.after AND ($4::jsonb IS NULL OR {})",
            readable("t.id", "'range'", "t.domain_id", "lower(t.range)", "$1")
        ),
    );
    let addresses = gap(
        "SELECT t.id, t.address FROM ad t",
        "address",
        &|x| x.to_owned(),
        &format!(
            "AND ($4::jsonb IS NULL OR {})",
            readable("t.id", "'address'", "t.domain_id", "t.address", "$1")
        ),
    );
    // walk: the root prefixes in order, `n` the readable ones so far; gaps: the open
    // intervals of addresses between two of them (`after`, `before`), the last one only if
    // the walk reached the end of the domain.
    let sql = format!(
        "WITH RECURSIVE {},
              s AS MATERIALIZED (SELECT * FROM kartenn_scopes($4)),
              walk (id, prefix, nature, n) AS (
                (SELECT f.id, f.prefix, f.nature,
                        CASE WHEN $4::jsonb IS NULL OR {first} THEN 1 ELSE 0 END
                 FROM ((SELECT c.id, c.prefix, c.nature, 0 AS o FROM pf c
                        WHERE $5::inet IS NOT NULL AND c.domain_id = $3
                          AND c.prefix >>= $5::inet
                        ORDER BY masklen(c.prefix) LIMIT 1)
                       UNION ALL
                       (SELECT q.id, q.prefix, q.nature, 1 FROM pf q
                        WHERE q.domain_id = $3 AND ($5::inet IS NULL OR q.prefix >= $5::cidr)
                        ORDER BY q.prefix LIMIT 1)) f
                 ORDER BY f.o LIMIT 1)
                UNION ALL
                SELECT x.id, x.prefix, x.nature,
                       w.n + CASE WHEN $4::jsonb IS NULL OR {next} THEN 1 ELSE 0 END
                FROM walk w CROSS JOIN LATERAL (
                  SELECT q.id, q.prefix, q.nature FROM pf q
                  WHERE q.domain_id = $3
                    AND q.prefix > set_masklen(broadcast(w.prefix), {max_w})::cidr
                  ORDER BY q.prefix LIMIT 1) x
                WHERE $8::bigint IS NULL OR w.n <= $8
              ),
              roots AS MATERIALIZED (
                SELECT id, prefix, nature, n, host(prefix)::inet AS lo,
                       set_masklen(broadcast(prefix), {max_p})::inet AS hi
                FROM walk
              ),
              gaps AS (
                SELECT coalesce(lag(r.hi) OVER (ORDER BY r.prefix), {LOWEST}) AS after,
                       r.lo AS before
                FROM roots r
                UNION ALL
                SELECT coalesce((SELECT r.hi FROM roots r ORDER BY r.prefix DESC LIMIT 1),
                                {LOWEST}),
                       NULL
                WHERE $8::bigint IS NULL OR coalesce((SELECT max(n) FROM roots), 0) <= $8
              )
         SELECT kind, id, a, b, c FROM (
           SELECT 'p' AS kind, r.id, r.prefix::text AS a, r.nature AS b, NULL AS c,
                  r.lo AS start, 0 AS rank
           FROM roots r WHERE $4::jsonb IS NULL OR {root}
           UNION ALL
           SELECT 'r', g.id, host(lower(g.range)), host(upper(g.range)), g.usage,
                  host(lower(g.range))::inet, 1
           FROM gaps p CROSS JOIN LATERAL ({ranges}) g
           UNION ALL
           SELECT 'a', a.id, host(a.address), NULL, NULL, a.address, 2
           FROM gaps p CROSS JOIN LATERAL ({addresses}) a
         ) c
         WHERE $5::inet IS NULL OR (c.start, c.rank, c.id) > ($5::inet, $6::int, $7::uuid)
         ORDER BY c.start, c.rank, c.id
         LIMIT $8",
        relations(as_of)?
    );
    let rows: Vec<ChildRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(valid_at)
        .bind(as_of)
        .bind(domain)
        .bind(scope.to_json())
        .bind(after.map(|k| k.start.to_string()))
        .bind(after.map(|k| i32::from(k.rank)))
        .bind(after.map(|k| k.id))
        .bind(limit)
        .fetch_all(pool)
        .await?;
    rows.into_iter().map(node_of).collect()
}

/// Whether the addressing domain `id` exists at `valid_at` as asserted at `as_of` (now
/// when `None`).
///
/// # Errors
///
/// Database errors.
pub async fn domain_exists(
    pool: &PgPool,
    id: Uuid,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> sqlx::Result<bool> {
    let relation = match as_of {
        Some(_) => snapshot("addressing_domain", &["id"])
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        None => "(SELECT id FROM addressing_domain WHERE valid_period @> $1::timestamptz \
                  AND $2::timestamptz IS NULL)"
            .to_owned(),
    };
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH d AS NOT MATERIALIZED {relation} SELECT EXISTS (SELECT 1 FROM d WHERE id = $3)"
    )))
    .bind(valid_at)
    .bind(as_of)
    .bind(id)
    .fetch_one(pool)
    .await
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
    let content = children_of(pool, &p, valid_at, as_of, &ReadScope::all(), None, None).await?;
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
    let content = children_of(pool, &p, valid_at, as_of, &ReadScope::all(), None, None).await?;
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
    let content = children_of(pool, &p, valid_at, as_of, &ReadScope::all(), None, None).await?;
    Ok(occupancy::next_free_prefix(
        p.value,
        &prefixes_of(&content),
        len,
    ))
}
