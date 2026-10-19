//! Writes the OpenAPI contract of this build (spec 006, REQ-006-34) to the reference of
//! the repository, `specs/006-api-rest/openapi-v1.json`, or to the path given:
//! `cargo run -p kartenn-rest --bin openapi --all-features [path]`.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let path = std::env::args_os().nth(1).map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(kartenn_rest::openapi::REFERENCE),
        PathBuf::from,
    );
    let mut text =
        serde_json::to_string_pretty(&kartenn_rest::openapi::json()).expect("JSON serialises");
    text.push('\n');
    match std::fs::write(&path, text) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("openapi: writing {}: {e}", path.display());
            ExitCode::FAILURE
        }
    }
}
