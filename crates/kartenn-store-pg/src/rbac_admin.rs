//! Writes of the administration of the RBAC (spec 005, REQ-005-3, 25; task 005-7): roles,
//! their entries, bindings and attachments are versioned rows whose application period
//! ends when they are removed; the state of an identity is a column of `person`.

use kartenn_core::authz::{BUILTIN_GROUPS, BUILTIN_ROLES};
use kartenn_core::rules::{self, Violation};
use kartenn_service::operations::GroupMember;
use kartenn_service::port::{StoreError, WriteError};
use kartenn_service::rbac::{RBAC_OBJECT, RbacOp};
use sqlx::PgConnection;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

fn write_err(e: sqlx::Error) -> WriteError {
    WriteError::Store(store_err(e))
}

/// Ends the rows of `table` matching `condition` (on `$1`, `$2`) at `$3` (now if null):
/// shortens those valid then, removes those starting later; the number of rows touched.
async fn end_rows(
    conn: &mut PgConnection,
    table: &str,
    condition: &str,
    first: Option<Uuid>,
    second: Option<&str>,
    at: Option<OffsetDateTime>,
    changeset: Uuid,
) -> Result<u64, WriteError> {
    let shortened = sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {table} SET valid_period = tstzrange(lower(valid_period), coalesce($3::timestamptz, now())),
                            changeset_id = $4
         WHERE {condition} AND valid_period @> coalesce($3::timestamptz, now())
           AND lower(valid_period) < coalesce($3::timestamptz, now())"
    )))
    .bind(first)
    .bind(second)
    .bind(at)
    .bind(changeset)
    .execute(&mut *conn)
    .await
    .map_err(write_err)?
    .rows_affected();
    let removed = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM {table} WHERE {condition} AND lower(valid_period) >= coalesce($3::timestamptz, now())"
    )))
    .bind(first)
    .bind(second)
    .bind(at)
    .execute(&mut *conn)
    .await
    .map_err(write_err)?
    .rows_affected();
    Ok(shortened + removed)
}

/// The role `key` in force at `at` (now if null).
async fn role_id(
    conn: &mut PgConnection,
    key: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT id FROM role WHERE key = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(key)
    .bind(at)
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_err)
}

/// See [`kartenn_service::WriteTx::role_entries`].
pub(crate) async fn role_entries(
    conn: &mut PgConnection,
    key: &str,
) -> Result<Option<Vec<String>>, StoreError> {
    let Some(role) = role_id(conn, key, None).await? else {
        return Ok(None);
    };
    let entries = sqlx::query_scalar(
        "SELECT permission FROM role_permission WHERE role_id = $1 AND valid_period @> now()
         ORDER BY permission",
    )
    .bind(role)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    Ok(Some(entries))
}

/// See [`kartenn_service::WriteTx::custom_roles_citing`].
pub(crate) async fn custom_roles_citing(
    conn: &mut PgConnection,
    permissions: &[&str],
) -> Result<Vec<kartenn_service::rbac::StoredRole>, StoreError> {
    let rows: Vec<(
        String,
        sqlx::types::Json<kartenn_service::operations::RawLabels>,
        Vec<String>,
    )> = sqlx::query_as(
        "SELECT r.key, r.labels, array_agg(p.permission ORDER BY p.permission)
         FROM role r
         JOIN role_permission p ON p.role_id = r.id AND p.valid_period @> now()
         WHERE r.valid_period @> now() AND NOT r.builtin
         GROUP BY r.key, r.labels
         HAVING bool_or(p.permission = ANY($1) OR p.permission ~ '^prop\\..+\\.(write|propose)$')
         ORDER BY r.key",
    )
    .bind(permissions)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    Ok(rows
        .into_iter()
        .map(|(key, labels, entries)| kartenn_service::rbac::StoredRole {
            key,
            labels: labels.0,
            entries,
        })
        .collect())
}

