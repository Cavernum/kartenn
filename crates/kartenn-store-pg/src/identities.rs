//! The identities as authentication reads them (spec 005, REQ-005-3, 11, 12): service
//! accounts with their secrets, read at each request (revocation is immediate), and the
//! digest of a claimed identifier under the instance key of the audit (REQ-012-9).

use hmac::{Hmac, KeyInit, Mac};
use kartenn_core::period::Instant;
use kartenn_service::StoreError;
use kartenn_service::authn::{Identities, ServiceAccount, StoredSecret};
use sha2::Sha256;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::write::PgStore;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// A secret row: id, digest, key, expiry, revoked.
type SecretRow = (Uuid, Vec<u8>, String, Option<OffsetDateTime>, bool);

impl Identities for PgStore {
    async fn service_account(&self, name: &str) -> Result<Option<ServiceAccount>, StoreError> {
        let account: Option<(Uuid, bool)> = sqlx::query_as(
            "SELECT id, disabled_at IS NOT NULL FROM person
             WHERE kind = 'service_account' AND display_name = $1",
        )
        .bind(name)
        .fetch_optional(self.pool())
        .await
        .map_err(store_err)?;
        let Some((id, disabled)) = account else {
            return Ok(None);
        };
        let rows: Vec<SecretRow> = sqlx::query_as(
            "SELECT id, digest, key_id, expires_at, revoked_at IS NOT NULL
             FROM service_secret WHERE account_id = $1 ORDER BY created_at",
        )
        .bind(id)
        .fetch_all(self.pool())
        .await
        .map_err(store_err)?;
        let secrets = rows
            .into_iter()
            .filter_map(|(id, digest, key_id, expires, revoked)| {
                Some(StoredSecret {
                    id,
                    digest: digest.try_into().ok()?,
                    key_id,
                    expires_at: expires.and_then(|t| {
                        i64::try_from(t.unix_timestamp_nanos() / 1_000)
                            .ok()
                            .map(Instant)
                    }),
                    revoked,
                })
            })
            .collect();
        Ok(Some(ServiceAccount {
            id,
            disabled,
            secrets,
        }))
    }

    async fn upsert_person(
        &self,
        issuer: &str,
        subject: &str,
        name: &str,
    ) -> Result<(Uuid, bool), StoreError> {
        // A pseudonymised person keeps its pseudonym (004, REQ-004-33).
        sqlx::query_as(
            "INSERT INTO person (id, display_name, issuer, subject, kind)
             VALUES ($1, $2, $3, $4, 'person')
             ON CONFLICT (issuer, subject) DO UPDATE
                 SET display_name = CASE WHEN person.pseudonymized_at IS NULL
                                         THEN EXCLUDED.display_name ELSE person.display_name END
             RETURNING id, disabled_at IS NOT NULL",
        )
        .bind(Uuid::now_v7())
        .bind(name)
        .bind(issuer)
        .bind(subject)
        .fetch_one(self.pool())
        .await
        .map_err(store_err)
    }

    async fn claimed_digest(&self, claimed: &str) -> Result<String, StoreError> {
        let key: Vec<u8> = sqlx::query_scalar("SELECT hmac_key FROM audit.instance_key")
            .fetch_one(self.pool())
            .await
            .map_err(store_err)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(&key).map_err(store_err)?;
        mac.update(claimed.as_bytes());
        Ok(kartenn_core::audit::seal::hex(&mac.finalize().into_bytes()))
    }
}
