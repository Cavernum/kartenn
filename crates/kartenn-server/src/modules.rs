//! The business modules compiled in this build (ADR 0026): each enabled by a feature of
//! the server, registered here and checked before anything starts (REQ-008-1).

use std::sync::Arc;

use anyhow::Context as _;
use kartenn_service::module::{Module, ModuleRegistry};

/// The modules of the build, before checking.
fn compiled() -> Vec<Arc<dyn Module>> {
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