/// The key of the role `role_id`, from any of its versions (an SQL expression).
const ROLE_KEY: &str = "(SELECT key FROM role WHERE id = b.role_id
       UNION ALL SELECT key FROM role_history WHERE id = b.role_id LIMIT 1)";

/// A row of a stored binding: id, scope, group key, builtin group, role key.
type BindingRow = (
    Uuid,
    Option<Uuid>,
    Option<String>,
    Option<bool>,
    Option<String>,
);

fn stored(row: BindingRow) -> kartenn_service::port::StoredBinding {
    let (id, scope, group, builtin, role) = row;
    kartenn_service::port::StoredBinding {
        id,
        group: group.unwrap_or_default(),
        builtin: builtin.unwrap_or(false),
        role: role.unwrap_or_default(),
        scope: scope.unwrap_or(RBAC_OBJECT),
    }
}

/// See [`kartenn_service::WriteTx::stored_binding`], read as asserted before the writes of
/// the transaction (`binding_asserted`, migration 0066). A binding written before the
/// groups (task 025-4) names none: its scope is asked all the same.
pub(crate) async fn stored_binding(
    conn: &mut PgConnection,
    binding: Uuid,
) -> Result<Option<kartenn_service::port::StoredBinding>, StoreError> {
    let row: Option<BindingRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT b.id, b.scope_id, g.key, g.builtin, {ROLE_KEY}
         FROM binding_asserted b
         LEFT JOIN LATERAL (SELECT key, builtin FROM rbac_group WHERE id = b.group_id
                            UNION ALL
                            SELECT key, builtin FROM rbac_group_history WHERE id = b.group_id
                            LIMIT 1) g ON true
         WHERE b.id = $1 AND b.valid_period @> now()
         LIMIT 1"
    )))
    .bind(binding)
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_err)?;
    Ok(row.map(stored))
}

/// See [`kartenn_service::WriteTx::stored_group`], as asserted before the writes of the
/// transaction.
pub(crate) async fn stored_group(
    conn: &mut PgConnection,
    key: &str,
) -> Result<Option<kartenn_service::port::StoredGroup>, StoreError> {
    let group: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT id, builtin FROM rbac_group_asserted
         WHERE key = $1 AND valid_period @> now() LIMIT 1",
    )
    .bind(key)
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_err)?;
    let Some((id, builtin)) = group else {
        return Ok(None);
    };
    let rows: Vec<BindingRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT b.id, b.scope_id, $2::text, $3::boolean, {ROLE_KEY}
         FROM binding_asserted b WHERE b.group_id = $1 AND b.valid_period @> now()
         ORDER BY b.id"
    )))
    .bind(id)
    .bind(key)
    .bind(builtin)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)?;
    Ok(Some(kartenn_service::port::StoredGroup {
        builtin,
        bindings: rows.into_iter().map(stored).collect(),
    }))
}

/// See [`kartenn_service::WriteTx::member_group`].
pub(crate) async fn member_group(
    conn: &mut PgConnection,
    member: Uuid,
) -> Result<Option<String>, StoreError> {
    let key: Option<Option<String>> = sqlx::query_scalar(
        "SELECT (SELECT key FROM rbac_group WHERE id = m.group_id
                 UNION ALL SELECT key FROM rbac_group_history WHERE id = m.group_id LIMIT 1)
         FROM (SELECT group_id FROM group_member WHERE id = $1 AND sys_start < now()
               UNION ALL
               SELECT group_id FROM group_member_history WHERE id = $1 AND sys_start < now()) m
         LIMIT 1",
    )
    .bind(member)
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_err)?;
    Ok(key.flatten())
}

/// See [`kartenn_service::WriteTx::scopes_holding`].
pub(crate) async fn scopes_holding(
    conn: &mut PgConnection,
    subject: &kartenn_service::authz::Subject,
    permission: &str,
) -> Result<Vec<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT DISTINCT coalesce(b.scope_id, $5) FROM binding_asserted b
         JOIN role_asserted r ON r.id = b.role_id AND r.valid_period @> now()
         JOIN role_permission_asserted rp ON rp.role_id = r.id AND rp.valid_period @> now()
                                AND rp.permission IN ($4, '*')
         WHERE b.valid_period @> now()
           AND b.group_id IN (SELECT kartenn_member_of($1, $2, $3, now()))",
    )
    .bind(subject.id)
    .bind(subject.issuer.as_deref())
    .bind(&subject.groups)
    .bind(permission)
    .bind(RBAC_OBJECT)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_err)
}

