//! The integration tests of kartenn-service, one binary for the crate (ADR 0027): a
//! module per subject, the helpers in `support`.

mod support;

mod anchor_trees;
mod anchors;
mod anomalies;
mod anycast;
mod audit;
mod audit_events;
mod audit_journal;
mod audit_json_schema;
mod audit_reads;
mod audit_seal;
mod audit_verify;
mod cascade;
mod declarations;
mod diff;
mod expected;
mod explain;
mod filter;
mod grouped;
mod heirs;
mod import;
mod locks;
mod module_anchors;
mod module_catalogues;
mod module_diff;
mod module_pipeline;
mod obligations;
mod outbox;
mod pass_floor;
mod pipeline;
mod portion;
mod ranges;
mod rbac;
mod read;
mod references;
mod rename;
mod resolution;
mod review;
mod rights;
mod tree;
mod tree_ranges;
mod undo;
