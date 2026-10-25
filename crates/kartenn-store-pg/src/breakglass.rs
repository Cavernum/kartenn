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

/// The delivered group a break-glass account belongs to (spec 025, G7; 005/K17), bound to
/// the administrator role on the global scope by the synchronisation of the core.
pub const GROUP: &str = kartenn_core::authz::SECOURS.key;

/// Creates the account, with the connection of the owner of the schema; its identifier.
///
/// In the same transaction, makes the account a member of the delivered group `secours`
/// from now to `valid_until` (spec 025, G7; 005, K17): the bootstrap of a new instance.
/// The subcommand has no calling identity, so the membership is written here, under a
/// changeset of the system author, rather than through the pipeline.
///
/// # Errors
///
/// Database errors: a name taken, a connection without the owner's rights, or no
/// delivered group `secours` yet (the migration job synchronises it).
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
        join_secours(&mut tx, id, account, start).await?;
    }
    let until =
        At(i64::try_from(account.valid_until.unix_timestamp_nanos() / 1_000).map_err(store_err)?);
    let event = RequestContext::system()
        .event(&BREAKGLASS_CREATED, Outcome::Success)
        .map_err(store_err)?
        .target("person", [id])
        .detail("valid_until", rfc3339(until))
        .map_err(store_err)?
        .detail("group", GROUP)
        .map_err(store_err)?;
    crate::audit_journal::insert(&mut *tx, &event, instance).await?;
    tx.commit().await.map_err(store_err)?;
    Ok(id)
}

/// Makes the account `id` a member of [`GROUP`] over `[start, valid_until)`, under a
/// changeset of the system author (spec 025, G7).
async fn join_secours(
    tx: &mut sqlx::PgConnection,
    id: Uuid,
    account: &Stored<'_>,
    start: OffsetDateTime,
) -> Result<(), StoreError> {
    let group: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM rbac_group WHERE key = $1 AND builtin AND valid_period @> now()",
    )
    .bind(GROUP)
    .fetch_optional(&mut *tx)
    .await
    .map_err(store_err)?;
    let group = group.ok_or_else(|| {
        StoreError(format!(
            "no delivered group '{GROUP}': run `kartenn-server migrate` first"
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
        "INSERT INTO group_member (id, group_id, member_kind, identity_id, valid_period,
                                   changeset_id)
         VALUES ($1, $2, 'identity', $3, tstzrange($4, $5), $6)",
    )
    .bind(Uuid::now_v7())
    .bind(group)
    .bind(id)
    .bind(start)
    .bind(account.valid_until)
    .bind(changeset)
    .execute(&mut *tx)
    .await
    .map_err(store_err)?;
    Ok(())
}