/// The period of a new row: from `$1` (transaction time if null) to `$2`.
const PERIOD: &str = "tstzrange(coalesce($1::timestamptz, now()), $2::timestamptz)";

/// Defines the role `key`, or replaces its labels and entries.
async fn set_role(
    conn: &mut PgConnection,
    (key, labels, entries): (&str, &kartenn_core::labels::Labels, &[String]),
    at: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let labels = labels.to_json();
    let role = if let Some(role) = role_id(conn, key, at).await? {
        sqlx::query(
            "UPDATE role SET labels = $2, changeset_id = $3 WHERE id = $1 AND labels <> $2",
        )
        .bind(role)
        .bind(&labels)
        .bind(changeset)
        .execute(&mut *conn)
        .await
        .map_err(write_err)?;
        role
    } else {
        let role = Uuid::now_v7();
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO role (id, key, labels, builtin, valid_period, changeset_id)
             VALUES ($3, $4, $5, $6, {PERIOD}, $7)"
        )))
        .bind(at)
        .bind(until)
        .bind(role)
        .bind(key)
        .bind(&labels)
        .bind(BUILTIN_ROLES.iter().any(|r| r.key == key))
        .bind(changeset)
        .execute(&mut *conn)
        .await
        .map_err(write_err)?;
        role
    };
    let current: Vec<String> = sqlx::query_scalar(
        "SELECT permission FROM role_permission
         WHERE role_id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(role)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(write_err)?;
    for gone in current.iter().filter(|c| !entries.contains(c)) {
        end_rows(
            conn,
            "role_permission",
            "role_id = $1 AND permission = $2",
            Some(role),
            Some(gone),
            at,
            changeset,
        )
        .await?;
    }
    for added in entries.iter().filter(|e| !current.contains(e)) {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($3, $4, {PERIOD}, $5)"
        )))
        .bind(at)
        .bind(until)
        .bind(role)
        .bind(added)
        .bind(changeset)
        .execute(&mut *conn)
        .await
        .map_err(write_err)?;
    }
    Ok(())
}

/// The group `key` in force at `at` (now if null).
async fn group_id(
    conn: &mut PgConnection,
    key: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT id FROM rbac_group
         WHERE key = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(key)
    .bind(at)
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_err)
}

/// A new binding of `role` to the group `group` (spec 025, REQ-025-4).
#[allow(clippy::too_many_arguments)] // a binding, its period and its changeset
async fn bind(
    conn: &mut PgConnection,
    id: Uuid,
    group: &str,
    role: &str,
    (scope_kind, scope): (&str, Option<Uuid>),
    at: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let Some(role_id) = role_id(conn, role, at).await? else {
        return Err(WriteError::Rule(
            Violation::new(&rules::RBAC_UNKNOWN_ROLE, format!("no role '{role}'"))
                .with_subjects([id]),
        ));
    };
    // A group closed since a pending changeset named it: a conflict at its approval.
    let Some(group_id) = group_id(conn, group, at).await? else {
        return Err(WriteError::Rule(
            Violation::new(&rules::GROUP_CLOSED, format!("no group '{group}'")).with_subjects([id]),
        ));
    };
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO binding (id, group_id, role_id, scope_kind, scope_id, valid_period,
                              changeset_id)
         VALUES ($3, $4, $5, $6, $7, {PERIOD}, $8)"
    )))
    .bind(at)
    .bind(until)
    .bind(id)
    .bind(group_id)
    .bind(role_id)
    .bind(scope_kind)
    .bind(scope)
    .bind(changeset)
    .execute(&mut *conn)
    .await
    .map_err(write_err)?;
    Ok(())
}

