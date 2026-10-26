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

    /// REQ-018-10: every code of the build — permission, rule, problem, kind of object,
    /// state of the core and of its modules — has a French and an English label.
    #[test]
    fn every_code_has_french_and_english_labels() {
        let registry = super::registry().expect("the modules of the build");
        let catalogue = kartenn_rest::labels::CodeCatalogue::of(&registry);
        let labelled = |space: &str, key: &str| {
            catalogue
                .labels()
                .iter()
                .any(|l| l.space == space && l.key == key && !l.fr.is_empty() && !l.en.is_empty())
        };
        let mut missing = Vec::new();
        let of_modules = registry.permissions();
        let permissions = kartenn_core::authz::PERMISSIONS
            .iter()
            .chain(of_modules.iter())
            .map(|p| p.name);
        missing.extend(
            permissions
                .filter(|p| !labelled("permission", p))
                .map(|p| format!("permission/{p}")),
        );
        let rules = kartenn_core::rules::all_rules()
            .chain(registry.modules().flat_map(|m| m.rules().iter().copied()))
            .map(|r| r.id);
        missing.extend(
            rules
                .filter(|r| !labelled("rule", r))
                .map(|r| format!("rule/{r}")),
        );
        missing.extend(
            kartenn_rest::problem::ProblemKind::ALL
                .iter()
                .filter(|k| !labelled("problem", k.slug()))
                .map(|k| format!("problem/{}", k.slug())),
        );
        missing.extend(
            registry
                .object_types()
                .map(|t| t.kind)
                .chain(["core.prefix", "core.address", "core.entity", "core.site"])
                .filter(|k| !labelled("kind", k))
                .map(|k| format!("kind/{k}")),
        );
        missing.extend(
            [
                "changeset.pending",
                "import.running",
                "flow.active",
                "flow.noncompliant",
            ]
            .into_iter()
            .filter(|s| !labelled("state", s))
            .map(|s| format!("state/{s}")),
        );
        assert!(
            missing.is_empty(),
            "without French and English: {missing:?}"
        );
        let blank: Vec<String> = catalogue
            .labels()
            .iter()
            .filter(|l| l.fr.is_empty() || l.en.is_empty())
            .map(|l| format!("{}/{}", l.space, l.key))
            .collect();
        assert!(blank.is_empty(), "a label missing: {blank:?}");
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
            "machines.edit",
            "vlan.read",
            "vlan.approval.self",
            "flow.apply",
            "flow.waiver.grant",
        ] {
            assert!(catalogue.iter().any(|p| p.name == name), "{name}");
        }
    }

    /// The retired names of the build and of its modules, with what replaces each (spec
    /// 018, R3; spec 024, REQ-024-9).
    fn retired_names() -> Vec<(String, Vec<String>)> {
        let registry = super::registry().expect("the modules of the build");
        let catalogue: Vec<_> = kartenn_core::authz::PERMISSIONS
            .iter()
            .copied()
            .chain(registry.permissions())
            .collect();
        kartenn_core::authz::retired_names(&catalogue)
    }

    /// R3, REQ-024-9: each old name is out of the catalogue of the build, each new one in
    /// it (or a pattern of the properties).
    #[test]
    fn renamed_permissions_target_the_catalogue() {
        for (old, new) in retired_names() {
            assert!(kartenn_core::authz::permission(&old).is_none(), "{old}");
            for new in new {
                assert!(
                    kartenn_core::authz::permission(&new).is_some() || new.starts_with("prop.*."),
                    "{old}: {new}"
                );
            }
        }
        let olds: Vec<String> = retired_names().into_iter().map(|(old, _)| old).collect();
        for old in [
            "vlan.vlan.write",
            "machines.propose",
            "flow.write",
            "core.changeset.approve",
        ] {
            assert!(olds.iter().any(|o| o == old), "{old}");
        }
    }

    /// R4, REQ-018-2: the types named after their module read and write under the short
    /// names; the reader of the build reads them.
    #[test]
    fn a_module_kind_reads_under_the_short_permission() {
        use kartenn_core::authz::{READER, type_edit_permission, type_read_permission};
        let registry = super::registry().expect("the modules of the build");
        assert_eq!(type_read_permission("machines.machine"), "machines.read");
        assert_eq!(type_edit_permission("machines.machine"), "machines.edit");
        assert_eq!(type_read_permission("vlan.vlan"), "vlan.read");
        assert_eq!(type_read_permission("flow.flow"), "flow.read");
        let reader = READER.entries_with(&registry.permissions());
        for name in ["flow.read", "vlan.read", "machines.read"] {
            assert!(reader.contains(&name), "{name}");
        }
    }

    /// REQ-018-6, REQ-024-9: no old permission name is left in the repository, outside the
    /// renaming table, the migrations, the migration of the custom roles and their tests,
    /// and the decision logs.
    #[test]
    fn no_old_permission_name_is_left() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        // The table and its tests; the catalogue, which lists the old names; the tests of
        // the migration of the custom roles, which start from them.
        let exceptions = [
            "crates/kartenn-core/src/authz.rs",
            "docs/spec/permissions.md",
            "crates/kartenn-service/tests/it/rbac.rs",
            "crates/kartenn-server/src/modules.rs",
            // The test of migration 0063, which rewrites old names.
            "crates/kartenn-store-pg/tests/it/changesets.rs",
        ];
        let olds = retired_names();
        let mut found = Vec::new();
        for dir in OLD_NAME_FREE {
            visit(&root.join(dir), &mut |path, text| {
                let relative = path.strip_prefix(root).unwrap_or(path);
                if exceptions
                    .iter()
                    .any(|e| relative == std::path::Path::new(e))
                    || relative.starts_with("crates/kartenn-store-pg/migrations")
                    // Task 024-11: the shell v2 takes `approval.self` in its own change;
                    // remove this exception with it.
                    || relative.starts_with("web/v2")
                {
                    return;
                }
                for (old, _) in &olds {
                    if text.contains(old.as_str()) {
                        found.push(format!("{}: {old}", relative.display()));
                    }
                }
            });
        }
        assert_eq!(found, Vec::<String>::new());
    }

    /// The trees held free of the old permission names (REQ-018-6); the decision logs and
    /// the ADR keep them as history.
    const OLD_NAME_FREE: &[&str] = &["crates", "web", "clients", "docs/spec"];

    fn visit(dir: &std::path::Path, f: &mut dyn FnMut(&std::path::Path, &str)) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                // Dependencies, builds, caches and reports: not sources.
                let skipped = name.to_str().is_none_or(|n| {
                    n.starts_with('.')
                        || matches!(
                            n,
                            "target"
                                | "node_modules"
                                | "dist"
                                | "build"
                                | "coverage"
                                | "test-results"
                                | "playwright-report"
                                | "__pycache__"
                        )
                });
                if !skipped {
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
