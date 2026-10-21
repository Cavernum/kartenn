//! The integration tests of kartenn-store-pg, one binary for the crate (ADR 0027): a
//! module per subject.

mod audit_schema;
mod changesets;
mod core_objects;
mod dated_reads;
mod harness;
mod history_maintenance;
mod keys;
mod module_migrations;
mod properties;
mod rbac_schema;
mod special_registry;
mod versioning;
