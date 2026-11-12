//! The kube module as the core sees it (ADR 0026): its types of objects, permissions,
//! rules, operations, references and migrations, and its handler in the write pipeline
//! (feature `pg`). It depends on the machines module (REQ-019-4): a node designates a
//! machine of the 009.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use kartenn_core::authz::{Permission, TypePermissions};
use kartenn_core::labels::Compiled;
use kartenn_core::properties::{Declarant, Declaration, PropertyKey, TypeSpec};
use kartenn_core::rules::Rule;
use kartenn_core::values::Cardinality;
use kartenn_service::declarations::ModuleDeclarations;
use kartenn_service::module::{
    EdgeType, JobSpec, Module, ModuleHandler, ModuleMigration, ObjectType, OnClose, OperationType,
    Reference,
};

use crate::rules::KUBE_RULES;

/// The qualified type of a cluster.
pub const CLUSTER: &str = "kube.cluster";
/// The qualified type of a node.
pub const NODE: &str = "kube.node";
/// The qualified type of a namespace.
pub const NAMESPACE: &str = "kube.namespace";
/// The qualified type of a workload.
pub const WORKLOAD: &str = "kube.workload";
/// The qualified type of a service.
pub const SERVICE: &str = "kube.service";
/// The qualified type of an ingress.
pub const INGRESS: &str = "kube.ingress";
/// The qualified type of a mapping of a label to a value of an axis (Q-019-35).
pub const LABEL_MAPPING: &str = "kube.label_mapping";
/// The qualified type of the admitted values of an axis in a namespace (Q-019-35).
pub const ADMITTED_VALUES: &str = "kube.admitted_values";
/// The qualified type of a label key tracked in every cluster (Q-019-35, 019/X4).
pub const TRACKED_LABEL: &str = "kube.tracked_label";
/// A machine of the 009, as a reference names it.
pub const MACHINE: &str = "machines.machine";
/// An addressing domain of the core.
pub const DOMAIN: &str = "domain";
/// A prefix of the core: a container of pod ranges, a network of services.
pub const PREFIX: &str = "prefix";
/// An address of the core: a cluster IP or an external address of a service, an address
/// of an ingress.
pub const ADDRESS: &str = "address";
/// The name of the module: the namespace of its keys and of its provenances.
pub const MODULE: &str = "kube";
/// The axis of the namespace of a workload, an external reference of the system `kube`
/// that `kube.sync` alone sets (REQ-019-33; Q-019-37 to 39).
pub const NAMESPACE_AXIS: &str = "kube.namespace";
/// The external system of the namespaces, a value of `core.external_system` the module
/// declares (Q-019-38).
pub const SYSTEM: &str = "kube";

/// The declarations of the module (design « Propriétés »): the axis `kube.namespace`,
/// carried by the workloads, single-valued, not inherited, and the system `kube` it
/// names (ADR 0026, amendment 15).
fn module_declarations() -> ModuleDeclarations {
    let axis = Declaration {
        key: PropertyKey::parse(NAMESPACE_AXIS).expect("a valid key"),
        declarant: Declarant::Module(MODULE.into()),
        spec: TypeSpec::ExternalRef,
        cardinality: Cardinality::Single,
        carriers: BTreeSet::from([WORKLOAD.to_owned()]),
        inherit: None,
        obligations: BTreeSet::new(),
        expected: None,
    };
    ModuleDeclarations {
        module: MODULE.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        properties: vec![(axis, Vec::new())],
        labels: BTreeMap::from([(
            NAMESPACE_AXIS.to_owned(),
            Compiled::new("Namespace Kubernetes", "Kubernetes namespace"),
        )]),
        external_systems: BTreeMap::from([(
            SYSTEM.to_owned(),
            Compiled::new("Kubernetes", "Kubernetes"),
        )]),
    }
}

