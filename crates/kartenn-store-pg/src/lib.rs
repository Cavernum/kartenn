//! PostgreSQL storage adapter of kartenn.

pub mod anomalies;
pub mod audit;
pub mod audit_journal;
pub mod audit_purge;
pub mod audit_relay;
pub mod audit_seal;
pub mod audit_verify;
mod diff;
mod expected;
pub mod explain;
pub mod filter;
mod imports;
pub mod keys;
pub mod maintenance;
mod obligations;
mod origins;
pub mod outbox;
mod overrides;
mod pending;
mod properties;
pub mod read;
pub mod registry;
pub mod temporal;
pub mod tree;
mod undo;
pub mod write;

/// Schema migrations of the core, applied by the dedicated migration job (D9).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
