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
/// keeps the per-domain index of the history (task 006-12d). The children are the roots
/// of the subtree of `parent`, walked from the cursor as [`roots_of`] walks the domain: a
/// page costs its own rows, not the whole subtree (task 007-18).
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
    tops(
        pool,
        parent.domain,
        Some(&parent.value),
        valid_at,
        as_of,
        scope,
        after,
        limit,
    )
    .await
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

/// An ordered probe of `table`, read as `alias`: its first `limit` rows in `order` that meet
/// `cond`, valid at `$1` as asserted at `$2` (`past`) or now. In the past, the current table
/// and its history are probed each on its own keyset index, then merged: the `UNION ALL`
/// relation of [`relations`] would sort every row of the domain before the limit, at every
/// step of the walk (task 007-18). The history, partitioned by month, is probed only when
/// the domain has rows closed after `$2` (the relation `recent` of [`tops`]): a page read
/// shortly after its first one, under a cursor, costs as a read of the present.
fn probe(
    past: bool,
    table: &str,
    alias: &str,
    cols: &str,
    cond: &str,
    order: &str,
    limit: &str,
) -> String {
    let current = format!("{alias}.valid_period @> $1::timestamptz AND {cond}");
    if !past {
        return format!(
            "SELECT {cols} FROM {table} {alias} WHERE {current} ORDER BY {order} LIMIT {limit}"
        );
    }
    let asserted = format!("{alias}.sys_start <= $2::timestamptz");
    format!(
        "SELECT * FROM (
           (SELECT {cols} FROM {table} {alias} WHERE {current} AND {asserted}
            ORDER BY {order} LIMIT {limit})
           UNION ALL
           (SELECT {cols} FROM {table}_history {alias}
            WHERE (SELECT {table} FROM recent)
              AND {current} AND {asserted} AND {alias}.sys_end > $2::timestamptz
            ORDER BY {order} LIMIT {limit})) {alias}
         ORDER BY {order} LIMIT {limit}"
    )
}

