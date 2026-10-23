//! Machines and interfaces module of kartenn (spec 009): host names (RFC 1123 §2.1),
//! EUI-48 addresses (RFC 9542), the hosting rules of a machine (009/M5) and the 802.1Q
//! mode of an interface (009/M9), with the rules they enforce; the module as the core
//! sees it (ADR 0026), and, with the feature `pg`, its handler in the write pipeline, its
//! storage in PostgreSQL and its port of reading for the modules that depend on it.

pub mod hosting;
mod labels;
pub mod mac;
pub mod module;
pub mod name;
pub mod rules;
pub mod vlan_mode;

#[cfg(feature = "pg")]
pub mod handler;
#[cfg(feature = "pg")]
mod pg;
#[cfg(feature = "pg")]
pub mod read;
#[cfg(feature = "rest")]
pub mod rest;

pub use hosting::{Host, MachineKind};
pub use mac::Mac;
pub use module::Machines;
pub use name::HostName;
pub use vlan_mode::VlanMode;
