//! Pending changesets and approval policies in PostgreSQL (spec 004, REQ-004-19 to 24,
//! 39 to 43; tasks 004-11 to 14).

use kartenn_core::audit::{AuditEvent, CHANGESET_EXPIRED};
use kartenn_core::rules::{self, Violation};
use kartenn_service::ChangesetRequest;
use kartenn_service::audit::{Instance, Outcome, RequestContext};
use kartenn_service::port::{ApprovalPolicy, Pending, PendingChangeset, StoreError, WriteError};
use sqlx::types::Uuid;
use sqlx::{PgConnection, PgPool};

/// Days a changeset may stay pending (REQ-004-23); configurable with the deployment
/// settings (006).
pub(crate) const PENDING_DAYS: i32 = 30;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

type PendingRow = (
    Uuid,
    serde_json::Value,
    String,
    String,
    Option<serde_json::Value>,
);

fn decode(
    (id, request, kind, state, required): PendingRow,
) -> Result<PendingChangeset, StoreError> {
    Ok(PendingChangeset {
        id,
        request: serde_json::from_value::<ChangesetRequest>(request).map_err(store_err)?,
        kind,
        state,
        pending: required
            .map(serde_json::from_value::<Pending>)
            .transpose()
            .map_err(store_err)?
            .unwrap_or(Pending {
                policy: None,
                perimeter: Vec::new(),
                required: Vec::new(),
                author: None,
                on_behalf_of: None,
                review: false,
                reason: None,
            }),
    })
}

const COLUMNS: &str = "id, request, kind, state, required_rights";

/// A changeset journaled as pending.
pub(crate) async fn find(pool: &PgPool, id: Uuid) -> Result<Option<PendingChangeset>, StoreError> {
    let row: Option<PendingRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM changeset WHERE id = $1 AND request IS NOT NULL"
    )))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(store_err)?;
    row.map(decode).transpose()
}

/// Pending proposals, oldest first.
pub(crate) async fn proposals(pool: &PgPool) -> Result<Vec<PendingChangeset>, StoreError> {
    let rows: Vec<PendingRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM changeset WHERE state = 'pending' AND kind = 'proposal'
         ORDER BY submitted_at, id"
    )))
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    rows.into_iter().map(decode).collect()
}

/// The event that tells the author what became of a changeset (REQ-004-43).
async fn event(
    conn: &mut PgConnection,
    id: Uuid,
    state: &str,
    author: Uuid,
    by: Option<Uuid>,
    rejection: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO outbox (changeset_id, payload) VALUES ($1, jsonb_build_object(
            'specversion', '1.0', 'id', $1::text || '/' || $2, 'source', $3::text,
            'type', 'org.kartenn.changeset.' || $2, 'time', now(),
            'datacontenttype', 'application/json',
            'data', jsonb_strip_nulls(jsonb_build_object(
                'changeset', $1, 'author', $4, 'by', $5, 'rejection', $6))))",
    )
    .bind(id)
    .bind(state)
    .bind(crate::write::EVENT_SOURCE)
    .bind(author)
    .bind(by)
    .bind(rejection)
    .execute(&mut *conn)
    .await
    .map_err(store_err)?;
    crate::outbox::notify(conn).await
}

/// Ends a pending changeset (`rejected` by `by`, or `withdrawn`), with its event, in the
/// transaction of `conn`.
pub(crate) async fn close(
    conn: &mut PgConnection,
    id: Uuid,
    state: &str,
    by: Uuid,
    rejection: Option<&str>,
    audit: &AuditEvent,
    instance: &Instance,
) -> Result<bool, StoreError> {
    let author: Option<Uuid> = sqlx::query_scalar(
        "UPDATE changeset SET state = $2,
                validator_id = CASE WHEN $2 = 'rejected' THEN $3 END, rejection = $4
         WHERE id = $1 AND state = 'pending' RETURNING author_id",
    )
    .bind(id)
    .bind(state)
    .bind(by)
    .bind(rejection)
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_err)?;
    let Some(author) = author else {
        return Ok(false);
    };
    event(conn, id, state, author, Some(by), rejection).await?;
    crate::audit_journal::insert(&mut *conn, audit, instance).await?;
    Ok(true)
}

