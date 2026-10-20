//! The migrations of the business modules (spec 008, REQ-008-2; ADR 0026, clause 3): each
//! module in its own schema, after the core and its dependencies, each migration once,
//! tracked in `module_migration` with the SHA-256 of its SQL.

use kartenn_service::module::{ModuleMigration, ModuleRegistry};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

/// Why the migrations of a module stop the job.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ModuleMigrationError {
    /// A migration applied before has changed since.
    #[error("module {module}: migration {version} ({name}) changed since it was applied")]
    Changed {
        /// The module.
        module: String,
        /// The version.
        version: i64,
        /// Its name.
        name: String,
    },
    /// Versions not strictly increasing in the code of the module.
    #[error("module {module}: migration versions must increase, {version} is out of order")]
    Order {
        /// The module.
        module: String,
        /// The version out of order.
        version: i64,
    },
    /// Database failure, or a migration that failed.
    #[error("module {module}: {source}")]
    Database {
        /// The module.
        module: String,
        /// The failure.
        source: sqlx::Error,
    },
}

/// The SHA-256 of a migration.
fn checksum(migration: &ModuleMigration) -> Vec<u8> {
    Sha256::digest(migration.sql.as_bytes()).to_vec()
}

/// Applies the migrations of every module of `registry` that this database lacks, in the
/// order of the registry, each in a transaction of its own with the schema of the module
/// first on the search path. Modules absent from the build are left alone: their schema
/// and their rows stay. Run by the migration job after the migrations of the core.
///
/// # Errors
///
/// [`ModuleMigrationError`]: a changed migration, versions out of order, or a failure.
pub async fn migrate(pool: &PgPool, registry: &ModuleRegistry) -> Result<(), ModuleMigrationError> {
    for module in registry.modules() {
        let name = module.name();
        let failed = |source| ModuleMigrationError::Database {
            module: name.to_owned(),
            source,
        };
        let mut last = i64::MIN;
        for migration in module.migrations() {
            if migration.version <= last {
                return Err(ModuleMigrationError::Order {
                    module: name.to_owned(),
                    version: migration.version,
                });
            }
            last = migration.version;
            let sum = checksum(migration);
            let mut tx = pool.begin().await.map_err(failed)?;
            // One job at a time per database, like the migrations of the core.
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext('kartenn.module_migration'))")
                .execute(&mut *tx)
                .await
                .map_err(failed)?;
            let applied: Option<Vec<u8>> = sqlx::query_scalar(
                "SELECT checksum FROM module_migration WHERE module = $1 AND version = $2",
            )
            .bind(name)
            .bind(migration.version)
            .fetch_optional(&mut *tx)
            .await
            .map_err(failed)?;
            match applied {
                Some(applied) if applied == sum => continue,
                Some(_) => {
                    return Err(ModuleMigrationError::Changed {
                        module: name.to_owned(),
                        version: migration.version,
                        name: migration.name.to_owned(),
                    });
                }
                None => {}
            }
            // The name is checked by the registry (`[a-z][a-z0-9_]*`): safe as an identifier.
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE SCHEMA IF NOT EXISTS {name}; \
                 GRANT USAGE ON SCHEMA {name} TO kartenn_app; \
                 SET LOCAL search_path = {name}, public;"
            )))
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
            sqlx::raw_sql(migration.sql)
                .execute(&mut *tx)
                .await
                .map_err(failed)?;
            sqlx::query(
                "INSERT INTO public.module_migration (module, version, name, checksum) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(name)
            .bind(migration.version)
            .bind(migration.name)
            .bind(&sum)
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
            tx.commit().await.map_err(failed)?;
            tracing::info!(
                module = name,
                version = migration.version,
                migration = migration.name,
                "module migration applied"
            );
        }
    }
    Ok(())
}
