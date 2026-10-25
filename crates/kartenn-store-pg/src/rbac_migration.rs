//! The report of the bindings of the former subjects on groups (spec 025, REQ-025-14, G9;
//! task 025-4): written by the migration `0068_rbac_bindings_reported`, listed once by
//! the migration job and audited (`rbac.migration.reported`).

use kartenn_core::audit::RBAC_MIGRATION_REPORTED;
use kartenn_service::StoreError;
use kartenn_service::audit::{Instance, Outcome, RequestContext};
use serde_json::json;
use sqlx::types::Uuid;
use sqlx::{FromRow, PgPool};

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// The former subject of the bindings a group received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormerSubject {
    /// An identity, by its identifier and name.
    Identity {
        /// Identifier.
        id: Uuid,
        /// Display name, if the identity is known.
        name: Option<String>,
    },
    /// A group of an issuer.
    IdpGroup {
        /// Issuer.
        issuer: String,
        /// Group, as the tokens name it.
        group: String,
    },
}

/// A group created by the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedGroup {
    /// Its key: `compte-…` or `idp-…`.
    pub key: String,
    /// Its only member, the former subject of its bindings.
    pub subject: FormerSubject,
    /// The bindings reported on it.
    pub bindings: i32,
}

/// A binding of a whole issuer, closed by the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedBinding {
    /// The binding.
    pub id: Uuid,
    /// The issuer.
    pub issuer: String,
    /// Its role.
    pub role: Option<String>,
    /// Its scope kind and object.
    pub scope: (String, Option<Uuid>),
}

/// What the report did, listed by the migration job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The changeset of the report.
    pub changeset: Uuid,
    /// The groups created, by key.
    pub groups: Vec<ReportedGroup>,
    /// The bindings of whole issuers closed.
    pub closed: Vec<ClosedBinding>,
}

impl Report {
    /// The lines printed by `kartenn-server migrate`.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "bindings reported on groups (spec 025), changeset {}:",
            self.changeset
        )];
        for g in &self.groups {
            let member = match &g.subject {
                FormerSubject::Identity { id, name } => format!(
                    "identity {id}{}",
                    name.as_deref()
                        .map(|n| format!(" ({n})"))
                        .unwrap_or_default()
                ),
                FormerSubject::IdpGroup { issuer, group } => {
                    format!("group {group} of {issuer}")
                }
            };
            lines.push(format!(
                "  group {}: member {member}, {} binding(s)",
                g.key, g.bindings
            ));
        }
        for c in &self.closed {
            let scope = match c.scope.1 {
                Some(id) => format!("{} {id}", c.scope.0),
                None => c.scope.0.clone(),
            };
            lines.push(format!(
                "  closed binding {} of the whole issuer {}: role {}, scope {scope}",
                c.id,
                c.issuer,
                c.role.as_deref().unwrap_or("?")
            ));
        }
        lines
    }
}

#[derive(FromRow)]
struct Row {
    changeset_id: Uuid,
    kind: String,
    group_key: Option<String>,
    subject_kind: String,
    identity_id: Option<Uuid>,
    name: Option<String>,
    issuer: Option<String>,
    group_name: Option<String>,
    bindings: Option<i32>,
    binding_id: Option<Uuid>,
    role_key: Option<String>,
    scope_kind: Option<String>,
    scope_id: Option<Uuid>,
}

/// Lists the report not listed yet, audits it as `rbac.migration.reported` by the system
/// and marks it listed, in one transaction; `None` when there is nothing (left) to list.
/// Called by the migration job, after the migrations.
///
/// # Errors
///
/// Database and audit failures.
pub async fn report(pool: &PgPool, instance: &Instance) -> Result<Option<Report>, StoreError> {
    let mut tx = pool.begin().await.map_err(store_err)?;
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT r.changeset_id, r.kind, r.group_key, r.subject_kind, r.identity_id,
                p.display_name AS name, r.issuer, r.group_name, r.bindings, r.binding_id,
                r.role_key, r.scope_kind, r.scope_id
         FROM rbac_migration_report r LEFT JOIN person p ON p.id = r.identity_id
         WHERE r.reported_at IS NULL ORDER BY r.seq FOR UPDATE OF r",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(store_err)?;
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let mut report = Report {
        changeset: first.changeset_id,
        groups: Vec::new(),
        closed: Vec::new(),
    };
    for row in rows {
        if row.kind == "group" {
            let subject = if row.subject_kind == "identity" {
                FormerSubject::Identity {
                    id: row.identity_id.unwrap_or_default(),
                    name: row.name,
                }
            } else {
                FormerSubject::IdpGroup {
                    issuer: row.issuer.unwrap_or_default(),
                    group: row.group_name.unwrap_or_default(),
                }
            };
            report.groups.push(ReportedGroup {
                key: row.group_key.unwrap_or_default(),
                subject,
                bindings: row.bindings.unwrap_or_default(),
            });
        } else {
            report.closed.push(ClosedBinding {
                id: row.binding_id.unwrap_or_default(),
                issuer: row.issuer.unwrap_or_default(),
                role: row.role_key,
                scope: (row.scope_kind.unwrap_or_default(), row.scope_id),
            });
        }
    }
    let groups: Vec<&str> = report.groups.iter().map(|g| g.key.as_str()).collect();
    let closed: Vec<Uuid> = report.closed.iter().map(|c| c.id).collect();
    let event = RequestContext::system()
        .event(&RBAC_MIGRATION_REPORTED, Outcome::Success)
        .map_err(store_err)?
        .changeset(report.changeset)
        .detail("groups", json!(groups))
        .map_err(store_err)?
        .detail("closed", json!(closed))
        .map_err(store_err)?;
    crate::audit_journal::insert(&mut *tx, &event, instance).await?;
    sqlx::query("UPDATE rbac_migration_report SET reported_at = now() WHERE reported_at IS NULL")
        .execute(&mut *tx)
        .await
        .map_err(store_err)?;
    tx.commit().await.map_err(store_err)?;
    Ok(Some(report))
}
