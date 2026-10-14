//! Audit search and pseudonymisation (REQ-004-27, 31 to 33).

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
/// fields are replaced, their internal identifier and the history stay untouched.
/// Returns the number of people pseudonymised.
///
/// # Errors
///
/// Database errors.
pub async fn pseudonymise(pool: &PgPool, delay_days: i32) -> sqlx::Result<u64> {
    let done = sqlx::query(
        "UPDATE person
         SET display_name = 'personne #' || left(id::text, 8), email = NULL, issuer = NULL, subject = NULL,
             pseudonymized_at = now()
         WHERE departed_at IS NOT NULL AND departed_at + make_interval(days => $1) <= now()
           AND pseudonymized_at IS NULL",
    )
    .bind(delay_days)
    .execute(pool)
    .await?;
    Ok(done.rows_affected())
}
