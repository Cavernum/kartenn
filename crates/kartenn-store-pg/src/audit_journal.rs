//! Audit events in the `audit` schema (spec 012, REQ-012-10 to 12; ADR 0019): the
//! adapter of the port `AuditJournal`, and the insertion shared with `WriteTx::audit`.

use kartenn_core::audit::AuditEvent;
use kartenn_service::StoreError;
use kartenn_service::audit::{AuditJournal, Instance};
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgExecutor, PgPool};

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Inserts `event`, recorded by `instance`, with `executor` (a pool or a transaction).
pub(crate) async fn insert<'e>(
    executor: impl PgExecutor<'e>,
    event: &AuditEvent,
    instance: &Instance,
) -> Result<(), StoreError> {
    let occurred_at =
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(event.occurred_at.0) * 1000)
            .map_err(store_err)?;
    let (target_kind, target_ids) = event
        .target
        .as_ref()
        .map_or((None, None), |t| (Some(t.kind.as_str()), Some(&t.ids)));
    sqlx::query(
        "INSERT INTO audit.event
             (id, name, severity, criticality, occurred_at, actor_kind, actor_id, auth_method,
              issuer, claimed, source_ip, user_agent, target_kind, target_ids, outcome, reason,
              motive, changeset_id, trace_id, span_id, request_id, service_version,
              instance_id, details)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::inet, $12, $13, $14, $15, $16,
                 $17, $18, $19, $20, $21, $22, $23, $24)",
    )
    .bind(event.id)
    .bind(event.kind.name)
    .bind(event.kind.severity.as_str())
    .bind(event.kind.criticality.as_str())
    .bind(occurred_at)
    .bind(event.actor.kind.as_str())
    .bind(event.actor.id)
    .bind(event.actor.auth_method.as_deref())
    .bind(event.actor.issuer.as_deref())
    .bind(event.actor.claimed.as_deref())
    .bind(event.source.ip.map(|ip| ip.to_string()))
    .bind(event.source.user_agent.as_deref())
    .bind(target_kind)
    .bind(target_ids)
    .bind(event.outcome.as_str())
    .bind(event.reason.as_deref())
    .bind(event.motive.as_deref())
    .bind(event.changeset)
    .bind(event.correlation.trace_id.as_deref())
    .bind(event.correlation.span_id.as_deref())
    .bind(event.correlation.request_id.as_deref())
    .bind(instance.version)
    .bind(instance.id)
    .bind(sqlx::types::Json(&event.details))
    .execute(executor)
    .await
    .map(|_| ())
    .map_err(store_err)
}

/// The audit journal in PostgreSQL, for events outside a write transaction.
#[derive(Debug, Clone)]
pub struct PgAuditJournal {
    pool: PgPool,
    instance: Instance,
}

impl PgAuditJournal {
    /// A journal on `pool`, recording as `instance`.
    #[must_use]
    pub const fn new(pool: PgPool, instance: Instance) -> Self {
        Self { pool, instance }
    }

    /// The instance it records as.
    #[must_use]
    pub const fn instance(&self) -> &Instance {
        &self.instance
    }
}

impl AuditJournal for PgAuditJournal {
    async fn record(&self, event: &AuditEvent) -> Result<(), StoreError> {
        insert(&self.pool, event, &self.instance).await
    }
}
