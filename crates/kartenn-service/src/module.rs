//! The module contract (ADR 0026; spec 008, REQ-008-1): what a compiled business module
//! declares to the core, and the registry that checks the modules of a build at startup
//! and orders them along their dependencies.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, LazyLock};

use kartenn_core::authz::Permission;
use kartenn_core::rules::Rule;

use crate::declarations::ModuleDeclarations;

/// The name reserved for the core (ADR 0026, clause 1).
pub const CORE: &str = "core";

/// The collections the core serves under `/v1`: no module may declare one of them
/// (REQ-008-1). A test of `kartenn-rest` checks this list against its contract.
pub const CORE_COLLECTIONS: &[&str] = &[
    "addresses",
    "anomalies",
    "audit-events",
    "auth",
    "bindings",
    "capabilities",
    "changesets",
    "domains",
    "entities",
    "identities",
    "imports",
    "issuer-attachments",
    "log-filters",
    "me",
    "openapi.json",
    "prefixes",
    "property-declarations",
    "property-gaps",
    "proposals",
    "ranges",
    "roles",
    "service-accounts",
    "sites",
];

/// A type of object of a module (ADR 0026, clause 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectType {
    /// Qualified name, `<module>.<type>` (`vlan.vlan`).
    pub kind: &'static str,
    /// Its REST collection under `/v1` (`vlans`).
    pub collection: &'static str,
    /// The type of its parent in the tree of the module, if any.
    pub parent: Option<&'static str>,
    /// Whether `core.owner` must be set on it (otherwise inherited or optional).
    pub owner_required: bool,
    /// Whether it carries `core.site`.
    pub carries_site: bool,
    /// Whether it is anchored in an addressing domain (its writes lock that domain).
    pub domain_anchored: bool,
}

/// What closing the target of a reference does to its sources (D33, ADR 0026 clause 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnClose {
    /// Refused while a source lives.
    Refuse,
    /// Closes the sources with the target, when the changeset asks for the cascade.
    Cascade,
    /// Clears the reference in the sources.
    Nullify,
}

/// A reference from objects of a module to objects of the core or of a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The type of the source, qualified.
    pub from: &'static str,
    /// The type of the target: a core kind (`prefix`) or a qualified module type.
    pub to: &'static str,
    /// What closing the target does.
    pub on_close: OnClose,
}

/// A type of edge the module contributes to the topology graph (ADR 0026, clause 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeType {
    /// Qualified name (`vlan.carries`).
    pub name: &'static str,
    /// The type of its origin.
    pub from: &'static str,
    /// The type of its end.
    pub to: &'static str,
}

/// A SQL migration of a module, applied once in the schema of the module (REQ-008-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleMigration {
    /// Its version, strictly increasing within the module.
    pub version: i64,
    /// A short name.
    pub name: &'static str,
    /// The SQL, usually `include_str!`.
    pub sql: &'static str,
}

/// The behaviour of a module inside the write pipeline. Its methods are wired step by
/// step (task 008-3); a module without operations keeps the defaults.
pub trait ModuleHandler: Send + Sync + fmt::Debug {}

/// A business module compiled in the build (ADR 0026).
pub trait Module: Send + Sync + fmt::Debug + 'static {
    /// Its name, `[a-z][a-z0-9_]*`, the namespace of everything it declares.
    fn name(&self) -> &'static str;
    /// The version of its crate.
    fn version(&self) -> &'static str;
    /// The modules it needs.
    fn depends_on(&self) -> &'static [&'static str] {
        &[]
    }
    /// Its types of objects.
    fn object_types(&self) -> &'static [ObjectType] {
        &[]
    }
    /// Its property declarations (003/P12).
    fn declarations(&self) -> Option<ModuleDeclarations> {
        None
    }
    /// Its permissions.
    fn permissions(&self) -> &'static [&'static Permission] {
        &[]
    }
    /// Its validation rules.
    fn rules(&self) -> &'static [&'static Rule] {
        &[]
    }
    /// Its references.
    fn references(&self) -> &'static [Reference] {
        &[]
    }
    /// Its edges of the topology graph.
    fn edges(&self) -> &'static [EdgeType] {
        &[]
    }
    /// Its migrations, in version order.
    fn migrations(&self) -> &'static [ModuleMigration] {
        &[]
    }
    /// Its handler in the write pipeline.
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        None
    }
}

