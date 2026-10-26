//! The machines module as the core sees it (ADR 0026): its types of objects, permissions,
//! rules, operations and migrations, and its handler in the write pipeline (feature
//! `pg`).

use std::collections::BTreeSet;
use std::sync::Arc;

use kartenn_core::authz::{Permission, TypePermissions};
use kartenn_core::labels::Compiled;
use kartenn_core::properties::{Declarant, Declaration, PropertyKey, TypeSpec};
use kartenn_core::rules::Rule;
use kartenn_core::values::Cardinality;
use kartenn_service::declarations::ModuleDeclarations;
use kartenn_service::module::{
    EdgeType, Module, ModuleHandler, ModuleMigration, ObjectType, OnClose, OperationType, Reference,
};

use crate::rules::MACHINES_RULES;

/// The qualified type of a cluster.
pub const CLUSTER: &str = "machines.cluster";
/// The qualified type of a machine.
pub const MACHINE: &str = "machines.machine";
/// The qualified type of an interface.
pub const INTERFACE: &str = "machines.interface";
/// The role of an interface (REQ-009-8).
pub const INTERFACE_ROLE: &str = "machines.interface_role";
/// The medium of an interface (REQ-009-26, M17).
pub const INTERFACE_MEDIA: &str = "machines.interface_media";
/// The FQDN of a machine.
pub const FQDN: &str = "machines.fqdn";
/// The roles of an interface the module ships; an administrator adds others.
pub const INTERFACE_ROLES: &[&str] = &[
    "administration",
    "backup",
    "monitoring",
    "storage",
    "cluster",
    "service",
];
/// The media of an interface the module ships (M17); an administrator adds others.
pub const INTERFACE_MEDIAS: &[&str] = &[
    "ethernet",
    "sfp",
    "wifi",
    "aggregate",
    "virtual",
    "loopback",
];

/// The French and English labels of the declarations and values of the module (spec
/// 018, R10): `<property>` or `<property>/<value>`.
const LABELS: &[(&str, &str, &str)] = &[
    (INTERFACE_ROLE, "Rôle de l'interface", "Interface role"),
    (FQDN, "Nom complet (FQDN)", "Fully qualified domain name"),
    (
        "machines.interface_role/administration",
        "Administration",
        "Administration",
    ),
    ("machines.interface_role/backup", "Sauvegarde", "Backup"),
    (
        "machines.interface_role/monitoring",
        "Supervision",
        "Monitoring",
    ),
    ("machines.interface_role/storage", "Stockage", "Storage"),
    ("machines.interface_role/cluster", "Grappe", "Cluster"),
    ("machines.interface_role/service", "Service", "Service"),
    (INTERFACE_MEDIA, "Média de l'interface", "Interface medium"),
    ("machines.interface_media/ethernet", "Ethernet", "Ethernet"),
    ("machines.interface_media/sfp", "SFP", "SFP"),
    ("machines.interface_media/wifi", "Wi-Fi", "Wi-Fi"),
    ("machines.interface_media/aggregate", "Agrégat", "Aggregate"),
    ("machines.interface_media/virtual", "Virtuelle", "Virtual"),
    ("machines.interface_media/loopback", "Bouclage", "Loopback"),
];

/// A property of the module carried by `carrier`, single-valued and not inherited.
fn declaration(key: &str, spec: TypeSpec, carrier: &str) -> Declaration {
    Declaration {
        key: PropertyKey::parse(key).expect("a valid key"),
        declarant: Declarant::Module("machines".into()),
        spec,
        cardinality: Cardinality::Single,
        carriers: BTreeSet::from([carrier.to_owned()]),
        inherit: None,
        obligations: BTreeSet::new(),
        expected: None,
    }
}

/// The type of the clusters, as its routes serve it.
#[cfg(feature = "rest")]
pub(crate) fn cluster_type() -> &'static ObjectType {
    &TYPES[0]
}

/// The type of the machines, as its routes serve it.
#[cfg(feature = "rest")]
pub(crate) fn machine_type() -> &'static ObjectType {
    &TYPES[1]
}

/// The type of the interfaces, as its routes serve it.
#[cfg(feature = "rest")]
pub(crate) fn interface_type() -> &'static ObjectType {
    &TYPES[2]
}

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
    ModuleMigration {
        version: 4,
        name: "interface_suggestions",
        sql: include_str!("../migrations/0004_interface_suggestions.sql"),
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
    CLUSTER_EDIT = "machines.cluster.edit", "Créer, modifier ou clore un cluster dans un changeset et le soumettre.";
    CLUSTER_APPLY = "machines.cluster.apply", "Appliquer ou rejeter le changeset d'un autre qui touche un cluster.";
    CLUSTER_APPROVAL_SELF = "machines.cluster.approval.self", "Appliquer son propre changeset qui touche un cluster.";
    MACHINE_READ = "machines.read", "Lire les machines.";
    MACHINE_EDIT = "machines.edit", "Créer, modifier ou clore une machine dans un changeset et le soumettre.";
    MACHINE_APPLY = "machines.apply", "Appliquer ou rejeter le changeset d'un autre qui touche une machine.";
    MACHINE_APPROVAL_SELF = "machines.approval.self", "Appliquer son propre changeset qui touche une machine.";
    INTERFACE_READ = "machines.interface.read", "Lire les interfaces.";
    INTERFACE_EDIT = "machines.interface.edit", "Créer, modifier ou clore une interface dans un changeset et le soumettre.";
    INTERFACE_APPLY = "machines.interface.apply", "Appliquer ou rejeter le changeset d'un autre qui touche une interface.";
    INTERFACE_APPROVAL_SELF = "machines.interface.approval.self", "Appliquer son propre changeset qui touche une interface.";
}