/// Defines the group `key`, or replaces its labels (spec 025, REQ-025-1).
async fn set_group(
    conn: &mut PgConnection,
    key: &str,
    labels: &kartenn_core::labels::Labels,
    (at, until): (Option<OffsetDateTime>, Option<OffsetDateTime>),
    changeset: Uuid,
) -> Result<(), WriteError> {
    let labels = labels.to_json();
    if let Some(group) = group_id(conn, key, at).await? {
        sqlx::query(
            "UPDATE rbac_group SET labels = $2, changeset_id = $3 WHERE id = $1 AND labels <> $2",
        )
        .bind(group)
        .bind(&labels)
        .bind(changeset)
        .execute(&mut *conn)
        .await
        .map_err(write_err)?;
        return Ok(());
    }
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO rbac_group (id, key, labels, builtin, valid_period, changeset_id)
         VALUES ($3, $4, $5, $6, {PERIOD}, $7)"
    )))
    .bind(at)
    .bind(until)
    .bind(Uuid::now_v7())
    .bind(key)
    .bind(&labels)
    .bind(BUILTIN_GROUPS.iter().any(|g| g.key == key))
    .bind(changeset)
    .execute(&mut *conn)
    .await
    .map_err(write_err)?;
    Ok(())
}

/// A new membership `id` of `member` in the group `group` (spec 025, REQ-025-2): a
/// requester is not an account (016/G10, INV-60); a member appears once in a group.
async fn add_member(
    conn: &mut PgConnection,
    (id, group, member): (Uuid, &str, &GroupMember),
    (at, until): (Option<OffsetDateTime>, Option<OffsetDateTime>),
    changeset: Uuid,
) -> Result<(), WriteError> {
    let Some(group_id) = group_id(conn, group, at).await? else {
        return Err(WriteError::Rule(
            Violation::new(&rules::GROUP_UNKNOWN, format!("no group '{group}'"))
                .with_subjects([id]),
        ));
    };
    let (kind, identity, issuer, name) = match member {
        GroupMember::Identity { id } => ("identity", Some(*id), None, None),
        GroupMember::IdpGroup { issuer, group } => (
            "idp_group",
            None,
            Some(issuer.as_str()),
            Some(group.as_str()),
        ),
    };
    if let Some(identity) = identity {
        let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM person WHERE id = $1")
            .bind(identity)
            .fetch_optional(&mut *conn)
            .await
            .map_err(write_err)?;
        if kind.as_deref() == Some("requester") {
            return Err(WriteError::Rule(
                Violation::new(
                    &rules::RBAC_REQUESTER,
                    format!("identity {identity} is a requester, not an account"),
                )
                .with_subjects([id]),
            ));
        }
    }
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO group_member (id, group_id, member_kind, identity_id, issuer, group_name,
                                   valid_period, changeset_id)
         VALUES ($3, $4, $5, $6, $7, $8, {PERIOD}, $9)"
    )))
    .bind(at)
    .bind(until)
    .bind(id)
    .bind(group_id)
    .bind(kind)
    .bind(identity)
    .bind(issuer)
    .bind(name)
    .bind(changeset)
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        if e.as_database_error()
            .and_then(|d| d.constraint())
            .is_some_and(|c| c == "group_member_unique")
        {
            WriteError::Rule(
                Violation::new(
                    &rules::GROUP_INVALID,
                    format!("this member is already in the group '{group}' then"),
                )
                .with_subjects([id]),
            )
        } else {
            write_err(e)
        }
    })?;
    Ok(())
}