/// Why the modules of a build cannot run together: the server stops (REQ-008-1).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RegistryError {
    /// A name outside `[a-z][a-z0-9_]*`.
    #[error("module `{0}`: invalid name, expected [a-z][a-z0-9_]*")]
    InvalidName(String),
    /// The name of the core.
    #[error("module `core`: the name is reserved for the core")]
    ReservedName,
    /// Two modules of the same name.
    #[error("module `{0}` is registered twice")]
    Duplicate(String),
    /// A dependency that is not in the build.
    #[error("module `{module}` depends on `{dependency}`, which is not in the build")]
    MissingDependency {
        /// The module.
        module: String,
        /// Its missing dependency.
        dependency: String,
    },
    /// Dependencies that loop.
    #[error("modules {0:?} depend on each other in a cycle")]
    Cycle(Vec<String>),
    /// A type of object outside the namespace of its module.
    #[error("module `{module}`: type `{kind}` must be named `{module}.<type>`")]
    ForeignKind {
        /// The module.
        module: String,
        /// The type.
        kind: String,
    },
    /// A REST collection declared twice, or taken from the core.
    #[error("collection `/v1/{collection}` of module `{module}` is already served by `{owner}`")]
    DuplicateCollection {
        /// The collection.
        collection: String,
        /// The module that declares it last.
        module: String,
        /// The module, or `core`, that has it.
        owner: String,
    },
}

/// The modules of the build, checked and in dependency order (ADR 0026, clause 2).
#[derive(Debug, Clone, Default)]
pub struct ModuleRegistry {
    modules: Vec<Arc<dyn Module>>,
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl ModuleRegistry {
    /// Checks `modules` and orders them so that each comes after its dependencies; ties
    /// keep the order given.
    ///
    /// # Errors
    ///
    /// [`RegistryError`] for an invalid, reserved or repeated name, a missing dependency,
    /// a cycle, a type outside the namespace of its module or a collection served twice.
    pub fn new(modules: Vec<Arc<dyn Module>>) -> Result<Self, RegistryError> {
        let mut by_name: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, module) in modules.iter().enumerate() {
            let name = module.name();
            if name == CORE {
                return Err(RegistryError::ReservedName);
            }
            if !valid_name(name) {
                return Err(RegistryError::InvalidName(name.to_owned()));
            }
            if by_name.insert(name, i).is_some() {
                return Err(RegistryError::Duplicate(name.to_owned()));
            }
        }
        let mut collections: BTreeMap<&str, &str> =
            CORE_COLLECTIONS.iter().map(|c| (*c, CORE)).collect();
        for module in &modules {
            for dependency in module.depends_on() {
                if !by_name.contains_key(dependency) {
                    return Err(RegistryError::MissingDependency {
                        module: module.name().to_owned(),
                        dependency: (*dependency).to_owned(),
                    });
                }
            }
            for ty in module.object_types() {
                if ty
                    .kind
                    .split_once('.')
                    .is_none_or(|(m, t)| m != module.name() || t.is_empty())
                {
                    return Err(RegistryError::ForeignKind {
                        module: module.name().to_owned(),
                        kind: ty.kind.to_owned(),
                    });
                }
                if let Some(owner) = collections.insert(ty.collection, module.name()) {
                    return Err(RegistryError::DuplicateCollection {
                        collection: ty.collection.to_owned(),
                        module: module.name().to_owned(),
                        owner: owner.to_owned(),
                    });
                }
            }
        }
        // Kahn: repeatedly take the first module whose dependencies are all placed.
        let mut placed: BTreeSet<&str> = BTreeSet::new();
        let mut order = Vec::with_capacity(modules.len());
        let mut rest = modules;
        while !rest.is_empty() {
            let Some(next) = rest
                .iter()
                .position(|m| m.depends_on().iter().all(|d| placed.contains(d)))
            else {
                let mut cycle: Vec<String> = rest.iter().map(|m| m.name().to_owned()).collect();
                cycle.sort();
                return Err(RegistryError::Cycle(cycle));
            };
            let module = rest.remove(next);
            placed.insert(module.name());
            order.push(module);
        }
        Ok(Self { modules: order })
    }

