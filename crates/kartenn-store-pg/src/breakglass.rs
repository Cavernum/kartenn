//! Creation of a break-glass account (spec 005, REQ-005-14; K9): by the owner of the schema
//! only (the subcommand `breakglass create`), never by the API; audited.

use kartenn_core::audit::{BREAKGLASS_CREATED, rfc3339};
use kartenn_core::period::Instant as At;
use kartenn_service::StoreError;
use kartenn_service::audit::{Instance, Outcome, RequestContext};
use kartenn_service::declarations::SYSTEM_AUTHOR;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// What the account stores (from `kartenn_auth::breakglass::create`).
#[derive(Debug, Clone)]
pub struct Stored<'a> {
    /// Its login.
    pub name: &'a str,
    /// Argon2id hash of the passphrase.
    pub password_hash: &'a str,
    /// TOTP secret sealed under the key ring.
    pub totp_sealed: &'a [u8],
    /// Key of the ring that sealed it.
    pub totp_key_id: &'a str,
    /// End of its validity.
    pub valid_until: OffsetDateTime,
}

/// The delivered role a break-glass account holds (K17).
pub const ROLE: &str = "administrator";

/// Creates the account, with the connection of the owner of the schema; its identifier.
///
/// In the same transaction, binds the account to the delivered role `administrator`,
/// global scope, from now to `valid_until` (spec 005, K17): the bootstrap of a new
/// instance. The subcommand has no calling identity, so the binding is written here, under
/// a changeset of the system author, rather than through the pipeline.
///
/// # Errors
///
/// Database errors: a name taken, a connection without the owner's rights, or no
/// delivered `administrator` role yet (the migration job synchronises it).
pub async fn create(
    pool: &PgPool,
    account: &Stored<'_>,
    instance: &Instance,
) -> Result<Uuid, StoreError> {
    let id = Uuid::now_v7();
    let mut tx = pool.begin().await.map_err(store_err)?;
    sqlx::query("INSERT INTO person (id, display_name, kind) VALUES ($1, $2, 'breakglass')")
        .bind(id)
        .bind(account.name)
        .execute(&mut *tx)
        .await
        .map_err(store_err)?;
    sqlx::query(
        "INSERT INTO breakglass_account (account_id, password_hash, totp_secret, totp_key_id, valid_until)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(account.password_hash)
    .bind(account.totp_sealed)
    .bind(account.totp_key_id)
    .bind(account.valid_until)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?;
    let start = OffsetDateTime::now_utc();
    if account.valid_until > start {
        bind_administrator(&mut tx, id, account, start).await?;
    }
    let until =
        At(i64::try_from(account.valid_until.unix_timestamp_nanos() / 1_000).map_err(store_err)?);
    let event = RequestContext::system()
        .event(&BREAKGLASS_CREATED, Outcome::Success)
        .map_err(store_err)?
        .target("person", [id])
        .detail("valid_until", rfc3339(until))
        .map_err(store_err)?
        .detail("role", ROLE)
        .map_err(store_err)?
        .detail("scope", "global")
        .map_err(store_err)?;
    crate::audit_journal::insert(&mut *tx, &event, instance).await?;
    tx.commit().await.map_err(store_err)?;
    Ok(id)
}

/// Binds the account `id` to [`ROLE`], global scope, over `[start, valid_until)`, under a
/// changeset of the system author (K17).
async fn bind_administrator(
    tx: &mut sqlx::PgConnection,
    id: Uuid,
    account: &Stored<'_>,
    start: OffsetDateTime,
) -> Result<(), StoreError> {
    let role: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM role WHERE key = $1 AND builtin AND valid_period @> now()",
    )
    .bind(ROLE)
    .fetch_optional(&mut *tx)
    .await
    .map_err(store_err)?;
    let role = role.ok_or_else(|| {
        StoreError(format!(
            "no delivered role '{ROLE}': run `kartenn-server migrate` first"
        ))
    })?;
    let changeset = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period, applied_at)
         VALUES ($1, $2, $3, 'applied', 'direct', tstzrange($4, $5), now())",
    )
    .bind(changeset)
    .bind(SYSTEM_AUTHOR)
    .bind(format!("break-glass account {}", account.name))
    .bind(start)
    .bind(account.valid_until)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?;
    sqlx::query(
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, valid_period,
                              changeset_id)
         VALUES ($1, 'identity', $2, $3, 'global', tstzrange($4, $5), $6)",
    )
    .bind(Uuid::now_v7())
    .bind(id)
    .bind(role)
    .bind(start)
    .bind(account.valid_until)
    .bind(changeset)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?;
    Ok(())
}
