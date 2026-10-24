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
mod declared;
mod diff_sections;
mod drafts;
mod errors;
mod events;
mod filter;
mod imports;
mod labels;
mod modules;
mod objects;
mod on_behalf;
mod on_behalf_e2e;
mod plugins;
mod quota;
mod resources;
mod rights;
mod sessions;
mod simulated;
mod socle;
mod suggestions;
mod transport;
mod web;