/// Writes an operation on a group or its members (spec 025): closing a group ends its
/// bindings and its memberships with it (REQ-025-8).
async fn write_group(
    conn: &mut PgConnection,
    op: &RbacOp,
    (at, until): (Option<OffsetDateTime>, Option<OffsetDateTime>),
    changeset: Uuid,
) -> Result<(), WriteError> {
    match op {
        RbacOp::SetGroup { key, labels } => {
            set_group(conn, key, labels, (at, until), changeset).await
        }
        RbacOp::CloseGroup { key } => {
            let Some(group) = group_id(conn, key, at).await? else {
                return Err(WriteError::NoVersion(RBAC_OBJECT));
            };
            for (table, condition) in [
                ("binding", "group_id = $1 AND $2::text IS NULL"),
                ("group_member", "group_id = $1 AND $2::text IS NULL"),
                ("rbac_group", "id = $1 AND $2::text IS NULL"),
            ] {
                end_rows(conn, table, condition, Some(group), None, at, changeset).await?;
            }
            Ok(())
        }
        RbacOp::AddMember { id, group, member } => {
            add_member(conn, (*id, group, member), (at, until), changeset).await
        }
        RbacOp::RemoveMember { id } => {
            let condition = "id = $1 AND $2::text IS NULL";
            let ended = end_rows(
                conn,
                "group_member",
                condition,
                Some(*id),
                None,
                at,
                changeset,
            );
            if ended.await? == 0 {
                return Err(WriteError::NoVersion(*id));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Disables or enables an identity, revokes its sessions.
async fn identity_state(
    conn: &mut PgConnection,
    identity: Uuid,
    disabled: Option<bool>,
    revoke_sessions: bool,
) -> Result<(), WriteError> {
    let updated = sqlx::query(
        "UPDATE person
         SET disabled_at = CASE WHEN $2::boolean IS NULL THEN disabled_at
                                WHEN $2 THEN coalesce(disabled_at, now())
                                ELSE NULL END,
             revoked_after = CASE WHEN $3 THEN now() ELSE revoked_after END
         WHERE id = $1 AND kind IN ('person', 'service_account')",
    )
    .bind(identity)
    .bind(disabled)
    .bind(revoke_sessions)
    .execute(&mut *conn)
    .await
    .map_err(write_err)?
    .rows_affected();
    // A break-glass account lives and dies by the owner of the schema only (REQ-005-14),
    // and kartenn itself is not an identity to disable: neither is found here.
    if updated == 0 {
        return Err(WriteError::NoVersion(identity));
    }
    Ok(())
}

/// A secret of `account`; a third active one is refused (rotation keeps two).
async fn add_secret(
    conn: &mut PgConnection,
    account: Uuid,
    secret: &kartenn_service::operations::SecretRecord,
) -> Result<(), WriteError> {
    let digest: Vec<u8> = (0..secret.digest.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&secret.digest[i..i + 2], 16).ok())
        .collect();
    let expires = secret
        .expires_at
        .map(|m| OffsetDateTime::from_unix_timestamp_nanos(i128::from(m) * 1_000))
        .transpose()
        .map_err(|e| WriteError::Store(store_err(e)))?;
    sqlx::query(
        "INSERT INTO service_secret (id, account_id, digest, key_id, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(secret.id)
    .bind(account)
    .bind(digest)
    .bind(&secret.key_id)
    .bind(expires)
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        if e.as_database_error()
            .is_some_and(|d| d.message().contains("two active secrets"))
        {
            WriteError::Rule(
                Violation::new(
                    &rules::RBAC_INVALID,
                    "a service account holds two active secrets at most: revoke one first",
                )
                .with_subjects([account]),
            )
        } else {
            write_err(e)
        }
    })?;
    Ok(())
}

/// Writes an operation on a service account.
async fn account(conn: &mut PgConnection, op: &RbacOp) -> Result<(), WriteError> {
    match op {
        RbacOp::CreateAccount { id, name, secret } => {
            sqlx::query(
                "INSERT INTO person (id, display_name, kind) VALUES ($1, $2, 'service_account')",
            )
            .bind(id)
            .bind(name)
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                if e.as_database_error()
                    .and_then(|d| d.constraint())
                    .is_some_and(|c| c == "person_service_account_name")
                {
                    WriteError::Rule(
                        Violation::new(
                            &rules::RBAC_INVALID,
                            format!("a service account is already named '{name}'"),
                        )
                        .with_subjects([*id]),
                    )
                } else {
                    write_err(e)
                }
            })?;
            add_secret(conn, *id, secret).await
        }
        RbacOp::AddSecret { account, secret } => {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM person WHERE id = $1 AND kind = 'service_account')",
            )
            .bind(account)
            .fetch_one(&mut *conn)
            .await
            .map_err(write_err)?;
            if !exists {
                return Err(WriteError::NoVersion(*account));
            }
            add_secret(conn, *account, secret).await
        }
        RbacOp::RevokeSecret { account, secret } => {
            let revoked = sqlx::query(
                "UPDATE service_secret SET revoked_at = now()
                 WHERE id = $1 AND account_id = $2 AND revoked_at IS NULL",
            )
            .bind(secret)
            .bind(account)
            .execute(&mut *conn)
            .await
            .map_err(write_err)?
            .rows_affected();
            if revoked == 0 {
                return Err(WriteError::NoVersion(*secret));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Writes `op` over a period starting at `at` (transaction time if null) and ending at
/// `until` (unbounded if null).
pub(crate) async fn write(
    conn: &mut PgConnection,
    op: &RbacOp,
    at: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    match op {
        RbacOp::SetRole {
            key,
            labels,
            entries,
        } => set_role(conn, (key, labels, entries), at, until, changeset).await,
        RbacOp::CloseRole { key } => {
            let Some(role) = role_id(conn, key, at).await? else {
                return Err(WriteError::NoVersion(RBAC_OBJECT));
            };
            for (table, condition) in [
                ("binding", "role_id = $1 AND $2::text IS NULL"),
                ("role_permission", "role_id = $1 AND $2::text IS NULL"),
                ("role", "id = $1 AND $2::text IS NULL"),
            ] {
                end_rows(conn, table, condition, Some(role), None, at, changeset).await?;
            }
            Ok(())
        }
        RbacOp::SetGroup { .. }
        | RbacOp::CloseGroup { .. }
        | RbacOp::AddMember { .. }
        | RbacOp::RemoveMember { .. } => write_group(conn, op, (at, until), changeset).await,
        RbacOp::Bind {
            id,
            group,
            role,
            scope_kind,
            scope,
        } => {
            bind(
                conn,
                *id,
                group,
                role,
                (scope_kind, *scope),
                at,
                until,
                changeset,
            )
            .await
        }
        RbacOp::Unbind { id } => {
            let condition = "id = $1 AND $2::text IS NULL";
            if end_rows(conn, "binding", condition, Some(*id), None, at, changeset).await? == 0 {
                return Err(WriteError::NoVersion(*id));
            }
            Ok(())
        }
        RbacOp::Attach { issuer, entity } => {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO issuer_attachment (issuer, entity_id, valid_period, changeset_id)
                 VALUES ($3, $4, {PERIOD}, $5)"
            )))
            .bind(at)
            .bind(until)
            .bind(issuer)
            .bind(entity)
            .bind(changeset)
            .execute(&mut *conn)
            .await
            .map_err(write_err)?;
            Ok(())
        }
        RbacOp::Detach { issuer, entity } => {
            let condition = "entity_id IS NOT DISTINCT FROM $1 AND issuer = $2";
            let ended = end_rows(
                conn,
                "issuer_attachment",
                condition,
                *entity,
                Some(issuer),
                at,
                changeset,
            )
            .await?;
            if ended == 0 {
                return Err(WriteError::NoVersion(entity.unwrap_or(RBAC_OBJECT)));
            }
            Ok(())
        }
        RbacOp::IdentityState {
            identity,
            disabled,
            revoke_sessions,
        } => identity_state(conn, *identity, *disabled, *revoke_sessions).await,
        RbacOp::CreateAccount { .. } | RbacOp::AddSecret { .. } | RbacOp::RevokeSecret { .. } => {
            account(conn, op).await
        }
    }
}
