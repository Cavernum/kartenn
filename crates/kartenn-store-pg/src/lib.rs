//! PostgreSQL storage adapter of kartenn.

pub mod anomalies;
pub mod audit;
pub mod audit_journal;
pub mod audit_purge;
pub mod audit_relay;
pub mod audit_seal;
pub mod audit_verify;
pub mod breakglass;
pub mod changesets;
pub mod clock;
mod diff;
mod expected;
pub mod explain;
pub mod filter;
pub(crate) mod floor;
pub mod idempotency;
mod identities;
pub mod imports;
pub mod jobs;
pub mod keys;
pub mod list;
pub mod maintenance;
mod obligations;
mod origins;
pub mod outbox;
mod overrides;
mod pending;
mod properties;
pub mod property_read;
pub mod rbac;
mod rbac_admin;
pub mod rbac_read;
pub mod read;
pub mod registry;
pub mod temporal;
pub mod tree;
mod undo;
pub mod write;

/// Schema migrations of the core, applied by the dedicated migration job (D9).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
