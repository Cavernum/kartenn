//! The integration tests of kartenn-server, one binary for the crate (ADR 0027): a
//! module per subject. `tests/http.rs` stays a binary of its own: it sends SIGHUP to its
//! own process, which would reach every test of a shared binary.

mod dev_idp;
mod jobs;
mod overrides;
mod pool;
mod run;
mod secrets;
mod trace;
mod version;
