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
