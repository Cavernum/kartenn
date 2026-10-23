//! The business modules compiled in this build (ADR 0026): each enabled by a feature of
//! the server, registered here and checked before anything starts (REQ-008-1).

use std::sync::Arc;

use anyhow::Context as _;
use kartenn_service::module::{Module, ModuleRegistry};

/// The modules of the build, before checking.
fn compiled() -> Vec<Arc<dyn Module>> {
    let modules: Vec<Arc<dyn Module>> = vec![
        #[cfg(feature = "vlan")]
        Arc::new(kartenn_vlan::Vlan),
        #[cfg(feature = "machines")]
        Arc::new(kartenn_machines::Machines),
        #[cfg(feature = "policy")]
        Arc::new(kartenn_policy::Policy),
        #[cfg(feature = "flow")]
        Arc::new(kartenn_flow::Flows),
    ];
    modules
}

/// The routes of the modules of the build, mounted under `/v1` (ADR 0026, clause 9).
#[must_use]
pub fn routers() -> Vec<kartenn_rest::ModuleRouter> {
    let routers: Vec<kartenn_rest::ModuleRouter> = vec![
        #[cfg(feature = "vlan")]
        kartenn_vlan::rest::routes(),
        #[cfg(feature = "machines")]
        kartenn_machines::rest::routes(),
        #[cfg(feature = "policy")]
        kartenn_policy::rest::routes(),
        #[cfg(feature = "flow")]
        kartenn_flow::rest::routes(),
    ];
    routers
}

/// The sources of expected values the modules of the build provide (REQ-003-21): the
/// inference of the policy (spec 010, REQ-010-50 to 52).
#[must_use]
pub fn sources() -> kartenn_service::expected::Sources {
    let sources = kartenn_service::expected::Sources::none();
    #[cfg(feature = "policy")]
    let sources = sources.with(Arc::new(kartenn_policy::infer::InferenceSource));
    sources
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

    /// REQ-018-1: the catalogue of the build follows the naming rule (`flow.read`, the
    /// module never repeated as the resource), no resource named after an action.
    #[test]
    fn permission_names_omit_the_module_resource() {
        let registry = super::registry().expect("the modules of the build");
        let catalogue: Vec<_> = kartenn_core::authz::PERMISSIONS
            .iter()
            .copied()
            .chain(registry.permissions())
            .collect();
        assert_eq!(
            kartenn_core::authz::catalogue_problems(&catalogue),
            Vec::<String>::new()
        );
        for name in [
            "flow.read",
            "machines.write",
            "vlan.read",
            "flow.waiver.grant",
        ] {
            assert!(catalogue.iter().any(|p| p.name == name), "{name}");
        }
    }

    /// R3: each old name is out of the catalogue of the build, each new one in it.
    #[test]
    fn renamed_permissions_target_the_catalogue() {
        let _registry = super::registry().expect("the modules of the build");
        for (old, new) in kartenn_core::authz::RENAMED_PERMISSIONS {
            assert!(kartenn_core::authz::permission(old).is_none(), "{old}");
            assert!(kartenn_core::authz::permission(new).is_some(), "{new}");
        }
    }

    /// R4, REQ-018-2: the types named after their module read and write under the short
    /// names; the reader of the build reads them.
    #[test]
    fn a_module_kind_reads_under_the_short_permission() {
        use kartenn_core::authz::{READER, type_read_permission, type_write_permission};
        let registry = super::registry().expect("the modules of the build");
        assert_eq!(type_read_permission("machines.machine"), "machines.read");
        assert_eq!(type_write_permission("machines.machine"), "machines.write");
        assert_eq!(type_read_permission("vlan.vlan"), "vlan.read");
        assert_eq!(type_read_permission("flow.flow"), "flow.read");
        let reader = READER.entries_with(&registry.permissions());
        for name in ["flow.read", "vlan.read", "machines.read"] {
            assert!(reader.contains(&name), "{name}");
        }
    }

    /// REQ-018-6: no old permission name is left in the repository, outside the renaming
    /// table, the migration of the custom roles and their tests, and the decision logs.
    #[test]
    fn no_old_permission_name_is_left() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let exceptions = ["crates/kartenn-core/src/authz.rs"];
        let mut found = Vec::new();
        for dir in OLD_NAME_FREE {
            visit(&root.join(dir), &mut |path, text| {
                let relative = path.strip_prefix(root).unwrap_or(path);
                if exceptions
                    .iter()
                    .any(|e| relative == std::path::Path::new(e))
                {
                    return;
                }
                for (old, _) in kartenn_core::authz::RENAMED_PERMISSIONS {
                    if text.contains(old) {
                        found.push(format!("{}: {old}", relative.display()));
                    }
                }
            });
        }
        assert_eq!(found, Vec::<String>::new());
    }

    /// The trees held free of the old permission names.
    const OLD_NAME_FREE: &[&str] = &["crates"];

    fn visit(dir: &std::path::Path, f: &mut dyn FnMut(&std::path::Path, &str)) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                if !matches!(
                    name.to_str(),
                    Some("target" | "node_modules" | "dist" | ".venv" | "__pycache__")
                ) {
                    visit(&path, f);
                }
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                f(&path, &text);
            }
        }
    }

    /// `docs/spec/regles-modules.md` is the catalogue of the rules of the modules of this
    /// build (spec 008, REQ-008-8; spec 009, REQ-009-13).
    #[test]
    fn the_published_module_rules_match_the_code() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/spec/regles-modules.md"
        );
        let registry = super::registry().expect("the modules of the build");
        let generated = kartenn_core::rules::modules_catalogue_markdown(
            registry.modules().map(|m| (m.name(), m.rules())),
        );
        if std::env::var_os("KARTENN_BLESS").is_some() {
            std::fs::write(path, &generated).expect("write the catalogue");
        }
        let published = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            published, generated,
            "docs/spec/regles-modules.md is stale: run `KARTENN_BLESS=1 cargo test -p kartenn-server modules`"
        );
    }
}
