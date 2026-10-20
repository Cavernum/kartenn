//! VLAN module of kartenn: IEEE 802.1Q VLANs grouped in VLAN groups (spec 008).
//!
//! This crate holds the pure domain (identifiers, names, VID ranges, occupancy and
//! rules); the handler, the storage adapter and the REST routes plug into the module
//! contract (ADR 0026).

pub mod group;
pub mod rules;
pub mod vlan;
