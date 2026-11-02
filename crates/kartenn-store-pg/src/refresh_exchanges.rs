//! The exchanges of refresh tokens shared by the instances (spec 021, A24, A25; D9;
//! migration 0074): the claim of an instance that presents a token to its issuer, under a
//! lease, then the sealed result of its exchange, kept a short window. Each row is named by
//! a fingerprint of the token; the database clock decides (`now()`).

use std::time::Duration;

use kartenn_service::authn::{Claim, RefreshExchanges};
use kartenn_service::module::BoxFuture;
use kartenn_service::port::StoreError;
use sqlx::PgPool;
use sqlx::types::Uuid;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// [`RefreshExchanges`] in PostgreSQL.
#[derive(Debug, Clone)]
pub struct PgRefreshExchanges {
    pool: PgPool,
}

impl PgRefreshExchanges {
    /// On `pool`.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn claim(
        &self,
        key: &[u8; 32],
        holder: Uuid,
        lease: Duration,
    ) -> Result<Claim, StoreError> {
        // The lock serializes the reading and the claim among the instances; it is freed
        // at the commit, before the issuer is called (021/A24).
        let mut tx = self.pool.begin().await.map_err(store_err)?;
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('refresh-exchange/' || encode($1, 'hex'), 0))",
        )
        .bind(&key[..])
        .execute(&mut *tx)
        .await
        .map_err(store_err)?;
        let found: Option<(Option<Uuid>, Option<String>)> = sqlx::query_as(
            "SELECT holder, sealed FROM refresh_exchange WHERE key = $1 AND ends_at > now()",
        )
        .bind(&key[..])
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_err)?;
        let claim = match found {
            Some((_, Some(sealed))) => Claim::Kept(sealed),
            // The lease of another holder. Its own claim, still running, means that the
            // request which ran it was dropped: this instance runs no other exchange of
            // the token (`Oidc`), so it takes the claim over at once.
            Some((Some(other), None)) if other != holder => Claim::Busy,
            _ => {
                sqlx::query(
                    "INSERT INTO refresh_exchange (key, holder, ends_at)
                     VALUES ($1, $2, now() + make_interval(secs => $3))
                     ON CONFLICT (key) DO UPDATE
                        SET holder = excluded.holder, sealed = NULL, ends_at = excluded.ends_at",
                )
                .bind(&key[..])
                .bind(holder)
                .bind(lease.as_secs_f64())
                .execute(&mut *tx)
                .await
                .map_err(store_err)?;
                Claim::Ours
            }
        };
        tx.commit().await.map_err(store_err)?;
        Ok(claim)
    }

    async fn keep(
        &self,
        key: &[u8; 32],
        holder: Uuid,
        sealed: &str,
        window: Duration,
    ) -> Result<bool, StoreError> {
        sqlx::query(
            "UPDATE refresh_exchange
                SET holder = NULL, sealed = $3, ends_at = now() + make_interval(secs => $4)
              WHERE key = $1 AND holder = $2",
        )
        .bind(&key[..])
        .bind(holder)
        .bind(sealed)
        .bind(window.as_secs_f64())
        .execute(&self.pool)
        .await
        .map(|done| done.rows_affected() == 1)
        .map_err(store_err)
    }

    async fn release(&self, key: &[u8; 32], holder: Uuid) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM refresh_exchange WHERE key = $1 AND holder = $2")
            .bind(&key[..])
            .bind(holder)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(store_err)
    }
}

impl RefreshExchanges for PgRefreshExchanges {
    fn claim<'a>(
        &'a self,
        key: &'a [u8; 32],
        holder: Uuid,
        lease: Duration,
    ) -> BoxFuture<'a, Result<Claim, StoreError>> {
        Box::pin(self.claim(key, holder, lease))
    }

    fn keep<'a>(
        &'a self,
        key: &'a [u8; 32],
        holder: Uuid,
        sealed: &'a str,
        window: Duration,
    ) -> BoxFuture<'a, Result<bool, StoreError>> {
        Box::pin(self.keep(key, holder, sealed, window))
    }

    fn release<'a>(
        &'a self,
        key: &'a [u8; 32],
        holder: Uuid,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(self.release(key, holder))
    }
}

/// Deletes the exchanges past their end (task `refresh-exchange-purge`); the number
/// deleted.
///
/// # Errors
///
/// Database errors.
pub async fn purge(pool: &PgPool) -> Result<u64, StoreError> {
    sqlx::query("DELETE FROM refresh_exchange WHERE ends_at <= now()")
        .execute(pool)
        .await
        .map(|done| done.rows_affected())
        .map_err(store_err)
}