/// The scan of a gap `p` of [`tops`]: past `p.after` and the cursor (or, without one, from
/// the first address of the prefix `$9` the walk stays in), before `p.before` if the gap is
/// bounded, else up to the last address of `$9`, in order. Each bound is a plain comparison
/// so that the keyset index of the table serves it: one branch for a bounded gap, one for
/// the last one. `table` is read as `t` (`cols`), ordered by `t.{value}`; `bound(x)` is the
/// address `x` as a value of that column; `exact` what the index leaves to check,
/// readability included.
fn gap(
    past: bool,
    (table, cols): (&str, &str),
    value: &str,
    bound: &dyn Fn(&str) -> String,
    exact: &str,
) -> String {
    let common = format!(
        "t.domain_id = $3 AND t.{value} > {after} AND t.{value} >= {from} {exact}",
        after = bound("p.after"),
        from = bound(&format!(
            "coalesce($5::inet, host($9::cidr)::inet, {LOWEST})"
        )),
    );
    let order = format!("t.{value}, t.id");
    let branch = |cond: String| probe(past, table, "t", cols, &cond, &order, "$8 + 1");
    format!(
        "({})
         UNION ALL
         ({})",
        branch(format!(
            "{common} AND p.before IS NOT NULL AND t.{value} < {}",
            bound("p.before")
        )),
        branch(format!(
            "{common} AND p.before IS NULL AND ($9::cidr IS NULL OR t.{value} <= {})",
            bound("host(broadcast($9::cidr))::inet")
        )),
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
    tops(pool, domain, None, valid_at, as_of, scope, after, limit).await
}

/// The nodes of the domain `domain` that no other prefix of the domain contains, those
/// inside the prefix `within` only when it is given: the roots of the domain, or the
/// direct children of `within` (the walk of [`roots_of`], bounded by `within`). The bounds
/// of `within` are plain comparisons of `cidr`: the prefixes strictly inside it are those
/// past it and up to the host route of its last address.
#[allow(clippy::too_many_arguments)] // the read and its page
#[allow(clippy::too_many_lines)] // one query, built from its parts
async fn tops(
    pool: &PgPool,
    domain: Uuid,
    within: Option<&Prefix>,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
    scope: &ReadScope,
    after: Option<&ChildKey>,
    limit: Option<i64>,
) -> sqlx::Result<Vec<Node>> {
    let max = |x: &str| format!("CASE family({x}) WHEN 4 THEN 32 ELSE 128 END");
    let max_in = max("$9::cidr");
    let inside = |x: &str| {
        format!(
            "($9::cidr IS NULL OR ({x} > $9::cidr \
              AND {x} <= set_masklen(broadcast($9::cidr), {max_in})::cidr))"
        )
    };
    let (inside_c, inside_q) = (inside("c.prefix"), inside("q.prefix"));
    let (first, next, root) = (
        readable("f.id", "'prefix'", "$3", "f.prefix", "$1"),
        readable("x.id", "'prefix'", "$3", "x.prefix", "$1"),
        readable("r.id", "'prefix'", "$3", "r.prefix", "$1"),
    );
    let (max_w, max_p) = (max("w.prefix"), max("prefix"));
    let past = as_of.is_some();
    // Whether each history holds rows of the domain closed after `$2` (see [`probe`]).
    let recent = if past {
        let closed = |table: &str| {
            format!(
                "EXISTS (SELECT 1 FROM {table}_history h \
                 WHERE h.domain_id = $3 AND h.sys_end > $2::timestamptz) AS {table}"
            )
        };
        format!(
            "recent AS MATERIALIZED (SELECT {}, {}, {}),",
            closed("prefix"),
            closed("ip_range"),
            closed("address")
        )
    } else {
        String::new()
    };
    let prefixes = |alias: &str, cols: &str, cond: &str, order: &str| {
        probe(past, "prefix", alias, cols, cond, order, "1")
    };
    let holder = prefixes(
        "c",
        "c.id, c.prefix, c.nature, 0 AS o",
        &format!(
            "$5::inet IS NOT NULL AND c.domain_id = $3 AND c.prefix >>= $5::inet AND {inside_c}"
        ),
        "masklen(c.prefix)",
    );
    let start = prefixes(
        "q",
        "q.id, q.prefix, q.nature, 1 AS o",
        &format!("q.domain_id = $3 AND ($5::inet IS NULL OR q.prefix >= $5::cidr) AND {inside_q}"),
        "q.prefix",
    );
    let step = prefixes(
        "q",
        "q.id, q.prefix, q.nature",
        &format!(
            "q.domain_id = $3 AND q.prefix > set_masklen(broadcast(w.prefix), {max_w})::cidr \
             AND {inside_q}"
        ),
        "q.prefix",
    );
    let ranges = gap(
        past,
        ("ip_range", "t.id, t.range, t.usage"),
        "range",
        &|x| format!("inetrange({x}, {x}, '[]')"),
        &format!(
            "AND lower(t.range) > p.after AND ($4::jsonb IS NULL OR {})",
            readable("t.id", "'range'", "t.domain_id", "lower(t.range)", "$1")
        ),
    );
    let addresses = gap(
        past,
        ("address", "t.id, t.address"),
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
        "WITH RECURSIVE {recent}
              s AS MATERIALIZED (SELECT * FROM kartenn_scopes($4)),
              walk (id, prefix, nature, n) AS (
                (SELECT f.id, f.prefix, f.nature,
                        CASE WHEN $4::jsonb IS NULL OR {first} THEN 1 ELSE 0 END
                 FROM (({holder}) UNION ALL ({start})) f
                 ORDER BY f.o LIMIT 1)
                UNION ALL
                SELECT x.id, x.prefix, x.nature,
                       w.n + CASE WHEN $4::jsonb IS NULL OR {next} THEN 1 ELSE 0 END
                FROM walk w CROSS JOIN LATERAL ({step}) x
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
         LIMIT $8"
    );
    // Planned with the values each time: after five runs a prepared statement falls back to
    // a generic plan, which cannot tell a null cursor or bound from a set one and walks
    // 10⁴ children in 150 ms instead of 2 ms (task 007-18).
    let rows: Vec<ChildRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .persistent(false)
        .bind(valid_at)
        .bind(as_of)
        .bind(domain)
        .bind(scope.to_json())
        .bind(after.map(|k| k.start.to_string()))
        .bind(after.map(|k| i32::from(k.rank)))
        .bind(after.map(|k| k.id))
        .bind(limit)
        .bind(within.map(ToString::to_string))
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
