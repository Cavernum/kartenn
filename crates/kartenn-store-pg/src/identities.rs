//! The identities as authentication reads them (spec 005, REQ-005-3, 11, 12): service
//! accounts with their secrets, read at each request (revocation is immediate), and the
//! digest of a claimed identifier under the instance key of the audit (REQ-012-9).

use hmac::{Hmac, KeyInit, Mac};
use kartenn_core::period::Instant;
use kartenn_service::StoreError;
use kartenn_service::authn::{
    BREAKGLASS_FAILURES_MAX, BreakglassAccount, BreakglassAttempt, BreakglassRecorded, HeldBinding,
    Identities, IdentityState, Rights, ServiceAccount, StoredSecret,
};
use kartenn_service::authz::Subject;
use sha2::Sha256;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::write::PgStore;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// A break-glass row: id, hash, sealed TOTP secret, last step, failures, validity,
/// disabled.
type BreakglassRow = (Uuid, String, Vec<u8>, i64, i32, OffsetDateTime, bool);

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

    async fn identity_state(&self, identity: Uuid) -> Result<Option<IdentityState>, StoreError> {
        let row: Option<(bool, Option<OffsetDateTime>)> = sqlx::query_as(
            "SELECT disabled_at IS NOT NULL, revoked_after FROM person WHERE id = $1",
        )
        .bind(identity)
        .fetch_optional(self.pool())
        .await
        .map_err(store_err)?;
        Ok(row.map(|(disabled, revoked)| IdentityState {
            disabled,
            revoked_after: revoked.and_then(|t| {
                i64::try_from(t.unix_timestamp_nanos() / 1_000)
                    .ok()
                    .map(Instant)
            }),
        }))
    }

    async fn breakglass(&self, name: &str) -> Result<Option<BreakglassAccount>, StoreError> {
        let row: Option<BreakglassRow> = sqlx::query_as(
            "SELECT p.id, b.password_hash, b.totp_secret, b.last_totp_step, b.failures,
                        b.valid_until, p.disabled_at IS NOT NULL
                 FROM person p JOIN breakglass_account b ON b.account_id = p.id
                 WHERE p.kind = 'breakglass' AND p.display_name = $1",
        )
        .bind(name)
        .fetch_optional(self.pool())
        .await
        .map_err(store_err)?;
        Ok(row.map(
            |(id, password_hash, totp_sealed, last_step, failures, valid_until, disabled)| {
                BreakglassAccount {
                    id,
                    password_hash,
                    totp_sealed,
                    last_step,
                    failures,
                    valid_until: Instant(
                        i64::try_from(valid_until.unix_timestamp_nanos() / 1_000)
                            .unwrap_or(i64::MAX),
                    ),
                    disabled,
                }
            },
        ))
    }

    async fn record_breakglass(
        &self,
        id: Uuid,
        attempt: BreakglassAttempt,
    ) -> Result<BreakglassRecorded, StoreError> {
        let mut tx = self.pool().begin().await.map_err(store_err)?;
        let recorded = match attempt {
            // Conditional: of two requests with the same code, one updates the row, the
            // other finds it already past that step.
            BreakglassAttempt::Accepted(step) => sqlx::query(
                "UPDATE breakglass_account SET failures = 0, last_totp_step = $2
                 WHERE account_id = $1 AND last_totp_step < $2",
            )
            .bind(id)
            .bind(step)
            .execute(&mut *tx)
            .await
            .map_err(store_err)
            .map(|done| {
                if done.rows_affected() == 1 {
                    BreakglassRecorded::Taken
                } else {
                    BreakglassRecorded::Replayed
                }
            })?,
            BreakglassAttempt::Failed => BreakglassRecorded::Failures(
                sqlx::query_scalar(
                    "UPDATE breakglass_account SET failures = failures + 1
                     WHERE account_id = $1 RETURNING failures",
                )
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(store_err)?,
            ),
        };
        if let BreakglassRecorded::Failures(failures) = recorded
            && failures >= BREAKGLASS_FAILURES_MAX
        {
            sqlx::query(
                "UPDATE person SET disabled_at = coalesce(disabled_at, now()) WHERE id = $1",
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(store_err)?;
        }
        tx.commit().await.map_err(store_err)?;
        Ok(recorded)
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

    async fn rights(&self, subject: &Subject) -> Result<Rights, StoreError> {
        let mut conn = self.pool().acquire().await.map_err(store_err)?;
        let roles = crate::rbac::roles_of_on(&mut conn, subject).await?;
        drop(conn);
        let name = crate::rbac_read::identity(self.pool(), subject.id)
            .await?
            .map(|i| i.name);
        let bindings = crate::rbac_read::bindings_of(self.pool(), subject)
            .await?
            .into_iter()
            .map(|b| HeldBinding {
                role: b.role,
                scope_kind: b.scope_kind,
                scope: b.scope_id,
            })
            .collect();
        let global_permissions = crate::rbac_read::global_entries(self.pool(), subject).await?;
        Ok(Rights {
            name,
            roles,
            bindings,
            global_permissions,
        })
    }
}
