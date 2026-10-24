//! Flow policy module of kartenn (spec 010): classification axes, catalogue of services,
//! rules by attributes and patterns, verdicts with their outcome and explanation,
//! policy differential and inference of expected classifications.
//!
//! This crate holds the pure domain, the module as the core sees it (ADR 0026), and, with
//! the feature `pg`, its handler in the write pipeline and its storage in PostgreSQL.

pub mod diff;
pub mod evaluate;
pub mod inference;
mod labels;
pub mod matrix;
pub mod model;
pub mod module;
pub mod rules;
pub mod snapshot;

#[cfg(feature = "pg")]
pub mod cache;
#[cfg(feature = "pg")]
mod checks;
#[cfg(feature = "pg")]
mod codec;
#[cfg(feature = "pg")]
mod coherence;
#[cfg(feature = "pg")]
mod contribution;
#[cfg(feature = "pg")]
pub mod handler;
#[cfg(feature = "import")]
pub mod import;
#[cfg(feature = "pg")]
pub mod infer;
#[cfg(feature = "pg")]
mod load;
#[cfg(feature = "pg")]
pub mod mismatch;
#[cfg(feature = "pg")]
mod objects;
#[cfg(feature = "pg")]
mod pg;
#[cfg(feature = "pg")]
pub mod preview;
#[cfg(feature = "pg")]
pub mod read;
#[cfg(feature = "pg")]
pub mod report;
#[cfg(feature = "rest")]
pub mod rest;
#[cfg(feature = "rest")]
pub mod stats;
#[cfg(feature = "pg")]
pub mod verdict;

pub use module::Policy;
