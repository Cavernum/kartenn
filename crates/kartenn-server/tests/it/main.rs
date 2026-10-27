//! The integration tests of kartenn-server, one binary for the crate (ADR 0027): a
//! module per subject. `tests/http.rs` stays a binary of its own: it sends SIGHUP to its
//! own process, which would reach every test of a shared binary.

mod database_file;
mod dev_idp;
mod jobs;
mod overrides;
mod pool;
mod rbac_report;
mod run;
mod secrets;
mod trace;
mod version;

/// The `kartenn-server` binary, without the `KARTENN_*` variables of the caller: the
/// configuration reads every one of them (`config::load`), so a variable of the tooling
/// such as `KARTENN_BLESS` would be refused as an unknown setting.
fn server() -> std::process::Command {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kartenn-server"));
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("KARTENN_") {
            command.env_remove(name);
        }
    }
    command
}
