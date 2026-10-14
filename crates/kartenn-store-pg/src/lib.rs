//! PostgreSQL storage adapter of kartenn.

pub mod registry;
pub mod temporal;

/// Schema migrations of the core, applied by the dedicated migration job (D9).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
