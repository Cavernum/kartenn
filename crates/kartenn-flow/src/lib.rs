//! Flow registry module of kartenn (spec 011): requested flows between two endpoints for
//! a service, their triage against the flow policy (spec 010), waivers, and the state
//! and validity derived from the changeset, the period, the evaluation and the waiver.
//!
//! This crate holds the pure domain and the module as the core sees it (ADR 0026), with
//! its migrations and its handler in the write pipeline (the references of the endpoints;
//! the operations come with task 011-3).

pub mod evaluation;
pub mod handler;
pub mod model;
pub mod module;
mod pg;
pub mod rules;
pub mod state;

pub use module::Flows;