/// The types of the module so far (design « Types d'objets »): the cluster, root of the
/// tree `kube`, carries `core.owner` and `core.site`; its nodes and namespaces, and the
/// workloads, services and ingresses of its namespaces, inherit them and are anchored in
/// one of its domains (pod, service, external).
static TYPES: &[ObjectType] = &[
    ObjectType {
        kind: CLUSTER,
        collection: "kube-clusters",
        parent: None,
        owner_required: true,
        carries_site: true,
        domain_anchored: false,
    },
    ObjectType {
        kind: NODE,
        collection: "kube-nodes",
        parent: Some(CLUSTER),
        owner_required: false,
        carries_site: false,
        domain_anchored: true,
    },
    ObjectType {
        kind: NAMESPACE,
        collection: "kube-namespaces",
        parent: Some(CLUSTER),
        owner_required: false,
        carries_site: false,
        domain_anchored: true,
    },
    ObjectType {
        kind: WORKLOAD,
        collection: "workloads",
        parent: Some(NAMESPACE),
        owner_required: false,
        carries_site: false,
        domain_anchored: true,
    },
    ObjectType {
        kind: SERVICE,
        collection: "kube-services",
        parent: Some(NAMESPACE),
        owner_required: false,
        carries_site: false,
        domain_anchored: true,
    },
    ObjectType {
        kind: INGRESS,
        collection: "ingresses",
        parent: Some(NAMESPACE),
        owner_required: false,
        carries_site: false,
        domain_anchored: true,
    },
    global(LABEL_MAPPING, "kube-label-mappings"),
    global(ADMITTED_VALUES, "kube-admitted-values"),
    global(TRACKED_LABEL, "kube-tracked-labels"),
];

/// The type of the clusters, as its routes serve it.
#[cfg(feature = "rest")]
pub(crate) fn cluster_type() -> &'static ObjectType {
    &TYPES[0]
}

/// A setting of the module (Q-019-35): no anchor, no parent, written under the global
/// `kube.settings.edit`.
const fn global(kind: &'static str, collection: &'static str) -> ObjectType {
    ObjectType {
        kind,
        collection,
        parent: None,
        owner_required: false,
        carries_site: false,
        domain_anchored: false,
    }
}

/// The settings of the module are read with the clusters, which the connector reads, and
/// written under the settings of the module (spec 027, P4, P5; Q-019-35): no
/// `kube.settings.read` (019/X16).
static TYPE_PERMISSIONS: &[TypePermissions] = &[
    settings(LABEL_MAPPING),
    settings(ADMITTED_VALUES),
    settings(TRACKED_LABEL),
];

const fn settings(kind: &'static str) -> TypePermissions {
    TypePermissions {
        kind,
        read: CLUSTER_READ.name,
        edit: SETTINGS_EDIT.name,
    }
}

static OPERATIONS: &[OperationType] = &[
    OperationType {
        name: "create_cluster",
        defines: Some(CLUSTER),
    },
    OperationType {
        name: "update_cluster",
        defines: None,
    },
    // The batch of the connector: one object of a cluster per operation, created, changed
    // or closed by its identity in the cluster (019/X9).
    OperationType {
        name: "sync",
        defines: None,
    },
    // The settings of the module (Q-019-35): a mapping created, changed or closed; the
    // admitted values of an axis in a namespace, replaced; the tracked labels, replaced.
    OperationType {
        name: "set_label_mapping",
        defines: None,
    },
    OperationType {
        name: "set_admitted_values",
        defines: None,
    },
    OperationType {
        name: "set_tracked_labels",
        defines: None,
    },
];

static MIGRATIONS: &[ModuleMigration] = &[
    ModuleMigration {
        version: 1,
        name: "kube",
        sql: include_str!("../migrations/0001_kube.sql"),
    },
    ModuleMigration {
        version: 2,
        name: "node_pod_cidrs",
        sql: include_str!("../migrations/0002_node_pod_cidrs.sql"),
    },
    ModuleMigration {
        version: 3,
        name: "workload",
        sql: include_str!("../migrations/0003_workload.sql"),
    },
    ModuleMigration {
        version: 4,
        name: "service",
        sql: include_str!("../migrations/0004_service.sql"),
    },
    ModuleMigration {
        version: 5,
        name: "ingress",
        sql: include_str!("../migrations/0005_ingress.sql"),
    },
    ModuleMigration {
        version: 6,
        name: "observation",
        sql: include_str!("../migrations/0006_observation.sql"),
    },
    ModuleMigration {
        version: 7,
        name: "observation_retention",
        sql: include_str!("../migrations/0007_observation_retention.sql"),
    },
    ModuleMigration {
        version: 8,
        name: "settings",
        sql: include_str!("../migrations/0008_settings.sql"),
    },
];

