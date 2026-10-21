//! The business modules compiled in this build (ADR 0026): each enabled by a feature of
//! the server, registered here and checked before anything starts (REQ-008-1).

use std::sync::Arc;

use anyhow::Context as _;
use kartenn_service::module::{Module, ModuleRegistry};

/// The modules of the build, before checking.
fn compiled() -> Vec<Arc<dyn Module>> {
    Vec::new()
}

/// The routes of the modules of the build, mounted under `/v1` (ADR 0026, clause 9).
#[must_use]
pub fn routers() -> Vec<kartenn_rest::ModuleRouter> {
    Vec::new()
}

/// The registry of the modules of the build.
///
/// # Errors
///
/// When the modules cannot run together (a repeated or reserved name, a missing
/// dependency, a cycle, a collection served twice): the server must not start.
pub fn registry() -> anyhow::Result<Arc<ModuleRegistry>> {
    ModuleRegistry::new(compiled())
        .map(Arc::new)
        .context("checking the modules of the build")
}

#[cfg(test)]
mod tests {
    /// The reference contract of the repository is the contract of this build, its
    /// modules included (spec 006, REQ-006-34; spec 008, REQ-008-9): written by
    /// `kartenn-server openapi`.
    #[test]
    fn the_reference_contract_is_that_of_the_build() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(kartenn_rest::openapi::REFERENCE);
        let registry = super::registry().expect("the modules of the build");
        let produced = kartenn_rest::openapi::json_with(&registry, super::routers());
        let reference: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("the reference"))
                .expect("the reference is JSON");
        assert!(
            reference == produced,
            "the reference contract is stale: run `cargo run -p kartenn-server --all-features -- openapi`"
        );
    }

    /// `docs/spec/permissions.md` is the catalogue of this build: the core and its modules
    /// (spec 005, REQ-005-16; spec 008, REQ-008-8).
    #[test]
    fn the_published_catalogue_matches_the_code() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/spec/permissions.md"
        );
        let registry = super::registry().expect("the modules of the build");
        let generated = kartenn_core::authz::catalogue_markdown_with(&registry.permissions());
        if std::env::var_os("KARTENN_BLESS").is_some() {
            std::fs::write(path, &generated).expect("write the catalogue");
        }
        let published = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            published, generated,
            "docs/spec/permissions.md is stale: run `KARTENN_BLESS=1 cargo test -p kartenn-server modules`"
        );
    }
}
