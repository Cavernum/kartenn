//! Filters of the technical outputs changed at run time (spec 012, REQ-012-27).

use kartenn_core::audit::AuditEvent;
use kartenn_core::period::Instant;
use kartenn_service::StoreError;
use kartenn_service::observability::{Override, Overrides};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::write::PgStore;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

fn time(at: Instant) -> Result<OffsetDateTime, StoreError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(at.0) * 1000).map_err(store_err)
}

impl Overrides for PgStore {
    async fn set_override(
        &self,
        over: &Override,
        by: Option<Uuid>,
        event: &AuditEvent,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool().begin().await.map_err(store_err)?;
        crate::audit_journal::insert(&mut *tx, event, self.instance()).await?;
        sqlx::query(
            "INSERT INTO observability_override (output, filter, expires_at, set_by)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (output) DO UPDATE SET filter = EXCLUDED.filter,
                 expires_at = EXCLUDED.expires_at, set_by = EXCLUDED.set_by, set_at = now()",
        )
        .bind(&over.output)
        .bind(&over.filter)
        .bind(time(over.expires_at)?)
        .bind(by)
        .execute(&mut *tx)
        .await
        .map_err(store_err)?;
        tx.commit().await.map_err(store_err)
    }

    async fn overrides(&self) -> Result<Vec<Override>, StoreError> {
        let rows: Vec<(String, String, OffsetDateTime)> = sqlx::query_as(
            "SELECT output, filter, expires_at FROM observability_override WHERE expires_at > now()",
        )
        .fetch_all(self.pool())
        .await
        .map_err(store_err)?;
        rows.into_iter()
            .map(|(output, filter, expires_at)| {
                let micros =
                    i64::try_from(expires_at.unix_timestamp_nanos() / 1000).map_err(store_err)?;
                Ok(Override {
                    output,
                    filter,
                    expires_at: Instant(micros),
                })
            })
            .collect()
    }
}