/// Whether a permission marked `[global]` is: granted at the global scope only.
macro_rules! global_only {
    () => {
        false
    };
    (global) => {
        true
    };
}

macro_rules! permissions {
    ($list:ident; $($([$global:ident])? $name:ident = $id:literal, $summary:literal;)*) => {
        $(
            static $name: Permission = Permission {
                name: $id,
                summary: $summary,
                global_only: global_only!($($global)?),
            };
        )*
        static $list: &[&Permission] = &[$(&$name),*];
    };
}

// The grammar of the 024 (M7) for each type of the module so far (design « Droits, rôles
// et audit »).
permissions! {
    PERMISSIONS;
    CLUSTER_READ = "kube.cluster.read", "Lire les clusters Kubernetes.";
    CLUSTER_EDIT = "kube.cluster.edit", "Créer, modifier ou clore un cluster Kubernetes dans un changeset et le soumettre.";
    CLUSTER_APPLY = "kube.cluster.apply", "Appliquer ou rejeter le changeset d'un autre qui touche un cluster Kubernetes.";
    CLUSTER_APPROVAL_SELF = "kube.cluster.approval.self", "Appliquer son propre changeset qui touche un cluster Kubernetes.";
    NODE_READ = "kube.node.read", "Lire les nœuds Kubernetes.";
    NODE_EDIT = "kube.node.edit", "Créer, modifier ou clore un nœud Kubernetes dans un changeset et le soumettre.";
    NODE_APPLY = "kube.node.apply", "Appliquer ou rejeter le changeset d'un autre qui touche un nœud Kubernetes.";
    NODE_APPROVAL_SELF = "kube.node.approval.self", "Appliquer son propre changeset qui touche un nœud Kubernetes.";
    NAMESPACE_READ = "kube.namespace.read", "Lire les namespaces Kubernetes.";
    NAMESPACE_EDIT = "kube.namespace.edit", "Créer, modifier ou clore un namespace Kubernetes dans un changeset et le soumettre.";
    NAMESPACE_APPLY = "kube.namespace.apply", "Appliquer ou rejeter le changeset d'un autre qui touche un namespace Kubernetes.";
    NAMESPACE_APPROVAL_SELF = "kube.namespace.approval.self", "Appliquer son propre changeset qui touche un namespace Kubernetes.";
    WORKLOAD_READ = "kube.workload.read", "Lire les workloads Kubernetes.";
    WORKLOAD_EDIT = "kube.workload.edit", "Créer, modifier ou clore un workload Kubernetes dans un changeset et le soumettre.";
    WORKLOAD_APPLY = "kube.workload.apply", "Appliquer ou rejeter le changeset d'un autre qui touche un workload Kubernetes.";
    WORKLOAD_APPROVAL_SELF = "kube.workload.approval.self", "Appliquer son propre changeset qui touche un workload Kubernetes.";
    SERVICE_READ = "kube.service.read", "Lire les services Kubernetes.";
    SERVICE_EDIT = "kube.service.edit", "Créer, modifier ou clore un service Kubernetes dans un changeset et le soumettre.";
    SERVICE_APPLY = "kube.service.apply", "Appliquer ou rejeter le changeset d'un autre qui touche un service Kubernetes.";
    SERVICE_APPROVAL_SELF = "kube.service.approval.self", "Appliquer son propre changeset qui touche un service Kubernetes.";
    INGRESS_READ = "kube.ingress.read", "Lire les ingress Kubernetes.";
    INGRESS_EDIT = "kube.ingress.edit", "Créer, modifier ou clore un ingress Kubernetes dans un changeset et le soumettre.";
    INGRESS_APPLY = "kube.ingress.apply", "Appliquer ou rejeter le changeset d'un autre qui touche un ingress Kubernetes.";
    INGRESS_APPROVAL_SELF = "kube.ingress.approval.self", "Appliquer son propre changeset qui touche un ingress Kubernetes.";
    // The journal of the pods observed, outside the changesets (ADR 0043, decision 2):
    // evaluated on the pod domain of the cluster (Q-019-23), in no delivered role.
    OBSERVATION_RECORD = "kube.observation.record", "Écrire un lot d'observations au journal des pods d'un cluster Kubernetes (son connecteur seulement).";
    OBSERVATION_READ = "kube.observation.read", "Lire le journal des pods observés d'un cluster Kubernetes : quel pod avait une adresse, où un workload a tourné.";
    // The settings of the module (spec 027, P4, P5; Q-019-35): global, for its three
    // types of settings; read under `kube.cluster.read`.
    [global] SETTINGS_EDIT = "kube.settings.edit", "Modifier dans un changeset et soumettre les paramètres du module Kubernetes : correspondances de labels vers des valeurs d'axe, valeurs admises par namespace, labels suivis.";
    [global] SETTINGS_APPLY = "kube.settings.apply", "Appliquer ou rejeter le changeset d'un autre qui touche les paramètres du module Kubernetes.";
    [global] SETTINGS_APPROVAL_SELF = "kube.settings.approval.self", "Appliquer son propre changeset qui touche les paramètres du module Kubernetes.";
}

