//! PostgreSQL storage adapter of kartenn.

pub mod anomalies;
pub mod audit;
mod diff;
pub mod keys;
mod obligations;
mod origins;
pub mod outbox;
mod properties;
pub mod registry;
pub mod temporal;
pub mod tree;
pub mod write;

/// Schema migrations of the core, applied by the dedicated migration job (D9).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