/// Expires the changesets pending past their deadline, with their events, in the
/// transaction of `conn`.
pub(crate) async fn expire(
    conn: &mut PgConnection,
    instance: &Instance,
) -> Result<Vec<PendingChangeset>, StoreError> {
    type Expired = (
        Uuid,
        serde_json::Value,
        String,
        String,
        Option<serde_json::Value>,
        Uuid,
    );
    let rows: Vec<Expired> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE changeset SET state = 'expired'
         WHERE state = 'pending' AND expires_at <= now() RETURNING {COLUMNS}, author_id"
    )))
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    let mut expired = Vec::with_capacity(rows.len());
    for (id, request, kind, state, required, author) in rows {
        event(conn, id, "expired", author, None, None).await?;
        let audit = RequestContext::system()
            .event(&CHANGESET_EXPIRED, Outcome::Success)
            .map_err(store_err)?
            .target("changeset", [id])
            .changeset(id);
        crate::audit_journal::insert(&mut *conn, &audit, instance).await?;
        expired.push(decode((id, request, kind, state, required))?);
    }
    Ok(expired)
}

/// The event of a changeset journaled in a state other than `applied` and `refused`.
pub(crate) async fn journal_event(
    conn: &mut PgConnection,
    id: Uuid,
    state: &str,
    author: Uuid,
    validator: Option<Uuid>,
) -> Result<(), StoreError> {
    if matches!(state, "pending" | "failed_revalidation") {
        event(conn, id, state, author, validator, None).await?;
    }
    Ok(())
}

/// Locks a journaled changeset and gives its state.
pub(crate) async fn lock(conn: &mut PgConnection, id: Uuid) -> Result<Option<String>, StoreError> {
    sqlx::query_scalar("SELECT state FROM changeset WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(store_err)
}

type PolicyRow = (
    Uuid,
    String,
    Option<i32>,
    Option<i32>,
    Vec<String>,
    serde_json::Value,
    serde_json::Value,
    bool,
    serde_json::Value,
);

/// Identifiers under `key` in a list of anchors such as `[{"domain": "…"}]`.
fn anchors(list: &serde_json::Value, key: &str) -> Vec<Uuid> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a.get(key)?.as_str()?.parse().ok())
        .collect()
}

/// The texts under `key` in a JSON array of objects.
fn texts(array: &serde_json::Value, key: &str) -> Vec<String> {
    array
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| Some(a.get(key)?.as_str()?.to_owned()))
        .collect()
}

/// Enabled approval policies, by name.
pub(crate) async fn policies(conn: &mut PgConnection) -> Result<Vec<ApprovalPolicy>, StoreError> {
    let rows: Vec<PolicyRow> = sqlx::query_as(
        "SELECT id, name, min_operations, min_diff_objects, property_keys, perimeters, authors,
                enabled, module_triggers
         FROM approval_policy WHERE enabled ORDER BY name",
    )
    .fetch_all(conn)
    .await
    .map_err(store_err)?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                name,
                min_operations,
                min_diff_objects,
                property_keys,
                perimeters,
                authors,
                enabled,
                module_triggers,
            )| {
                ApprovalPolicy {
                    id,
                    name,
                    min_operations: min_operations.and_then(|m| u32::try_from(m).ok()),
                    min_diff_objects: min_diff_objects.and_then(|m| u64::try_from(m).ok()),
                    property_keys,
                    domains: anchors(&perimeters, "domain"),
                    scopes: ["entity", "site", "prefix"]
                        .iter()
                        .flat_map(|kind| {
                            anchors(&perimeters, kind)
                                .into_iter()
                                .map(move |id| ((*kind).to_owned(), id))
                        })
                        .collect(),
                    author_roles: texts(&authors, "role"),
                    bound_roles: texts(&perimeters, "bound_role"),
                    author_groups: texts(&authors, "group"),
                    module_triggers: module_triggers
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|t| Some((t.get(0)?.as_str()?.to_owned(), t.get(1)?.as_u64()?)))
                        .collect(),
                    enabled,
                }
            },
        )
        .collect())
}

/// Identifier of the policy named `name`.
pub(crate) async fn policy_named(
    conn: &mut PgConnection,
    name: &str,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar("SELECT id FROM approval_policy WHERE name = $1")
        .bind(name)
        .fetch_optional(conn)
        .await
        .map_err(store_err)
}

