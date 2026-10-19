//! Reading the administration of the rights (spec 006, REQ-006-4; task 006-18; design
//! « Droits »): the current roles, bindings, attachments of issuers, service accounts and
//! identities, by pages on their key, and the bindings that serve a subject.

use std::collections::BTreeMap;

use kartenn_service::authz::Subject;
use kartenn_service::port::StoreError;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// A current role.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct RoleRead {
    /// Identifier.
    pub id: Uuid,
    /// Key.
    pub key: String,
    /// Label.
    pub label: String,
    /// Delivered with kartenn.
    pub builtin: bool,
    /// Entries, sorted.
    pub entries: Vec<String>,
}

const ROLES: &str = "SELECT r.id, r.key, r.label, r.builtin,
            coalesce((SELECT array_agg(rp.permission ORDER BY rp.permission)
                      FROM role_permission rp
                      WHERE rp.role_id = r.id AND rp.valid_period @> now()), '{}') AS entries
     FROM role r WHERE r.valid_period @> now()";

/// The current roles after the key `after`, by key.
///
/// # Errors
///
/// Database errors.
pub async fn roles(
    pool: &PgPool,
    after: Option<&str>,
    limit: i64,
) -> Result<Vec<RoleRead>, StoreError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{ROLES} AND ($1::text IS NULL OR r.key > $1) ORDER BY r.key LIMIT $2"
    )))
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(store_err)
}

/// The current role `key`.
///
/// # Errors
///
/// Database errors.
pub async fn role(pool: &PgPool, key: &str) -> Result<Option<RoleRead>, StoreError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("{ROLES} AND r.key = $1")))
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(store_err)
}

/// A current binding.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct BindingRead {
    /// Identifier.
    pub id: Uuid,
    /// `identity`, `group` or `issuer`.
    pub subject_kind: String,
    /// The identity.
    pub subject_id: Option<Uuid>,
    /// The issuer of a group or of every identity.
    pub issuer: Option<String>,
    /// The group.
    pub group_name: Option<String>,
    /// Role key.
    pub role: String,
    /// `global`, `entity`, `site`, `domain` or `prefix`.
    pub scope_kind: String,
    /// The scope; `None` for global.
    pub scope_id: Option<Uuid>,
    /// Start of its period.
    pub valid_from: OffsetDateTime,
    /// End of its period; `None`: open.
    pub valid_to: Option<OffsetDateTime>,
}

const BINDINGS: &str = "SELECT b.id, b.subject_kind, b.subject_id, b.issuer, b.group_name,
            r.key AS role, b.scope_kind, b.scope_id,
            lower(b.valid_period) AS valid_from, upper(b.valid_period) AS valid_to
     FROM binding b JOIN role r ON r.id = b.role_id AND r.valid_period @> now()
     WHERE b.valid_period @> now()";

/// The current bindings after the identifier `after`, by identifier.
///
/// # Errors
///
/// Database errors.
pub async fn bindings(
    pool: &PgPool,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<BindingRead>, StoreError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{BINDINGS} AND ($1::uuid IS NULL OR b.id > $1) ORDER BY b.id LIMIT $2"
    )))
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(store_err)
}

/// The current binding `id`.
///
/// # Errors
///
/// Database errors.
pub async fn binding(pool: &PgPool, id: Uuid) -> Result<Option<BindingRead>, StoreError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("{BINDINGS} AND b.id = $1")))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(store_err)
}

/// The current bindings that serve `subject`: its own, those of its groups at its issuer
/// and those of its issuer, by role and scope.
///
/// # Errors
///
/// Database errors.
pub async fn bindings_of(pool: &PgPool, subject: &Subject) -> Result<Vec<BindingRead>, StoreError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{BINDINGS} AND ((b.subject_kind = 'identity' AND b.subject_id = $1)
                         OR (b.subject_kind = 'group' AND b.issuer = $2 AND b.group_name = ANY($3))
                         OR (b.subject_kind = 'issuer' AND b.issuer = $2))
         ORDER BY r.key, b.scope_kind, b.scope_id, b.id"
    )))
    .bind(subject.id)
    .bind(subject.issuer.as_deref())
    .bind(&subject.groups)
    .fetch_all(pool)
    .await
    .map_err(store_err)
}

/// The entries of the current roles of the global bindings that serve `subject`, sorted.
///
/// # Errors
///
/// Database errors.
pub async fn global_entries(pool: &PgPool, subject: &Subject) -> Result<Vec<String>, StoreError> {
    sqlx::query_scalar(
        "SELECT DISTINCT rp.permission FROM binding b
         JOIN role r ON r.id = b.role_id AND r.valid_period @> now()
         JOIN role_permission rp ON rp.role_id = r.id AND rp.valid_period @> now()
         WHERE b.valid_period @> now() AND b.scope_kind = 'global'
           AND ((b.subject_kind = 'identity' AND b.subject_id = $1)
                OR (b.subject_kind = 'group' AND b.issuer = $2 AND b.group_name = ANY($3))
                OR (b.subject_kind = 'issuer' AND b.issuer = $2))
         ORDER BY rp.permission",
    )
    .bind(subject.id)
    .bind(subject.issuer.as_deref())
    .bind(&subject.groups)
    .fetch_all(pool)
    .await
    .map_err(store_err)
}

/// A current attachment of an issuer.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AttachmentRead {
    /// Issuer.
    pub issuer: String,
    /// Entity; `None`: everywhere.
    pub entity: Option<Uuid>,
}

