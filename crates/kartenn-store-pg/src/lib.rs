//! PostgreSQL storage adapter of kartenn.

pub mod anomalies;
pub mod audit;
mod diff;
mod expected;
pub mod explain;
pub mod filter;
pub mod keys;
pub mod maintenance;
mod obligations;
mod origins;
pub mod outbox;
mod pending;
mod properties;
pub mod registry;
pub mod temporal;
pub mod tree;
mod undo;
pub mod write;

/// Schema migrations of the core, applied by the dedicated migration job (D9).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