/// The permission of a batch of observations (`POST /v1/kube-clusters/{id}/observations`).
pub const OBSERVATION_RECORD_PERMISSION: &str = OBSERVATION_RECORD.name;

/// The permission of the reads of the journal (`GET /v1/kube-clusters/{id}/observations`,
/// `GET /v1/workloads/{id}/observations`, `read::who_had`), on the pod domain of the
/// cluster (Q-019-23).
pub const OBSERVATION_READ_PERMISSION: &str = OBSERVATION_READ.name;

/// What closing an object does to the objects of the module that cite it (D33; design
/// « Références »).
static REFERENCES: &[Reference] = &[
    // A node and a namespace live in their cluster: closed with it.
    Reference {
        from: NODE,
        to: CLUSTER,
        on_close: OnClose::Cascade,
    },
    Reference {
        from: NAMESPACE,
        to: CLUSTER,
        on_close: OnClose::Cascade,
    },
    // A workload lives in its namespace: closed with it.
    Reference {
        from: WORKLOAD,
        to: NAMESPACE,
        on_close: OnClose::Cascade,
    },
    // A service lives in its namespace: closed with it.
    Reference {
        from: SERVICE,
        to: NAMESPACE,
        on_close: OnClose::Cascade,
    },
    // The cluster IPs and external addresses of a service (usage `vip`): kept while it
    // cites them, closed by the core with the service; the connector closes one once the
    // service no longer holds it (`kube.sync`).
    Reference {
        from: SERVICE,
        to: ADDRESS,
        on_close: OnClose::Owned,
    },
    // An ingress lives in its namespace: closed with it.
    Reference {
        from: INGRESS,
        to: NAMESPACE,
        on_close: OnClose::Cascade,
    },
    // The addresses of an ingress (usage `vip`), as those of a service. Its routes name a
    // service by its name, not by a reference (`kube.ingress.backend_missing` warns).
    Reference {
        from: INGRESS,
        to: ADDRESS,
        on_close: OnClose::Owned,
    },
    // The admitted values of an axis in a namespace: the namespace is not closed while they
    // live (Q-019-35).
    Reference {
        from: ADMITTED_VALUES,
        to: NAMESPACE,
        on_close: OnClose::Refuse,
    },
    // A workload a service selects: left out of the selection.
    Reference {
        from: SERVICE,
        to: WORKLOAD,
        on_close: OnClose::Nullify,
    },
    // The machine a node designates: no longer designated.
    Reference {
        from: NODE,
        to: MACHINE,
        on_close: OnClose::Nullify,
    },
    // The networks of the pod ranges of a node: kept while it cites them, closed by the
    // core with the node, in the cascade of its cluster or not; the connector closes one
    // once its range leaves the node (`kube.sync`).
    Reference {
        from: NODE,
        to: PREFIX,
        on_close: OnClose::Owned,
    },
    // The domains, containers and networks a cluster designates: kept while it lives.
    Reference {
        from: CLUSTER,
        to: DOMAIN,
        on_close: OnClose::Refuse,
    },
    Reference {
        from: CLUSTER,
        to: PREFIX,
        on_close: OnClose::Refuse,
    },
];

