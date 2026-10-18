//! Creation of a break-glass account (spec 005, REQ-005-14; K9): by the owner of the schema
//! only (the subcommand `breakglass create`), never by the API; audited.

use kartenn_core::audit::{BREAKGLASS_CREATED, rfc3339};
use kartenn_core::period::Instant as At;
use kartenn_service::StoreError;
use kartenn_service::audit::{Instance, Outcome, RequestContext};
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

/// Creates the account, with the connection of the owner of the schema; its identifier.
///
/// # Errors
///
/// Database errors: a name taken, or a connection without the owner's rights.
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
    let until =
        At(i64::try_from(account.valid_until.unix_timestamp_nanos() / 1_000).map_err(store_err)?);
    let event = RequestContext::system()
        .event(&BREAKGLASS_CREATED, Outcome::Success)
        .map_err(store_err)?
        .target("person", [id])
        .detail("valid_until", rfc3339(until))
        .map_err(store_err)?;
    crate::audit_journal::insert(&mut *tx, &event, instance).await?;
    tx.commit().await.map_err(store_err)?;
    Ok(id)
}
