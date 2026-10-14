//! Loading the IANA special-purpose registries and reporting public-flag drift
//! (REQ-002-34).

use kartenn_core::registry::{Flag, SpecialRegistry};
use sqlx::PgPool;
use sqlx::types::Uuid;

fn flag(f: Flag) -> &'static str {
    match f {
        Flag::True => "true",
        Flag::False => "false",
        Flag::NotApplicable => "n/a",
        Flag::Unspecified => "unspecified",
    }
}

/// Inserts the entries of `registry` under its version, if not loaded yet (idempotent).
///
/// # Errors
///
/// Database errors.
pub async fn load(pool: &PgPool, registry: &SpecialRegistry) -> sqlx::Result<u64> {
    let mut inserted = 0;
    let mut tx = pool.begin().await?;
    for e in registry.entries() {
        inserted += sqlx::query(
            "INSERT INTO special_registry (registry_version, block, name, rfc, terminated, globally_reachable)
             VALUES ($1, $2::cidr, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
        )
        .bind(registry.updated())
        .bind(e.block.to_string())
        .bind(&e.name)
        .bind(&e.rfc)
        .bind(&e.terminated)
        .bind(flag(e.globally_reachable))
        .execute(&mut *tx)
        .await?
        .rows_affected();
    }
    tx.commit().await?;
    Ok(inserted)
}

/// An object whose stored public flag differs from the classification by `version`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicDrift {
    /// `prefix` or `address`.
    pub table: &'static str,
    /// Object.
    pub id: Uuid,
    /// Flag that should now be stored.
    pub public: bool,
}

/// Current prefixes and addresses whose `public` flag would change under `version`: the
/// report produced after a registry update (the flags themselves are rewritten by a
/// system changeset of the write pipeline).
///
/// # Errors
///
/// Database errors.
pub async fn public_drift(pool: &PgPool, version: &str) -> sqlx::Result<Vec<PublicDrift>> {
    let rows: Vec<(String, Uuid, bool)> = sqlx::query_as(
        "SELECT 'prefix', id, kartenn_is_public(prefix, $1) FROM prefix
          WHERE public IS DISTINCT FROM kartenn_is_public(prefix, $1)
         UNION ALL
         SELECT 'address', id, kartenn_is_public(address, $1) FROM address
          WHERE public IS DISTINCT FROM kartenn_is_public(address, $1)",
    )
    .bind(version)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(table, id, public)| PublicDrift {
            table: if table == "prefix" {
                "prefix"
            } else {
                "address"
            },
            id,
            public,
        })
        .collect())
}
