//! Maintenance of the history tables (spec 004, REQ-004-17, 38; task 004-25; ADR 0017).

use sqlx::PgPool;

/// Months of history partitions kept ahead of time by [`history_partitions`].
pub const MONTHS_AHEAD: i32 = 3;

/// Creates the monthly partitions of every history table for the current month and the
/// next `months` - 1, so that the default partition stays empty; existing partitions are
/// left alone and a month already holding rows in the default partition is skipped. Run
/// with the owner of the schema: by the migration job, then at least monthly.
///
/// # Errors
///
/// Database errors (the pool must be allowed to create tables).
pub async fn history_partitions(pool: &PgPool, months: i32) -> sqlx::Result<()> {
    sqlx::query("CALL kartenn_history_maintenance($1)")
        .bind(months)
        .execute(pool)
        .await?;
    Ok(())
}

/// Why a deployment failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DeployError {
    /// A migration failed.
    #[error(transparent)]
    Migration(#[from] sqlx::migrate::MigrateError),
    /// Database failure.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// The synchronisation of the declarations could not run.
    #[error("synchronising the declarations: {0}")]
    Sync(#[from] kartenn_service::RequestError),
    /// The declarations in code were refused: the deployment must stop (003/P12).
    #[error("the declarations of module {module} were refused: {reasons}")]
    Refused {
        /// Module.
        module: String,
        /// Rules and messages.
        reasons: String,
    },
}

/// The migration job (D9): migrations, history partitions ahead of time, the IANA
/// special-purpose registries embedded in the code, then the declarations of the core
/// synchronised from the code (003/P12, ADR 0014). Run once per
/// deployment, before the instances, with the owner of the schema.
///
/// # Errors
///
/// [`DeployError`]; a refused synchronisation fails the deployment.
pub async fn deploy(pool: &PgPool) -> Result<(), DeployError> {
    crate::MIGRATOR.run(pool).await?;
    history_partitions(pool, MONTHS_AHEAD).await?;
    // REQ-012-8: the migration job is audited, by the system.
    let event = kartenn_service::audit::RequestContext::system()
        .event(
            &kartenn_core::audit::SYS_MIGRATED,
            kartenn_service::audit::Outcome::Success,
        )
        .and_then(|e| e.detail("version", env!("CARGO_PKG_VERSION")))
        .map_err(|e| DeployError::Database(sqlx::Error::Protocol(e.to_string())))?;
    crate::audit_journal::insert(
        pool,
        &event,
        &kartenn_service::audit::Instance::new(env!("CARGO_PKG_VERSION")),
    )
    .await
    .map_err(|e| DeployError::Database(sqlx::Error::Protocol(e.0)))?;
    // The registries embedded in the code, under their version (REQ-002-34; idempotent).
    for family in kartenn_core::family::ALL {
        if let Some(registry) = kartenn_core::registry::special_registry(family) {
            crate::registry::load(pool, registry).await?;
        }
    }
    let modules = [kartenn_service::declarations::core_declarations()];
    let store = crate::write::PgStore::new(pool.clone());
    let outcomes = kartenn_service::declarations::sync_modules(
        &store,
        &kartenn_service::authz::Unrestricted,
        &modules,
        kartenn_service::pipeline::MAX_OPERATIONS,
    )
    .await?;
    for outcome in outcomes {
        if let kartenn_service::Outcome::Refused { violations, .. } = outcome {
            let reasons = violations
                .iter()
                .map(|v| format!("{}: {}", v.rule.id, v.message))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(DeployError::Refused {
                module: "core".into(),
                reasons,
            });
        }
    }
    Ok(())
}
