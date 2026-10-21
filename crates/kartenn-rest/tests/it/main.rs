//! The integration tests of kartenn-rest, one binary for the crate (ADR 0027): a module
//! per subject; `common` (authenticator of the tests), `wire` (the router on a real port)
//! and `support` are shared.

mod common;
mod support;
mod wire;

mod audit;
mod authn;
mod changesets;
mod compression;
mod contract;
mod csrf;
mod cursor;
mod cycle;
mod dated;
mod diff_sections;
mod errors;
mod filter;
mod imports;
mod modules;
mod objects;
mod on_behalf;
mod plugins;
mod quota;
mod resources;
mod rights;
mod sessions;
mod socle;
mod transport;
mod web;
