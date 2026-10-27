//! The VLANs of the core (spec 026; spec 008, parts B to F): their rules through the write
//! pipeline, their routes, their imports, and the migration of a base of the module `vlan`.

mod env;

mod groups;
mod imports;
mod migration;
mod networks;
mod references;
mod rest;
mod vlans;
