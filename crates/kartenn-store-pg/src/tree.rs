//! Navigation in the deduced addressing tree, occupancy and next free values
//! (REQ-002-14, 42 to 44).
//!
//! The parent of an object is the smallest prefix of its domain that strictly contains it
//! at the date considered (D34); nothing is stored. Occupancy and next free values reuse
//! the pure functions of `kartenn_core::occupancy` on the direct content read here.

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

/// Ancestors of a prefix, a range or an address at `at`, nearest first (REQ-002-14, 42).
/// A range lies in a single network, so the prefixes holding its first address hold it all.
///
/// # Errors
///
/// Database or decoding errors.
pub async fn ancestors(pool: &PgPool, object: Uuid, at: OffsetDateTime) -> sqlx::Result<Vec<Node>> {
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "WITH o AS (
             SELECT domain_id, prefix::inet AS v FROM prefix WHERE id = $1 AND valid_period @> $2
             UNION ALL SELECT domain_id, lower(range) FROM ip_range WHERE id = $1 AND valid_period @> $2
             UNION ALL SELECT domain_id, address FROM address WHERE id = $1 AND valid_period @> $2)
         SELECT p.id, p.prefix::text, p.nature FROM prefix p, o
         WHERE p.domain_id = o.domain_id AND p.prefix >> o.v AND p.valid_period @> $2
         ORDER BY masklen(p.prefix) DESC",
    )
    .bind(object)
    .bind(at)
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
    // kind: 'p' prefix (a = value, b = nature), 'r' range (a, b = bounds, c = usage),
    // 'a' address (a = value).
    let rows: Vec<ChildRow> = sqlx::query_as(
        "WITH p AS (SELECT domain_id, prefix FROM prefix WHERE id = $1 AND valid_period @> $2)
         SELECT 'p', q.id, q.prefix::text, q.nature, NULL FROM prefix q, p
         WHERE q.domain_id = p.domain_id AND q.prefix << p.prefix AND q.valid_period @> $2
           AND NOT EXISTS (SELECT 1 FROM prefix r WHERE r.domain_id = p.domain_id AND r.valid_period @> $2
                           AND r.prefix << p.prefix AND q.prefix << r.prefix)
         UNION ALL
         SELECT 'r', g.id, host(lower(g.range)), host(upper(g.range)), g.usage FROM ip_range g, p
         WHERE g.domain_id = p.domain_id AND lower(g.range) <<= p.prefix AND upper(g.range) <<= p.prefix
           AND g.valid_period @> $2
           AND NOT EXISTS (SELECT 1 FROM prefix r WHERE r.domain_id = p.domain_id AND r.valid_period @> $2
                           AND r.prefix << p.prefix AND lower(g.range) <<= r.prefix)
         UNION ALL
         SELECT 'a', a.id, host(a.address), NULL, NULL FROM address a, p
         WHERE a.domain_id = p.domain_id AND a.address <<= p.prefix AND a.valid_period @> $2
           AND NOT EXISTS (SELECT 1 FROM prefix r WHERE r.domain_id = p.domain_id AND r.valid_period @> $2
                           AND r.prefix << p.prefix AND a.address <<= r.prefix)",
    )
    .bind(prefix)
    .bind(at)
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

async fn prefix_object(
    pool: &PgPool,
    id: Uuid,
    at: OffsetDateTime,
) -> sqlx::Result<Option<PrefixObject>> {
    let row: Option<(Uuid, String, String, bool, bool)> = sqlx::query_as(
        "SELECT domain_id, prefix::text, nature, point_to_point, anycast FROM prefix WHERE id = $1 AND valid_period @> $2",
    )
    .bind(id)
    .bind(at)
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
    let Some(p) = prefix_object(pool, prefix, at).await? else {
        return Ok(None);
    };
    let content = children(pool, prefix, at).await?;
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
    let Some(p) = prefix_object(pool, network, at).await? else {
        return Ok(None);
    };
    if p.nature != Nature::Network {
        return Ok(None);
    }
    let content = children(pool, network, at).await?;
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
    let Some(p) = prefix_object(pool, container, at).await? else {
        return Ok(None);
    };
    if p.nature != Nature::Container {
        return Ok(None);
    }
    let content = children(pool, container, at).await?;
    Ok(occupancy::next_free_prefix(
        p.value,
        &prefixes_of(&content),
        len,
    ))
}
