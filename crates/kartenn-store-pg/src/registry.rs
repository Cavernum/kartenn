//! Loading the IANA special-purpose registries and reporting public-flag drift
//! (REQ-002-34).

use kartenn_core::registry::{Flag, SpecialRegistry};
use kartenn_service::ChangesetRequest;
use kartenn_service::declarations::SYSTEM_AUTHOR;
use kartenn_service::port::{Journal, Store, StoreError, WriteTx};
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
    /// `prefix`, `ip_range` or `address`.
    pub table: &'static str,
    /// Object.
    pub id: Uuid,
    /// Its addressing domain.
    pub domain: Uuid,
    /// Flag that should now be stored.
    pub public: bool,
}

/// Prefixes, ranges and addresses valid now or later whose `public` flag would change under
/// `version`, once by object (a range is public when its first or last address is, as the pipeline classifies it): the
/// report produced after a registry update (the flags themselves are rewritten by a
/// system changeset of the write pipeline).
///
/// # Errors
///
/// Database errors.
pub async fn public_drift(pool: &PgPool, version: &str) -> sqlx::Result<Vec<PublicDrift>> {
    let rows: Vec<(String, Uuid, Uuid, bool)> = sqlx::query_as(
        "SELECT 'prefix', id, domain_id, kartenn_is_public(prefix, $1) FROM prefix
          WHERE valid_period && tstzrange(now(), NULL)
            AND public IS DISTINCT FROM kartenn_is_public(prefix, $1)
         UNION
         SELECT 'ip_range', id, domain_id, r.p FROM ip_range,
             LATERAL (SELECT kartenn_is_public(lower(range), $1) OR kartenn_is_public(upper(range), $1) AS p) r
          WHERE valid_period && tstzrange(now(), NULL) AND public IS DISTINCT FROM r.p
         UNION
         SELECT 'address', id, domain_id, kartenn_is_public(address, $1) FROM address
          WHERE valid_period && tstzrange(now(), NULL)
            AND public IS DISTINCT FROM kartenn_is_public(address, $1)",
    )
    .bind(version)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(table, id, domain, public)| PublicDrift {
            table: match table.as_str() {
                "prefix" => "prefix",
                "ip_range" => "ip_range",
                _ => "address",
            },
            id,
            domain,
            public,
        })
        .collect())
}

/// Rewrites the flags of [`public_drift`] under `version` from now on, by system changesets
/// of at most `batch` objects, each holding the locks of its domains and of the public
/// space as the write pipeline does (the task `public-drift`, REQ-006-38). Returns how
/// many objects were rewritten.
///
/// # Errors
///
/// Database errors.
pub async fn rewrite_public(
    store: &crate::write::PgStore,
    version: &str,
    batch: usize,
) -> Result<usize, StoreError> {
    let drift = public_drift(store.pool(), version)
        .await
        .map_err(|e| StoreError(e.to_string()))?;
    for chunk in drift.chunks(batch.max(1)) {
        let id = Uuid::now_v7();
        let mut tx = store.begin().await?;
        let mut domains: Vec<Uuid> = chunk.iter().map(|d| d.domain).collect();
        domains.sort_unstable();
        domains.dedup();
        tx.lock(&domains, true).await?;
        for d in chunk {
            let patch = serde_json::json!({ "public": d.public, "registry_version": version });
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "SELECT kartenn_portion('{}'::regclass, $1, now(), NULL::timestamptz, $2::jsonb, $3)",
                d.table
            )))
            .bind(d.id)
            .bind(patch.to_string())
            .bind(id)
            .execute(&mut *tx.tx)
            .await
            .map_err(|e| StoreError(e.to_string()))?;
        }
        let request = ChangesetRequest {
            author: SYSTEM_AUTHOR,
            motive: format!("public flags under the registries of {version}"),
            period: None,
            operations: Vec::new(),
            simulate: false,
            idempotency_key: None,
        };
        let objects: Vec<Uuid> = chunk.iter().map(|d| d.id).collect();
        tx.journal_applied(&Journal {
            id,
            request: &request,
            state: "applied",
            kind: "direct",
            validator: None,
            pending: None,
            violations: &[],
            domains: &domains,
            keys: &[],
            objects: &objects,
            reevaluated: &[],
            diff: &[],
            cascaded: &[],
            audit: &[],
        })
        .await?;
        tx.commit().await?;
    }
    Ok(drift.len())
}