/// The current attachments after `after`, by issuer then entity (the global one first).
///
/// # Errors
///
/// Database errors.
pub async fn attachments(
    pool: &PgPool,
    after: Option<(&str, Option<Uuid>)>,
    limit: i64,
) -> Result<Vec<AttachmentRead>, StoreError> {
    sqlx::query_as(
        "SELECT issuer, entity_id AS entity FROM issuer_attachment
         WHERE valid_period @> now()
           AND ($1::text IS NULL
                OR (issuer, coalesce(entity_id, '00000000-0000-0000-0000-000000000000'::uuid))
                   > ($1, coalesce($2, '00000000-0000-0000-0000-000000000000'::uuid)))
         ORDER BY issuer, coalesce(entity_id, '00000000-0000-0000-0000-000000000000'::uuid)
         LIMIT $3",
    )
    .bind(after.map(|(issuer, _)| issuer))
    .bind(after.and_then(|(_, entity)| entity))
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(store_err)
}

/// A secret of a service account, without its digest.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SecretRead {
    /// Identifier.
    pub id: Uuid,
    /// The key of the ring that made its digest.
    pub key_id: String,
    /// Created.
    pub created_at: OffsetDateTime,
    /// Expiry.
    pub expires_at: Option<OffsetDateTime>,
    /// Revoked.
    pub revoked: bool,
}

/// A service account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRead {
    /// Identifier.
    pub id: Uuid,
    /// Name.
    pub name: String,
    /// Disabled.
    pub disabled: bool,
    /// Its secrets, oldest first.
    pub secrets: Vec<SecretRead>,
}

/// The service accounts `ids`, or those after `after` by identifier.
async fn accounts_where(
    pool: &PgPool,
    ids: Option<&[Uuid]>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<AccountRead>, StoreError> {
    let rows: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT id, display_name, disabled_at IS NOT NULL FROM person
         WHERE kind = 'service_account'
           AND ($1::uuid[] IS NULL OR id = ANY($1))
           AND ($2::uuid IS NULL OR id > $2)
         ORDER BY id LIMIT $3",
    )
    .bind(ids)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    let found: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    let secrets: Vec<(Uuid, SecretRead)> = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            String,
            OffsetDateTime,
            Option<OffsetDateTime>,
            bool,
        ),
    >(
        "SELECT account_id, id, key_id, created_at, expires_at, revoked_at IS NOT NULL
         FROM service_secret WHERE account_id = ANY($1) ORDER BY created_at, id",
    )
    .bind(&found)
    .fetch_all(pool)
    .await
    .map_err(store_err)?
    .into_iter()
    .map(|(account, id, key_id, created_at, expires_at, revoked)| {
        (
            account,
            SecretRead {
                id,
                key_id,
                created_at,
                expires_at,
                revoked,
            },
        )
    })
    .collect();
    let mut by_account: BTreeMap<Uuid, Vec<SecretRead>> = BTreeMap::new();
    for (account, secret) in secrets {
        by_account.entry(account).or_default().push(secret);
    }
    Ok(rows
        .into_iter()
        .map(|(id, name, disabled)| AccountRead {
            id,
            name,
            disabled,
            secrets: by_account.remove(&id).unwrap_or_default(),
        })
        .collect())
}

/// The service accounts after the identifier `after`, by identifier.
///
/// # Errors
///
/// Database errors.
pub async fn accounts(
    pool: &PgPool,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<AccountRead>, StoreError> {
    accounts_where(pool, None, after, limit).await
}

/// The service account `id`.
///
/// # Errors
///
/// Database errors.
pub async fn account(pool: &PgPool, id: Uuid) -> Result<Option<AccountRead>, StoreError> {
    Ok(accounts_where(pool, Some(&[id]), None, 1).await?.pop())
}

/// The service account a secret `secret` belongs to.
///
/// # Errors
///
/// Database errors.
pub async fn account_of_secret(pool: &PgPool, secret: Uuid) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar("SELECT account_id FROM service_secret WHERE id = $1")
        .bind(secret)
        .fetch_optional(pool)
        .await
        .map_err(store_err)
}

/// An identity and its state.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct IdentityRead {
    /// Identifier.
    pub id: Uuid,
    /// `person`, `service_account`, `breakglass` or `system`.
    pub kind: String,
    /// Display name.
    pub name: String,
    /// Issuer, for a person.
    pub issuer: Option<String>,
    /// Disabled.
    pub disabled: bool,
    /// Its sessions opened until then are revoked.
    pub revoked_after: Option<OffsetDateTime>,
}

const IDENTITIES: &str = "SELECT id, kind, display_name AS name, issuer,
            disabled_at IS NOT NULL AS disabled, revoked_after
     FROM person";

/// The identities after the identifier `after`, by identifier.
///
/// # Errors
///
/// Database errors.
pub async fn identities(
    pool: &PgPool,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<IdentityRead>, StoreError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{IDENTITIES} WHERE ($1::uuid IS NULL OR id > $1) ORDER BY id LIMIT $2"
    )))
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(store_err)
}

/// The identity `id`.
///
/// # Errors
///
/// Database errors.
pub async fn identity(pool: &PgPool, id: Uuid) -> Result<Option<IdentityRead>, StoreError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("{IDENTITIES} WHERE id = $1")))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(store_err)
}

/// The binding that changeset `changeset` created, if any.
///
/// # Errors
///
/// Database errors.
pub async fn created_binding(pool: &PgPool, changeset: Uuid) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar("SELECT id FROM binding WHERE changeset_id = $1 LIMIT 1")
        .bind(changeset)
        .fetch_optional(pool)
        .await
        .map_err(store_err)
}
