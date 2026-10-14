//! Navigation in the deduced addressing tree, occupancy and next free values
//! (REQ-002-14, 42 to 44).
//!
//! The parent of an object is the smallest prefix of its domain that strictly contains it
//! at the date considered (D34); nothing is stored. Occupancy and next free values reuse
//! the pure functions of `kartenn_core::occupancy` on the direct content read here.

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::objects::{Nature, PrefixObject};
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

/// Ancestors of a prefix or an address at `at`, nearest first (REQ-002-14, 42).
///
/// # Errors
///
/// Database or decoding errors.
pub async fn ancestors(pool: &PgPool, object: Uuid, at: OffsetDateTime) -> sqlx::Result<Vec<Node>> {
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "WITH o AS (
             SELECT domain_id, prefix::inet AS v FROM prefix WHERE id = $1 AND valid_period @> $2
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

/// Direct children of a prefix at `at`: prefixes and addresses whose deduced parent it is,
/// in address order (REQ-002-42).
///
/// # Errors
///
/// Database or decoding errors.
pub async fn children(pool: &PgPool, prefix: Uuid, at: OffsetDateTime) -> sqlx::Result<Vec<Node>> {
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "WITH p AS (SELECT domain_id, prefix FROM prefix WHERE id = $1 AND valid_period @> $2)
         SELECT q.id, q.prefix::text, q.nature FROM prefix q, p
         WHERE q.domain_id = p.domain_id AND q.prefix << p.prefix AND q.valid_period @> $2
           AND NOT EXISTS (SELECT 1 FROM prefix r WHERE r.domain_id = p.domain_id AND r.valid_period @> $2
                           AND r.prefix << p.prefix AND q.prefix << r.prefix)
         UNION ALL
         SELECT a.id, host(a.address), NULL FROM address a, p
         WHERE a.domain_id = p.domain_id AND a.address <<= p.prefix AND a.valid_period @> $2
           AND NOT EXISTS (SELECT 1 FROM prefix r WHERE r.domain_id = p.domain_id AND r.valid_period @> $2
                           AND r.prefix << p.prefix AND a.address <<= r.prefix)
         ORDER BY 2",
    )
    .bind(prefix)
    .bind(at)
    .fetch_all(pool)
    .await?;
    let mut nodes: Vec<Node> = rows
        .into_iter()
        .map(|(id, value, n)| match n {
            Some(n) => Ok(Node::Prefix {
                id,
                value: Prefix::parse_any(&ALL, &value).map_err(decode_err)?,
                nature: nature(&n)?,
            }),
            None => Ok(Node::Address {
                id,
                value: Addr::parse_any(&ALL, &value).map_err(decode_err)?,
            }),
        })
        .collect::<Result<_, sqlx::Error>>()?;
    nodes.sort_by_key(|n| match n {
        Node::Prefix { value, .. } => (value.network(), value.prefix_len()),
        Node::Address { value, .. } => (*value, value.family().width()),
    });
    Ok(nodes)
}

async fn prefix_object(
    pool: &PgPool,
    id: Uuid,
    at: OffsetDateTime,
) -> sqlx::Result<Option<PrefixObject>> {
    let row: Option<(Uuid, String, String, bool)> = sqlx::query_as(
        "SELECT domain_id, prefix::text, nature, point_to_point FROM prefix WHERE id = $1 AND valid_period @> $2",
    )
    .bind(id)
    .bind(at)
    .fetch_optional(pool)
    .await?;
    row.map(|(domain, value, n, point_to_point)| {
        Ok(PrefixObject {
            id,
            domain,
            value: Prefix::parse_any(&ALL, &value).map_err(decode_err)?,
            nature: nature(&n)?,
            point_to_point,
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

/// Occupancy of a prefix at `at` (REQ-002-43); `None` if it does not exist then.
/// Ranges are not stored yet: a network's occupancy counts its addresses.
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
        Nature::Network => occupancy::network_occupancy(&p, &[], &addresses_of(&content)),
        Nature::Container => occupancy::container_occupancy(p.value, &prefixes_of(&content)),
    }))
}

/// First free address of a network at `at` (REQ-002-44).
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
        &[],
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
