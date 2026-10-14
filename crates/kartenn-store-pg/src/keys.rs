//! Natural keys and external identifiers (REQ-002-40, 41).
//!
//! Every object has an immutable UUID; humans, imports and connectors may designate it
//! by a natural key instead, resolved at a date: a slug for entities, sites and domains,
//! `(domain, prefix)` and `(domain, address)` for addressing objects, or an external
//! identifier of a connector.

use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgExecutor, PgPool};

use crate::temporal::{AsOf, select_at};

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
