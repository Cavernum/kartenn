//! Flow policy module of kartenn (spec 010): classification axes, catalogue of services,
//! rules by attributes and patterns, verdicts with their outcome and explanation,
//! policy differential and inference of expected classifications.
//!
//! This crate holds the pure domain, without storage; the module as the core sees it
//! (ADR 0026) and its storage come with the tasks of phase P2.

pub mod model;
pub mod rules;
