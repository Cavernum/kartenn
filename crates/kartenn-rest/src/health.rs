//! Probes (REQ-006-36): `/health/live` while the process answers, `/health/ready` while
//! the database answers at the expected migration and every audit output is available.
//! Outside `/v1`, without authentication, with an empty body.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use kartenn_service::audit::AuditStatus;
use sqlx::PgPool;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Whether the instance can serve requests.
pub trait Readiness: Send + Sync {
    /// `true` when ready.
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;
}

/// Longest wait for the database before the instance is declared not ready.
pub const READY_WITHIN: Duration = Duration::from_secs(2);

/// Readiness of an instance on PostgreSQL.
#[derive(Debug, Clone)]
pub struct PgReadiness {
    pool: PgPool,
    audit: AuditStatus,
    expected: i64,
}

impl PgReadiness {
    /// Over `pool`, with the state of the audit outputs; the expected migration is the
    /// last one this build carries.
    #[must_use]
    pub fn new(pool: PgPool, audit: AuditStatus) -> Self {
        let expected = kartenn_store_pg::MIGRATOR
            .iter()
            .map(|m| m.version)
            .max()
            .unwrap_or_default();
        Self {
            pool,
            audit,
            expected,
        }
    }

    async fn migrated(&self) -> bool {
        let query = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT max(version) FROM _sqlx_migrations WHERE success",
        )
        .fetch_one(&self.pool);
        matches!(
            tokio::time::timeout(READY_WITHIN, query).await,
            Ok(Ok(Some(version))) if version >= self.expected
        )
    }
}

impl Readiness for PgReadiness {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async move { self.audit.available() && self.migrated().await })
    }
}

/// The process answers.
#[utoipa::path(
    get,
    path = "/health/live",
    operation_id = "live",
    tag = "operations",
    responses((status = 200, description = "The process answers."))
)]
async fn live() -> StatusCode {
    StatusCode::OK
}

/// The database answers at the expected migration and the audit journal is available.
#[utoipa::path(
    get,
    path = "/health/ready",
    operation_id = "ready",
    tag = "operations",
    responses(
        (status = 200, description = "Ready."),
        (status = 503, description = "Not ready."),
    )
)]
async fn ready(State(readiness): State<Arc<dyn Readiness>>) -> StatusCode {
    if readiness.ready().await {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

/// The routes `/health/live` and `/health/ready`.
pub fn routes() -> OpenApiRouter<Arc<dyn Readiness>> {
    OpenApiRouter::new()
        .routes(routes!(live))
        .routes(routes!(ready))
}
