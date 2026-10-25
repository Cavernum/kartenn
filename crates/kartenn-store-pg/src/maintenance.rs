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
    /// The migrations of a module (REQ-008-2).
    #[error(transparent)]
    Module(#[from] crate::modules::ModuleMigrationError),
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

/// What a deployment has to tell its operator.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct Deployed {
    /// The bindings of the former subjects reported on groups by this deployment (spec
    /// 025, REQ-025-14), listed once.
    pub rbac_report: Option<crate::rbac_migration::Report>,
}

/// The migration job (D9): migrations of the core, then of the modules of `modules` in
/// their order (REQ-008-2), history partitions ahead of time, the IANA special-purpose
/// registries embedded in the code, then the declarations of the core synchronised from
/// the code (003/P12, ADR 0014). Run once per deployment, before the instances, with the
/// owner of the schema.
///
/// # Errors
///
/// [`DeployError`]; a refused synchronisation or a changed module migration fails the
/// deployment.
pub async fn deploy(
    pool: &PgPool,
    modules: &kartenn_service::module::ModuleRegistry,
) -> Result<Deployed, DeployError> {
    crate::MIGRATOR.run(pool).await?;
    // The report of the bindings on groups the migrations made, listed and audited once
    // (spec 025, REQ-025-14).
    let rbac_report = crate::rbac_migration::report(
        pool,
        &kartenn_service::audit::Instance::new(env!("CARGO_PKG_VERSION")),
    )
    .await
    .map_err(|e| DeployError::Database(sqlx::Error::Protocol(e.0)))?;
    crate::modules::migrate(pool, modules).await?;
    // The background tasks of the modules (REQ-016-16).
    crate::jobs::declare_module_jobs(pool, &modules.jobs())
        .await
        .map_err(|e| DeployError::Database(sqlx::Error::Protocol(e.0)))?;
    // Their default approval policies, on their first deployment only (REQ-016-8).
    crate::pending::install_default_policies(pool, modules)
        .await
        .map_err(|e| DeployError::Database(sqlx::Error::Protocol(e.0)))?;
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
    // The declarations of the core, completed by the types of the modules (REQ-008-6),
    // then those of each module, in registry order.
    let declarations: Vec<_> = std::iter::once(
        kartenn_service::declarations::core_declarations_for(modules),
    )
    .chain(modules.modules().filter_map(|m| m.declarations()))
    .collect();
    let store =
        crate::write::PgStore::new(pool.clone()).with_modules(std::sync::Arc::new(modules.clone()));
    let outcomes = kartenn_service::declarations::sync_modules(
        &store,
        &kartenn_service::authz::Unrestricted,
        &declarations,
        kartenn_service::pipeline::MAX_OPERATIONS,
    )
    .await?;
    // The delivered roles (spec 005, K12).
    let roles =
        kartenn_service::rbac::sync_roles(&store, kartenn_service::pipeline::MAX_OPERATIONS)
            .await?;
    // The old permission names of the custom roles (spec 018, REQ-018-4; idempotent).
    let renamed = kartenn_service::rbac::rename_permissions(
        &store,
        kartenn_service::pipeline::MAX_OPERATIONS,
    )
    .await?;
    for outcome in outcomes.into_iter().chain(roles).chain(renamed) {
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
    Ok(Deployed { rbac_report })
}
