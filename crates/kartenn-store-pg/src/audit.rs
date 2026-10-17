//! Audit search and pseudonymisation (REQ-004-27, 31 to 33).

use kartenn_core::audit::PERSON_PSEUDONYMISED;
use kartenn_service::audit::{Instance, Outcome, RequestContext};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, QueryBuilder};

/// Criteria of an audit search; every criterion is optional.
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    /// Author.
    pub author: Option<Uuid>,
    /// State (`applied`, `refused`, `pending`…).
    pub state: Option<String>,
    /// Only retroactive changesets (REQ-004-27).
    pub retroactive: Option<bool>,
    /// Changesets that wrote this object (any version, current or history).
    pub object: Option<Uuid>,
    /// Submitted at or after.
    pub from: Option<OffsetDateTime>,
    /// Submitted before.
    pub to: Option<OffsetDateTime>,
}

/// One audit line.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AuditEntry {
    /// Changeset.
    pub id: Uuid,
    /// Author's display name (pseudonymised if the person left long enough ago).
    pub author: String,
    /// State.
    pub state: String,
    /// Motive.
    pub motive: String,
    /// Retroactive.
    pub retroactive: bool,
    /// Submission time.
    pub submitted_at: OffsetDateTime,
}

/// Versioned tables whose rows carry the changeset that wrote them.
const VERSIONED: [&str; 4] = ["addressing_domain", "prefix", "address", "entity"];

/// Changesets matching `filter`, most recent first, at most `limit` (REQ-004-32).
///
/// # Errors
///
/// Database errors.
pub async fn search(
    pool: &PgPool,
    filter: &AuditFilter,
    limit: i64,
) -> sqlx::Result<Vec<AuditEntry>> {
    let mut q = QueryBuilder::new(
        "SELECT c.id, p.display_name AS author, c.state, c.motive, c.retroactive, c.submitted_at
         FROM changeset c JOIN person p ON p.id = c.author_id WHERE true",
    );
    if let Some(a) = filter.author {
        q.push(" AND c.author_id = ").push_bind(a);
    }
    if let Some(s) = &filter.state {
        q.push(" AND c.state = ").push_bind(s.clone());
    }
    if let Some(r) = filter.retroactive {
        q.push(" AND c.retroactive = ").push_bind(r);
    }
    if let Some(from) = filter.from {
        q.push(" AND c.submitted_at >= ").push_bind(from);
    }
    if let Some(to) = filter.to {
        q.push(" AND c.submitted_at < ").push_bind(to);
    }
    if let Some(object) = filter.object {
        q.push(" AND c.id IN (");
        for (i, table) in VERSIONED.iter().enumerate() {
            if i > 0 {
                q.push(" UNION ");
            }
            // Table names are constants of this module, values are bound.
            q.push(format!("SELECT changeset_id FROM {table} WHERE id = "))
                .push_bind(object);
            q.push(format!(
                " UNION SELECT changeset_id FROM {table}_history WHERE id = "
            ))
            .push_bind(object);
        }
        q.push(")");
    }
    q.push(" ORDER BY c.submitted_at DESC, c.id DESC LIMIT ")
        .push_bind(limit);
    q.build_query_as().fetch_all(pool).await
}

/// Pseudonymises the people who left at least `delay_days` days ago (REQ-004-33): their identity
/// fields are replaced, their internal identifier and the history stay untouched; each one
/// is audited, by the system, in the same transaction (REQ-012-8). Returns the number of
/// people pseudonymised.
///
/// # Errors
///
/// Database errors.
pub async fn pseudonymise(
    pool: &PgPool,
    delay_days: i32,
    instance: &Instance,
) -> sqlx::Result<u64> {
    let mut tx = pool.begin().await?;
    let people: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE person
         SET display_name = 'personne #' || left(id::text, 8), email = NULL, issuer = NULL, subject = NULL,
             pseudonymized_at = now()
         WHERE departed_at IS NOT NULL AND departed_at + make_interval(days => $1) <= now()
           AND pseudonymized_at IS NULL
         RETURNING id",
    )
    .bind(delay_days)
    .fetch_all(&mut *tx)
    .await?;
    for person in &people {
        let event = RequestContext::system()
            .event(&PERSON_PSEUDONYMISED, Outcome::Success)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?
            .target("person", [*person]);
        crate::audit_journal::insert(&mut *tx, &event, instance)
            .await
            .map_err(|e| sqlx::Error::Protocol(e.0))?;
    }
    tx.commit().await?;
    Ok(people.len() as u64)
}

/// Row of `changeset_diff`: order, property key, object, value before, value after.
type DiffRow = (i64, String, Uuid, Option<String>, Option<String>);

/// One page of the differential detail of a changeset (REQ-003-19), in a stable order:
/// the entries after `after` (the `seq` of the last entry of the previous page), at most
/// `limit`, optionally for one property. Each entry comes with its `seq`.
///
/// # Errors
///
/// Database errors.
pub async fn diff_page(
    pool: &PgPool,
    changeset: Uuid,
    property: Option<&str>,
    after: Option<i64>,
    limit: i64,
) -> sqlx::Result<Vec<(i64, kartenn_service::port::DiffItem)>> {
    let rows: Vec<DiffRow> = sqlx::query_as(
        "SELECT seq, property_key, object_id, before::text, after::text FROM changeset_diff
         WHERE changeset_id = $1 AND ($2::text IS NULL OR property_key = $2)
           AND seq > coalesce($3, 0)
         ORDER BY seq LIMIT $4",
    )
    .bind(changeset)
    .bind(property)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(seq, property, object, before, after)| {
            (
                seq,
                kartenn_service::port::DiffItem {
                    property,
                    object,
                    before,
                    after,
                },
            )
        })
        .collect())
}