/// An address of the core, as a reference names it.
pub const ADDRESS: &str = "address";
/// A VLAN of the module `vlan`.
pub const VLAN: &str = "vlan.vlan";

/// What closing an object does to the objects of the module that cite it (REQ-009-14;
/// design « Références »).
static REFERENCES: &[Reference] = &[
    // An interface lives on its machine: closed with it.
    Reference {
        from: INTERFACE,
        to: MACHINE,
        on_close: OnClose::Cascade,
    },
    // The attachments of an interface, whose sources are the attached addresses: closing
    // the interface detaches them.
    Reference {
        from: INTERFACE,
        to: INTERFACE,
        on_close: OnClose::Nullify,
    },
    // An address of the core attached to an interface: detached.
    Reference {
        from: INTERFACE,
        to: ADDRESS,
        on_close: OnClose::Nullify,
    },
    // A VLAN an interface carries: taken off the interface.
    Reference {
        from: INTERFACE,
        to: VLAN,
        on_close: OnClose::Nullify,
    },
    // A host machine: no longer the host.
    Reference {
        from: MACHINE,
        to: MACHINE,
        on_close: OnClose::Nullify,
    },
    // A host or member cluster: no longer the host, nor the cluster of its members.
    Reference {
        from: MACHINE,
        to: CLUSTER,
        on_close: OnClose::Nullify,
    },
];

/// The edges of the topology graph (REQ-009-17). An [`EdgeType`] has one type of end: a
/// machine hosted on a cluster is `machines.hosted_in`, beside `machines.hosted_on` for a
/// host machine.
static EDGES: &[EdgeType] = &[
    EdgeType {
        name: "machines.has_address",
        from: INTERFACE,
        to: ADDRESS,
    },
    EdgeType {
        name: "machines.access",
        from: INTERFACE,
        to: VLAN,
    },
    EdgeType {
        name: "machines.tagged",
        from: INTERFACE,
        to: VLAN,
    },
    EdgeType {
        name: "machines.native",
        from: INTERFACE,
        to: VLAN,
    },
    EdgeType {
        name: "machines.hosted_on",
        from: MACHINE,
        to: MACHINE,
    },
    EdgeType {
        name: "machines.hosted_in",
        from: MACHINE,
        to: CLUSTER,
    },
    EdgeType {
        name: "machines.member_of",
        from: MACHINE,
        to: CLUSTER,
    },
];

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
    /// `machines.interface_role` (M10) and `machines.interface_media` (M17), closed lists
    /// an administrator extends, and `machines.fqdn`. Machines and interfaces carry any other property too (REQ-009-24).
    fn declarations(&self) -> Option<ModuleDeclarations> {
        Some(ModuleDeclarations {
            module: "machines".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            properties: vec![
                (
                    declaration(INTERFACE_ROLE, TypeSpec::ENUM, INTERFACE),
                    INTERFACE_ROLES.iter().map(|&v| v.to_owned()).collect(),
                ),
                (
                    declaration(INTERFACE_MEDIA, TypeSpec::ENUM, INTERFACE),
                    INTERFACE_MEDIAS.iter().map(|&v| v.to_owned()).collect(),
                ),
                (declaration(FQDN, TypeSpec::Fqdn, MACHINE), Vec::new()),
            ],
            labels: LABELS
                .iter()
                .map(|(key, fr, en)| ((*key).to_owned(), Compiled::new(fr, en)))
                .collect(),
        })
    }
    fn permissions(&self) -> &'static [&'static Permission] {
        PERMISSIONS
    }
    fn type_permissions(&self) -> &'static [TypePermissions] {
        // The machine is the resource of the module: `machines.<action>` (spec 018, R2).
        &[TypePermissions {
            kind: MACHINE,
            read: "machines.read",
            edit: "machines.edit",
        }]
    }
    fn rules(&self) -> &'static [&'static Rule] {
        MACHINES_RULES
    }
    fn labels(&self) -> Vec<kartenn_core::labels::CodeLabel> {
        kartenn_core::code_labels::of_module(PERMISSIONS, MACHINES_RULES, &crate::labels::ENGLISH)
    }
    fn operations(&self) -> &'static [OperationType] {
        OPERATIONS
    }
    fn references(&self) -> &'static [Reference] {
        REFERENCES
    }
    fn edges(&self) -> &'static [EdgeType] {
        EDGES
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
    fn stats(&self) -> Option<Arc<dyn kartenn_service::module::StatsContributor>> {
        #[cfg(feature = "rest")]
        {
            Some(Arc::new(crate::stats::Stats))
        }
        #[cfg(not(feature = "rest"))]
        {
            None
        }
    }
}
