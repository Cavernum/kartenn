//! The kube module as the core sees it (ADR 0026): its types of objects, permissions,
//! rules, operations, references and migrations, and its handler in the write pipeline
//! (feature `pg`). It depends on the machines module (REQ-019-4): a node designates a
//! machine of the 009.

use std::sync::Arc;

use kartenn_core::authz::Permission;
use kartenn_core::rules::Rule;
use kartenn_service::module::{
    Module, ModuleHandler, ModuleMigration, ObjectType, OnClose, OperationType, Reference,
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
/// A machine of the 009, as a reference names it.
pub const MACHINE: &str = "machines.machine";
/// An addressing domain of the core.
pub const DOMAIN: &str = "domain";
/// A prefix of the core: a container of pod ranges, a network of services.
pub const PREFIX: &str = "prefix";
/// An address of the core: a cluster IP or an external address of a service, an address
/// of an ingress.
pub const ADDRESS: &str = "address";

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
    // The batch of the connector: one object of a cluster per operation, created, changed
    // or closed by its identity in the cluster (019/X9).
    OperationType {
        name: "sync",
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
}

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

/// The Kubernetes module (spec 019).
#[derive(Debug, Clone, Copy, Default)]
pub struct Kube;

impl Module for Kube {
    fn name(&self) -> &'static str {
        "kube"
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
    fn permissions(&self) -> &'static [&'static Permission] {
        PERMISSIONS
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
}
