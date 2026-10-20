//! An OIDC issuer for development and end-to-end tests (spec 007, REQ-007-35, W16): users
//! and groups read in TOML, a page to choose who logs in, errors on purpose. It listens on
//! the loopback only and is never shipped.

pub mod certs;
pub mod config;
pub mod server;
