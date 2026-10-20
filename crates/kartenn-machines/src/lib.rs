//! Machines and interfaces module of kartenn (spec 009): the pure domain. Host names
//! (RFC 1123 §2.1), EUI-48 addresses (RFC 9542), the hosting rules of a machine (009/M5)
//! and the 802.1Q mode of an interface (009/M9), with the rules they enforce.
//!
//! Storage, the module handler and the REST routes come with the module contract
//! (ADR 0026, spec 008) and live behind features of this crate.

pub mod hosting;
pub mod mac;
pub mod name;
pub mod rules;
pub mod vlan_mode;

pub use hosting::{Host, MachineKind};
pub use mac::Mac;
pub use name::HostName;
pub use vlan_mode::VlanMode;
