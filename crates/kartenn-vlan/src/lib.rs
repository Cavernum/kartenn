//! VLAN module of kartenn: IEEE 802.1Q VLANs grouped in VLAN groups (spec 008).
//!
//! This crate holds the pure domain (identifiers, names, VID ranges, occupancy and
//! rules), the module as the core sees it (ADR 0026), and, with the feature `pg`, its
//! handler in the write pipeline and its storage in PostgreSQL.

pub mod group;
mod labels;
pub mod module;
pub mod rules;
pub mod vlan;

#[cfg(feature = "pg")]
pub mod handler;
#[cfg(feature = "pg")]
mod pg;
#[cfg(feature = "rest")]
pub mod rest;

pub use module::Vlan;
