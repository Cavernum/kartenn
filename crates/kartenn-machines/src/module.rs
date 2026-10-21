//! The machines module as the core sees it (ADR 0026): its types of objects, permissions,
//! rules, operations and migrations, and its handler in the write pipeline (feature
//! `pg`).

use std::sync::Arc;

use kartenn_core::authz::Permission;
use kartenn_core::rules::Rule;
use kartenn_service::module::{Module, ModuleHandler, ModuleMigration, ObjectType, OperationType};

use crate::rules::MACHINES_RULES;

/// The qualified type of a cluster.
pub const CLUSTER: &str = "machines.cluster";
/// The qualified type of a machine.
pub const MACHINE: &str = "machines.machine";
/// The qualified type of an interface.
pub const INTERFACE: &str = "machines.interface";

static TYPES: &[ObjectType] = &[
    // A cluster carries `core.owner` (required) and, optionally, `core.site`.
    ObjectType {
        kind: CLUSTER,
        collection: "clusters",
        parent: None,
        owner_required: true,
        carries_site: true,
        domain_anchored: false,
    },
    // A machine too; it is the root of the tree of the module (its interfaces).
    ObjectType {
        kind: MACHINE,
        collection: "machines",
        parent: None,
        owner_required: true,
        carries_site: true,
        domain_anchored: false,
    },
    // An interface is tied to its machine, whose owner and site it inherits (REQ-009-7).
    ObjectType {
        kind: INTERFACE,
        collection: "interfaces",
        parent: Some(MACHINE),
        owner_required: false,
        carries_site: false,
        domain_anchored: false,
    },
];

static OPERATIONS: &[OperationType] = &[
    OperationType {
        name: "create_cluster",
        defines: Some(CLUSTER),
    },
    OperationType {
        name: "update_cluster",
        defines: None,
    },
    OperationType {
        name: "create_machine",
        defines: Some(MACHINE),
    },
    OperationType {
        name: "update_machine",
        defines: None,
    },
    OperationType {
        name: "create_interface",
        defines: Some(INTERFACE),
    },
    OperationType {
        name: "update_interface",
        defines: None,
    },
    OperationType {
        name: "attach_address",
        defines: None,
    },
    OperationType {
        name: "detach_address",
        defines: None,
    },
];

static MIGRATIONS: &[ModuleMigration] = &[
    ModuleMigration {
        version: 1,
        name: "machines",
        sql: include_str!("../migrations/0001_machines.sql"),
    },
    ModuleMigration {
        version: 2,
        name: "interface",
        sql: include_str!("../migrations/0002_interface.sql"),
    },
    ModuleMigration {
        version: 3,
        name: "attachment",
        sql: include_str!("../migrations/0003_attachment.sql"),
    },
];

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
    CLUSTER_READ = "machines.cluster.read", "Lire les clusters.";
    CLUSTER_WRITE = "machines.cluster.write", "Créer, modifier ou clore un cluster.";
    CLUSTER_PROPOSE = "machines.cluster.propose", "Proposer un changement d'un cluster.";
    MACHINE_READ = "machines.machine.read", "Lire les machines.";
    MACHINE_WRITE = "machines.machine.write", "Créer, modifier ou clore une machine.";
    MACHINE_PROPOSE = "machines.machine.propose", "Proposer un changement d'une machine.";
    INTERFACE_READ = "machines.interface.read", "Lire les interfaces.";
    INTERFACE_WRITE = "machines.interface.write", "Créer, modifier ou clore une interface.";
    INTERFACE_PROPOSE = "machines.interface.propose", "Proposer un changement d'une interface.";
}

/// The machines and interfaces module (spec 009).
#[derive(Debug, Clone, Copy, Default)]
pub struct Machines;

impl Module for Machines {
    fn name(&self) -> &'static str {
        "machines"
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn depends_on(&self) -> &'static [&'static str] {
        &["vlan"]
    }
    fn object_types(&self) -> &'static [ObjectType] {
        TYPES
    }
    fn permissions(&self) -> &'static [&'static Permission] {
        PERMISSIONS
    }
    fn rules(&self) -> &'static [&'static Rule] {
        MACHINES_RULES
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