/// The edges of the topology graph (REQ-019-11; ADR 0026, clause 11): with those of the
/// machines module (`machines.hosted_on`, `hosted_in`, `member_of`), the chain « an
/// ingress routes to a service, a service selects a workload, a node runs on a machine,
/// a machine is hosted by a hypervisor ». The link from a workload to the nodes its pods
/// run on is observed (journal of observations), not an edge of the versioned model.
static EDGES: &[EdgeType] = &[
    // The machine a node designates.
    EdgeType {
        name: "kube.runs_on",
        from: NODE,
        to: MACHINE,
    },
    // The networks of the pod ranges of a node.
    EdgeType {
        name: "kube.node_range",
        from: NODE,
        to: PREFIX,
    },
    // The workloads a service selects.
    EdgeType {
        name: "kube.selects",
        from: SERVICE,
        to: WORKLOAD,
    },
    // The cluster IPs and external addresses of a service.
    EdgeType {
        name: "kube.service_address",
        from: SERVICE,
        to: ADDRESS,
    },
    // The services of its namespace a route of an ingress names.
    EdgeType {
        name: "kube.routes_to",
        from: INGRESS,
        to: SERVICE,
    },
    // The addresses of an ingress.
    EdgeType {
        name: "kube.ingress_address",
        from: INGRESS,
        to: ADDRESS,
    },
];

/// The Kubernetes module (spec 019).
#[derive(Debug, Clone, Copy, Default)]
pub struct Kube;

impl Module for Kube {
    fn name(&self) -> &'static str {
        MODULE
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn depends_on(&self) -> &'static [&'static str] {
        &["machines"]
    }
    fn object_types(&self) -> &'static [ObjectType] {
        TYPES
    }
    fn declarations(&self) -> Option<ModuleDeclarations> {
        Some(module_declarations())
    }
    fn permissions(&self) -> &'static [&'static Permission] {
        PERMISSIONS
    }
    fn type_permissions(&self) -> &'static [TypePermissions] {
        TYPE_PERMISSIONS
    }
    fn rules(&self) -> &'static [&'static Rule] {
        KUBE_RULES
    }
    fn labels(&self) -> Vec<kartenn_core::labels::CodeLabel> {
        kartenn_core::code_labels::of_module(PERMISSIONS, KUBE_RULES, &crate::labels::ENGLISH)
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
    fn jobs(&self) -> &'static [JobSpec] {
        #[cfg(feature = "pg")]
        {
            crate::jobs::JOBS
        }
        #[cfg(not(feature = "pg"))]
        {
            &[]
        }
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

    /// REQ-019-4: a build with `kube` and without `machines` does not start, and says why.
    #[test]
    fn kube_needs_machines() {
        let refused = ModuleRegistry::new(vec![Arc::new(Kube)]).expect_err("kube alone");
        let message = refused.to_string();
        assert!(message.contains("kube"), "{message}");
        assert!(message.contains("machines"), "{message}");
        let registry =
            ModuleRegistry::new(vec![Arc::new(Kube), Arc::new(kartenn_machines::Machines)])
                .expect("kube with machines");
        let names: Vec<&str> = registry.modules().map(|m| m.name()).collect();
        assert_eq!(names, ["machines", "kube"], "machines first");
    }

    /// REQ-018-1 (R2, R5), REQ-019-30: the permissions of the module follow the naming
    /// rule, alone and in the catalogue of the core and of the modules it depends on.
    #[test]
    fn catalogue_problems_empty() {
        let module = Kube.permissions();
        assert_eq!(
            kartenn_core::authz::catalogue_problems(module),
            Vec::<String>::new()
        );
        let catalogue: Vec<&Permission> = kartenn_core::authz::PERMISSIONS
            .iter()
            .copied()
            .chain(kartenn_machines::Machines.permissions().iter().copied())
            .chain(module.iter().copied())
            .collect();
        assert_eq!(
            kartenn_core::authz::catalogue_problems(&catalogue),
            Vec::<String>::new()
        );
        assert!(module.iter().all(|p| p.name.starts_with("kube.")));
    }
}