    /// A registry without modules, the build of the core alone.
    #[must_use]
    pub fn empty() -> &'static Self {
        static EMPTY: LazyLock<ModuleRegistry> = LazyLock::new(ModuleRegistry::default);
        &EMPTY
    }

    /// The modules, each after its dependencies.
    pub fn modules(&self) -> impl Iterator<Item = &Arc<dyn Module>> {
        self.modules.iter()
    }

    /// The module of this name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Module>> {
        self.modules.iter().find(|m| m.name() == name)
    }

    /// Whether there is no module.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Fake {
        name: &'static str,
        depends_on: &'static [&'static str],
        types: &'static [ObjectType],
    }

    impl Module for Fake {
        fn name(&self) -> &'static str {
            self.name
        }
        fn version(&self) -> &'static str {
            "0.0.0"
        }
        fn depends_on(&self) -> &'static [&'static str] {
            self.depends_on
        }
        fn object_types(&self) -> &'static [ObjectType] {
            self.types
        }
    }

    fn fake(name: &'static str, depends_on: &'static [&'static str]) -> Arc<dyn Module> {
        Arc::new(Fake {
            name,
            depends_on,
            types: &[],
        })
    }

    const fn ty(kind: &'static str, collection: &'static str) -> ObjectType {
        ObjectType {
            kind,
            collection,
            parent: None,
            owner_required: false,
            carries_site: false,
            domain_anchored: false,
        }
    }

    fn names(registry: &ModuleRegistry) -> Vec<&'static str> {
        registry.modules().map(|m| m.name()).collect()
    }

    #[test]
    fn registry_orders_dependencies_first() {
        let registry = ModuleRegistry::new(vec![
            fake("flow", &["policy"]),
            fake("policy", &[]),
            fake("vlan", &[]),
        ])
        .unwrap();
        assert_eq!(names(&registry), ["policy", "flow", "vlan"]);
        assert!(registry.get("flow").is_some());
        assert!(ModuleRegistry::empty().is_empty());
    }

    #[test]
    fn registry_refuses_duplicate() {
        assert_eq!(
            ModuleRegistry::new(vec![fake("vlan", &[]), fake("vlan", &[])]).unwrap_err(),
            RegistryError::Duplicate("vlan".into())
        );
    }

    #[test]
    fn registry_refuses_core_name() {
        assert_eq!(
            ModuleRegistry::new(vec![fake("core", &[])]).unwrap_err(),
            RegistryError::ReservedName
        );
        for wrong in ["Vlan", "1vlan", "vlan-x", ""] {
            assert_eq!(
                ModuleRegistry::new(vec![fake(wrong, &[])]).unwrap_err(),
                RegistryError::InvalidName(wrong.into())
            );
        }
    }

    #[test]
    fn registry_refuses_missing_dependency() {
        let error = ModuleRegistry::new(vec![fake("machines", &["vlan"])]).unwrap_err();
        assert_eq!(
            error,
            RegistryError::MissingDependency {
                module: "machines".into(),
                dependency: "vlan".into()
            }
        );
        let message = error.to_string();
        assert!(
            message.contains("machines") && message.contains("vlan"),
            "{message}"
        );
    }

    #[test]
    fn registry_refuses_cycle() {
        assert_eq!(
            ModuleRegistry::new(vec![fake("a", &["b"]), fake("b", &["a"]), fake("c", &[])])
                .unwrap_err(),
            RegistryError::Cycle(vec!["a".into(), "b".into()])
        );
    }

    #[test]
    fn registry_refuses_duplicate_collection() {
        static VLANS: &[ObjectType] = &[ty("vlan.vlan", "vlans")];
        static ALSO_VLANS: &[ObjectType] = &[ty("other.vlan", "vlans")];
        static PREFIXES: &[ObjectType] = &[ty("other.prefix", "prefixes")];
        let module = |name, types| -> Arc<dyn Module> {
            Arc::new(Fake {
                name,
                depends_on: &[],
                types,
            })
        };
        assert_eq!(
            ModuleRegistry::new(vec![module("vlan", VLANS), module("other", ALSO_VLANS)])
                .unwrap_err(),
            RegistryError::DuplicateCollection {
                collection: "vlans".into(),
                module: "other".into(),
                owner: "vlan".into()
            }
        );
        assert_eq!(
            ModuleRegistry::new(vec![module("other", PREFIXES)]).unwrap_err(),
            RegistryError::DuplicateCollection {
                collection: "prefixes".into(),
                module: "other".into(),
                owner: "core".into()
            }
        );
        assert_eq!(
            ModuleRegistry::new(vec![module("other", VLANS)]).unwrap_err(),
            RegistryError::ForeignKind {
                module: "other".into(),
                kind: "vlan.vlan".into()
            }
        );
    }
}
