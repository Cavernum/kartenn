//! The integration tests of kartenn-policy, one binary for the crate (ADR 0027): a module
//! per subject, the environment of the module in `support`.

mod support;

mod axes;
mod cache;
mod catalogue;
mod coherence;
mod dated;
mod differential;
mod import;
mod inference;
mod labels;
mod matrix;
mod mismatch;
mod obligation;
mod preview;
mod report;
mod rest;
mod rights;
mod settings_migration;
mod verdict;