/// Creates or replaces a policy.
pub(crate) async fn set_policy(
    conn: &mut PgConnection,
    policy: &ApprovalPolicy,
) -> Result<(), WriteError> {
    let perimeters: Vec<serde_json::Value> = policy
        .domains
        .iter()
        .map(|d| serde_json::json!({ "domain": d }))
        .chain(
            policy
                .scopes
                .iter()
                .map(|(kind, id)| serde_json::json!({ kind.as_str(): id })),
        )
        // What the changeset binds belongs with what it touches (005).
        .chain(
            policy
                .bound_roles
                .iter()
                .map(|r| serde_json::json!({ "bound_role": r })),
        )
        .collect();
    // REQ-025-13: an author is a role or a group of kartenn in force, as the changeset
    // leaves them; an account is neither.
    let (roles, groups): (Vec<String>, Vec<String>) = sqlx::query_as(
        "SELECT
             ARRAY(SELECT a FROM unnest($1::text[]) a
                   WHERE NOT EXISTS (SELECT 1 FROM role r
                                     WHERE r.key = a AND r.valid_period @> now())),
             ARRAY(SELECT a FROM unnest($2::text[]) a
                   WHERE NOT EXISTS (SELECT 1 FROM rbac_group g
                                     WHERE g.key = a AND g.valid_period @> now()))",
    )
    .bind(&policy.author_roles)
    .bind(&policy.author_groups)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| WriteError::Store(store_err(e)))?;
    if !roles.is_empty() || !groups.is_empty() {
        let unknown = roles
            .iter()
            .map(|r| format!("role '{r}'"))
            .chain(groups.iter().map(|g| format!("group '{g}'")))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(WriteError::Rule(
            Violation::new(
                &rules::POLICY_AUTHOR,
                format!(
                    "the policy '{}': no {unknown} in force; an author is a role or a group of kartenn, not an account",
                    policy.name
                ),
            )
            .with_subjects([policy.id]),
        ));
    }
    let authors: Vec<serde_json::Value> = policy
        .author_roles
        .iter()
        .map(|r| serde_json::json!({ "role": r }))
        .chain(
            policy
                .author_groups
                .iter()
                .map(|g| serde_json::json!({ "group": g })),
        )
        .collect();
    sqlx::query(
        "INSERT INTO approval_policy
             (id, name, min_operations, min_diff_objects, property_keys, perimeters, authors, enabled,
              module_triggers)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name,
             min_operations = EXCLUDED.min_operations,
             min_diff_objects = EXCLUDED.min_diff_objects,
             property_keys = EXCLUDED.property_keys, perimeters = EXCLUDED.perimeters,
             authors = EXCLUDED.authors, enabled = EXCLUDED.enabled,
             module_triggers = EXCLUDED.module_triggers",
    )
    .bind(policy.id)
    .bind(&policy.name)
    .bind(policy.min_operations.and_then(|m| i32::try_from(m).ok()))
    .bind(policy.min_diff_objects.and_then(|m| i32::try_from(m).ok()))
    .bind(&policy.property_keys)
    .bind(serde_json::Value::Array(perimeters))
    .bind(serde_json::Value::Array(authors))
    .bind(policy.enabled)
    .bind(serde_json::json!(policy.module_triggers))
    .execute(conn)
    .await
    .map_err(|e| WriteError::Store(store_err(e)))?;
    Ok(())
}

/// Installs the default approval policies of the modules of the build that were never
/// installed (REQ-016-8): marked of module origin, enabled, recorded so that a policy an
/// administrator disables, renames or removes never comes back. Returns their names.
///
/// # Errors
///
/// Database errors.
pub async fn install_default_policies(
    pool: &sqlx::PgPool,
    modules: &kartenn_service::module::ModuleRegistry,
) -> Result<Vec<String>, StoreError> {
    let mut installed = Vec::new();
    let mut tx = pool.begin().await.map_err(store_err)?;
    for (module, policy) in modules.default_policies() {
        let fresh: Option<String> = sqlx::query_scalar(
            "INSERT INTO module_default_policy (module, name) VALUES ($1, $2)
             ON CONFLICT DO NOTHING RETURNING name",
        )
        .bind(module)
        .bind(policy.name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_err)?;
        if fresh.is_none() {
            continue;
        }
        let triggers: Vec<(String, u64)> = policy
            .triggers
            .iter()
            .map(|(measure, threshold)| (format!("{module}.{measure}"), *threshold))
            .collect();
        sqlx::query(
            "INSERT INTO approval_policy (id, name, module_triggers, origin, enabled)
             VALUES ($1, $2, $3, 'module', true) ON CONFLICT (name) DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(policy.name)
        .bind(serde_json::json!(triggers))
        .execute(&mut *tx)
        .await
        .map_err(store_err)?;
        installed.push(policy.name.to_owned());
    }
    tx.commit().await.map_err(store_err)?;
    Ok(installed)
}
