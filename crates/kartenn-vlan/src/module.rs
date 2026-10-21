//! The VLAN module as the core sees it (ADR 0026): its types of objects, permissions,
//! rules, operations and migrations, and its handler in the write pipeline (feature
//! `pg`).

use std::sync::Arc;

use kartenn_core::authz::Permission;
use kartenn_core::rules::Rule;
use kartenn_service::module::{Module, ModuleHandler, ModuleMigration, ObjectType, OperationType};

use crate::rules::VLAN_RULES;

/// The qualified type of a group of VLAN.
pub const GROUP: &str = "vlan.group";
/// The qualified type of a VLAN.
pub const VLAN: &str = "vlan.vlan";

static TYPES: &[ObjectType] = &[
    // A group carries `core.owner` (required) and, anchored on a site, `core.site`; anchored
    // on a domain, it is in the scope and under the lock of the domain (V3).
    ObjectType {
        kind: GROUP,
        collection: "vlan-groups",
        parent: None,
        owner_required: true,
        carries_site: true,
        domain_anchored: true,
    },
    // A VLAN inherits the anchors of its group, along the tree of the module.
    ObjectType {
        kind: VLAN,
        collection: "vlans",
        parent: Some(GROUP),
        owner_required: false,
        carries_site: false,
        domain_anchored: true,
    },
];

static OPERATIONS: &[OperationType] = &[
    OperationType {
        name: "create_group",
        defines: Some(GROUP),
    },
    OperationType {
        name: "update_group",
        defines: None,
    },
    OperationType {
        name: "create_vlan",
        defines: Some(VLAN),
    },
    OperationType {
        name: "update_vlan",
        defines: None,
    },
];

static MIGRATIONS: &[ModuleMigration] = &[ModuleMigration {
    version: 1,
    name: "vlan",
    sql: include_str!("../migrations/0001_vlan.sql"),
}];

macro_rules! permissions {
    ($list:ident; $($name:ident = $id:literal, $summary:literal;)*) => {
        $(
            static $name: Permission = Permission {
                name: $id,
                summary: $summary,
                global_only: false,
            };
        )*
        static $list: &[&Permission] = &[$(&$name),*];
    };
}

permissions! {
    PERMISSIONS;
    GROUP_READ = "vlan.group.read", "Lire les groupes de VLAN.";
    GROUP_WRITE = "vlan.group.write", "Créer, modifier ou clore un groupe de VLAN.";
    GROUP_PROPOSE = "vlan.group.propose", "Proposer un changement d'un groupe de VLAN.";
    VLAN_READ = "vlan.vlan.read", "Lire les VLAN.";
    VLAN_WRITE = "vlan.vlan.write", "Créer, modifier ou clore un VLAN.";
    VLAN_PROPOSE = "vlan.vlan.propose", "Proposer un changement d'un VLAN.";
}

/// The VLAN module (spec 008).
#[derive(Debug, Clone, Copy, Default)]
pub struct Vlan;

impl Module for Vlan {
    fn name(&self) -> &'static str {
        "vlan"
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn object_types(&self) -> &'static [ObjectType] {
        TYPES
    }
    fn permissions(&self) -> &'static [&'static Permission] {
        PERMISSIONS
    }
    fn rules(&self) -> &'static [&'static Rule] {
        VLAN_RULES
    }
    fn operations(&self) -> &'static [OperationType] {
        OPERATIONS
    }
    fn migrations(&self) -> &'static [ModuleMigration] {
        MIGRATIONS
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        #[cfg(feature = "pg")]
        {
            Some(Arc::new(crate::handler::Handler))
        }
        #[cfg(not(feature = "pg"))]
        {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use kartenn_service::module::ModuleRegistry;

    use super::*;

    #[test]
    fn the_module_registers() {
        let registry = ModuleRegistry::new(vec![Arc::new(Vlan)]).expect("a valid module");
        assert_eq!(registry.object_types().count(), 2);
        assert!(
            registry
                .permissions()
                .iter()
                .all(|p| p.name.starts_with("vlan."))
        );
    }
}
