//! The workload of a pod (019/X3): its root controller, reached by climbing
//! `metadata.ownerReferences` (réf. `docs/reference/kubernetes-calcul.md`, § Pod).
//!
//! Documented by Kubernetes: owner references name an owner of the same namespace by kind,
//! name and UID, and are set on the dependents of `ReplicaSet`, `DaemonSet`, `Deployment`,
//! `Job` and `CronJob`; `StatefulSet` is a workload resource too.
//!
//! Decided by 019/X3: only the reference marked `controller: true` is followed; a
//! `Deployment` (through its `ReplicaSet`), a `StatefulSet`, a `DaemonSet` and a `CronJob`
//! (through its `Job`) are workloads whatever controls them, and the climb stops there (a
//! `StatefulSet` created by an operator is the workload); a `Job` or a `ReplicaSet`
//! without controller is a workload; a pod without controller, or controlled by its `v1`
//! `Node` (the mirror of a static pod), is its own workload, of kind `pod`.
//!
//! Chosen by kartenn: a kind is recognised by its API group and kind together (`apps` for
//! `Deployment`, `StatefulSet`, `DaemonSet`, `ReplicaSet`; `batch` for `Job` and `CronJob`;
//! the core group for `Pod`), so that a custom resource of the same kind name is not mistaken for
//! one; an owner is the one referenced only if its UID is the reference's; a controller of
//! a kind kartenn does not know (above a `ReplicaSet`, a `Job` or a pod), an owner not at
//! hand or more than [`MAX_DEPTH`] steps is an error the caller decides on (task 019-10),
//! never a guessed workload.

use std::fmt;

use uuid::Uuid;

/// Longest chain followed: a pod, a `Job` and a `CronJob` are two steps.
pub const MAX_DEPTH: usize = 4;

/// Kind of a workload (REQ-019-8), spelled as the `kube.workload.kind` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkloadKind {
    /// `apps/v1` `Deployment`.
    Deployment,
    /// `apps/v1` `StatefulSet`.
    StatefulSet,
    /// `apps/v1` `DaemonSet`.
    DaemonSet,
    /// `batch/v1` `CronJob`.
    CronJob,
    /// `batch/v1` `Job`.
    Job,
    /// `apps/v1` `ReplicaSet`.
    ReplicaSet,
    /// `v1` `Pod`.
    Pod,
}

impl WorkloadKind {
    /// Every kind, in the order of REQ-019-8.
    pub const ALL: [Self; 7] = [
        Self::Deployment,
        Self::StatefulSet,
        Self::DaemonSet,
        Self::CronJob,
        Self::Job,
        Self::ReplicaSet,
        Self::Pod,
    ];

    /// The kartenn spelling (`deployment`, `statefulset`…).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deployment => "deployment",
            Self::StatefulSet => "statefulset",
            Self::DaemonSet => "daemonset",
            Self::CronJob => "cronjob",
            Self::Job => "job",
            Self::ReplicaSet => "replicaset",
            Self::Pod => "pod",
        }
    }

    /// Whether the kind is a workload whatever controls it (019/X3): `Deployment`,
    /// `StatefulSet`, `DaemonSet`, `CronJob`. A `Job`, a `ReplicaSet` or a pod is one only
    /// without a controller to climb to.
    #[must_use]
    pub const fn is_root(self) -> bool {
        matches!(
            self,
            Self::Deployment | Self::StatefulSet | Self::DaemonSet | Self::CronJob
        )
    }

    /// Reads the kartenn spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == text)
    }

    /// The kind of an API object, from its `apiVersion` and `kind`; `None` for any other
    /// resource, a custom one of the same kind name included.
    #[must_use]
    pub fn of_api(api_version: &str, kind: &str) -> Option<Self> {
        let group = api_version.split_once('/').map_or("", |(group, _)| group);
        match (group, kind) {
            ("apps", "Deployment") => Some(Self::Deployment),
            ("apps", "StatefulSet") => Some(Self::StatefulSet),
            ("apps", "DaemonSet") => Some(Self::DaemonSet),
            ("apps", "ReplicaSet") => Some(Self::ReplicaSet),
            ("batch", "CronJob") => Some(Self::CronJob),
            ("batch", "Job") => Some(Self::Job),
            ("", "Pod") => Some(Self::Pod),
            _ => None,
        }
    }
}

impl fmt::Display for WorkloadKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An element of `metadata.ownerReferences`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerReference {
    /// `apiVersion` of the owner.
    pub api_version: String,
    /// `kind` of the owner.
    pub kind: String,
    /// Name of the owner, in the dependent's namespace.
    pub name: String,
    /// UID of the owner.
    pub uid: Uuid,
    /// Whether this owner is the managing controller.
    pub controller: bool,
}

/// What the climb needs of an object: its type, name, UID and owners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owned {
    /// `apiVersion`.
    pub api_version: String,
    /// `kind`.
    pub kind: String,
    /// `metadata.name`.
    pub name: String,
    /// `metadata.uid`.
    pub uid: Uuid,
    /// `metadata.ownerReferences`.
    pub owner_references: Vec<OwnerReference>,
}

/// The workload an object belongs to: its kind, name and UID (an attribute, 019/X3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkloadRef {
    /// Kind.
    pub kind: WorkloadKind,
    /// Name, in the namespace.
    pub name: String,
    /// UID of the object found.
    pub uid: Uuid,
}

/// Why no workload was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerChainError {
    /// The object climbed from is not of a workload kind.
    NotAWorkload {
        /// Its `apiVersion`.
        api_version: String,
        /// Its `kind`.
        kind: String,
    },
    /// The controller is of a kind kartenn does not follow (a custom resource, for
    /// instance); the last object of a known kind is given for the caller to decide.
    ForeignController {
        /// The reference to the foreign controller.
        controller: Box<OwnerReference>,
        /// The last object of a workload kind on the way.
        below: WorkloadRef,
    },
    /// The controller is not at hand (not read yet, or read with another UID).
    Missing(OwnerReference),
    /// The chain is longer than [`MAX_DEPTH`], a loop included.
    TooDeep,
}

impl fmt::Display for OwnerChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAWorkload { api_version, kind } => {
                write!(
                    f,
                    "« {api_version} {kind} » n'est pas une sorte de workload"
                )
            }
            Self::ForeignController { controller, below } => write!(
                f,
                "contrôleur « {} {} » non suivi au-dessus de {} « {} »",
                controller.api_version, controller.kind, below.kind, below.name
            ),
            Self::Missing(r) => write!(f, "propriétaire {} « {} » introuvable", r.kind, r.name),
            Self::TooDeep => write!(f, "chaîne de propriétaires de plus de {MAX_DEPTH} maillons"),
        }
    }
}

impl std::error::Error for OwnerChainError {}

/// Climbs from `object` to its workload. `lookup` finds an owner of the same namespace
/// from its reference (by kind and name, or by UID); an owner whose UID differs from the
/// reference's counts as missing.
///
/// # Errors
/// See [`OwnerChainError`].
pub fn workload_of<'a>(
    object: &'a Owned,
    lookup: impl Fn(&OwnerReference) -> Option<&'a Owned>,
) -> Result<WorkloadRef, OwnerChainError> {
    let mut current = object;
    for _ in 0..=MAX_DEPTH {
        let Some(kind) = WorkloadKind::of_api(&current.api_version, &current.kind) else {
            return Err(OwnerChainError::NotAWorkload {
                api_version: current.api_version.clone(),
                kind: current.kind.clone(),
            });
        };
        let here = || WorkloadRef {
            kind,
            name: current.name.clone(),
            uid: current.uid,
        };
        // A Deployment, a StatefulSet, a DaemonSet or a CronJob is a workload whatever
        // controls it (an operator, for instance): the climb stops there.
        if kind.is_root() {
            return Ok(here());
        }
        let Some(controller) = current.owner_references.iter().find(|r| r.controller) else {
            return Ok(here());
        };
        // The mirror of a static pod is controlled by its node: it is its own workload.
        if kind == WorkloadKind::Pod && is_node(controller) {
            return Ok(here());
        }
        if WorkloadKind::of_api(&controller.api_version, &controller.kind).is_none() {
            return Err(OwnerChainError::ForeignController {
                controller: Box::new(controller.clone()),
                below: here(),
            });
        }
        match lookup(controller) {
            Some(owner) if owner.uid == controller.uid => current = owner,
            _ => return Err(OwnerChainError::Missing(controller.clone())),
        }
    }
    Err(OwnerChainError::TooDeep)
}

/// Whether a reference names a `v1` `Node`, the controller of a mirror pod.
fn is_node(reference: &OwnerReference) -> bool {
    reference.api_version == "v1" && reference.kind == "Node"
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn uid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn object(api_version: &str, kind: &str, name: &str, n: u128) -> Owned {
        Owned {
            api_version: api_version.into(),
            kind: kind.into(),
            name: name.into(),
            uid: uid(n),
            owner_references: Vec::new(),
        }
    }

    fn owned_by(mut child: Owned, owner: &Owned, controller: bool) -> Owned {
        child.owner_references.push(OwnerReference {
            api_version: owner.api_version.clone(),
            kind: owner.kind.clone(),
            name: owner.name.clone(),
            uid: owner.uid,
            controller,
        });
        child
    }

    /// The owners at hand, found by UID.
    struct Store(HashMap<Uuid, Owned>);

    impl Store {
        fn new(objects: impl IntoIterator<Item = Owned>) -> Self {
            Self(objects.into_iter().map(|o| (o.uid, o)).collect())
        }

        fn climb(&self, from: &Owned) -> Result<WorkloadRef, OwnerChainError> {
            workload_of(from, |r| self.0.get(&r.uid))
        }
    }

    fn workload(kind: WorkloadKind, name: &str, n: u128) -> WorkloadRef {
        WorkloadRef {
            kind,
            name: name.into(),
            uid: uid(n),
        }
    }

    #[test]
    fn owner_chain_to_deployment() {
        let deployment = object("apps/v1", "Deployment", "pay", 1);
        let replicaset = owned_by(
            object("apps/v1", "ReplicaSet", "pay-5d8f7", 2),
            &deployment,
            true,
        );
        let pod = owned_by(object("v1", "Pod", "pay-5d8f7-x2k4q", 3), &replicaset, true);
        let store = Store::new([deployment, replicaset.clone()]);
        assert_eq!(
            store.climb(&pod),
            Ok(workload(WorkloadKind::Deployment, "pay", 1))
        );
        // A ReplicaSet without owner is its own workload.
        let lone_rs = object("apps/v1", "ReplicaSet", "legacy", 4);
        let pod = owned_by(object("v1", "Pod", "legacy-abcde", 5), &lone_rs, true);
        let store = Store::new([lone_rs]);
        assert_eq!(
            store.climb(&pod),
            Ok(workload(WorkloadKind::ReplicaSet, "legacy", 4))
        );
        // StatefulSet and DaemonSet own their pods directly.
        for (kind, workload_kind) in [
            ("StatefulSet", WorkloadKind::StatefulSet),
            ("DaemonSet", WorkloadKind::DaemonSet),
        ] {
            let set = object("apps/v1", kind, "db", 6);
            let pod = owned_by(object("v1", "Pod", "db-0", 7), &set, true);
            let store = Store::new([set]);
            assert_eq!(store.climb(&pod), Ok(workload(workload_kind, "db", 6)));
        }
    }

    #[test]
    fn cronjob_job_pod() {
        let cronjob = object("batch/v1", "CronJob", "backup", 1);
        let job = owned_by(object("batch/v1", "Job", "backup-29001", 2), &cronjob, true);
        let pod = owned_by(object("v1", "Pod", "backup-29001-q7z", 3), &job, true);
        let store = Store::new([cronjob, job]);
        assert_eq!(
            store.climb(&pod),
            Ok(workload(WorkloadKind::CronJob, "backup", 1))
        );
        // A Job without owner is its own workload.
        let job = object("batch/v1", "Job", "migrate", 4);
        let pod = owned_by(object("v1", "Pod", "migrate-k2", 5), &job, true);
        let store = Store::new([job]);
        assert_eq!(
            store.climb(&pod),
            Ok(workload(WorkloadKind::Job, "migrate", 4))
        );
    }

    #[test]
    fn bare_pod_is_its_own_workload() {
        let pod = object("v1", "Pod", "debug", 1);
        assert_eq!(
            Store::new([]).climb(&pod),
            Ok(workload(WorkloadKind::Pod, "debug", 1))
        );
        // An owner that is not the controller is not followed.
        let other = object("apps/v1", "Deployment", "not-controller", 2);
        let pod = owned_by(pod, &other, false);
        let store = Store::new([other]);
        assert_eq!(
            store.climb(&pod),
            Ok(workload(WorkloadKind::Pod, "debug", 1))
        );
    }

    #[test]
    fn operator_owned_statefulset_is_the_workload() {
        // An operator's resource controls a StatefulSet: the StatefulSet is the workload,
        // and the operator's resource is never looked up.
        let cluster = object("example.org/v1", "Database", "orders", 1);
        let set = owned_by(
            object("apps/v1", "StatefulSet", "orders-db", 2),
            &cluster,
            true,
        );
        let pod = owned_by(object("v1", "Pod", "orders-db-0", 3), &set, true);
        let store = Store::new([set]);
        assert_eq!(
            store.climb(&pod),
            Ok(workload(WorkloadKind::StatefulSet, "orders-db", 2))
        );
        // Likewise a Deployment, a DaemonSet and a CronJob stop the climb, whatever
        // controls them (here even a known kind).
        let above = object("apps/v1", "Deployment", "above", 4);
        for (api, kind, workload_kind) in [
            ("apps/v1", "Deployment", WorkloadKind::Deployment),
            ("apps/v1", "DaemonSet", WorkloadKind::DaemonSet),
            ("batch/v1", "CronJob", WorkloadKind::CronJob),
        ] {
            let root = owned_by(object(api, kind, "root", 5), &above, true);
            let store = Store::new([above.clone()]);
            assert_eq!(store.climb(&root), Ok(workload(workload_kind, "root", 5)));
        }
    }

    #[test]
    fn mirror_pod_is_its_own_workload() {
        // The mirror of a static pod is controlled by its node.
        let node = object("v1", "Node", "node-1", 1);
        let pod = owned_by(object("v1", "Pod", "store-node-1", 2), &node, true);
        assert_eq!(
            Store::new([]).climb(&pod),
            Ok(workload(WorkloadKind::Pod, "store-node-1", 2))
        );
    }

    #[test]
    fn unresolved_chains_are_errors() {
        // A controller of a kind kartenn does not know, above a ReplicaSet or a Job, is
        // foreign.
        let release = object("example.org/v1", "Release", "web", 1);
        let rs = owned_by(
            object("apps/v1", "ReplicaSet", "web-6c9", 2),
            &release,
            true,
        );
        let pod = owned_by(object("v1", "Pod", "web-6c9-a1", 3), &rs, true);
        let store = Store::new([release.clone(), rs]);
        assert!(matches!(
            store.climb(&pod),
            Err(OwnerChainError::ForeignController { below, .. })
                if below == workload(WorkloadKind::ReplicaSet, "web-6c9", 2)
        ));
        let job = owned_by(object("batch/v1", "Job", "run-1", 13), &release, true);
        let pod = owned_by(object("v1", "Pod", "run-1-a", 14), &job, true);
        assert!(matches!(
            Store::new([release, job]).climb(&pod),
            Err(OwnerChainError::ForeignController { below, .. })
                if below == workload(WorkloadKind::Job, "run-1", 13)
        ));
        // A custom resource that borrows a workload kind name is foreign too.
        let custom = object("example.org/v1", "Deployment", "web", 4);
        let pod = owned_by(object("v1", "Pod", "web-1", 5), &custom, true);
        assert!(matches!(
            Store::new([custom]).climb(&pod),
            Err(OwnerChainError::ForeignController { .. })
        ));
        // An owner not read yet, or read with another UID (recreated), is missing.
        let deployment = object("apps/v1", "Deployment", "pay", 6);
        let rs = owned_by(
            object("apps/v1", "ReplicaSet", "pay-1", 7),
            &deployment,
            true,
        );
        let pod = owned_by(object("v1", "Pod", "pay-1-a", 8), &rs, true);
        assert!(
            matches!(Store::new([]).climb(&pod), Err(OwnerChainError::Missing(r)) if r.name == "pay-1")
        );
        let recreated = object("apps/v1", "ReplicaSet", "pay-1", 9);
        assert!(matches!(
            workload_of(&pod, |_| Some(&recreated)),
            Err(OwnerChainError::Missing(_))
        ));
        // A loop stops.
        let mut a = object("apps/v1", "ReplicaSet", "a", 10);
        let b = owned_by(object("apps/v1", "ReplicaSet", "b", 11), &a, true);
        a = owned_by(a, &b, true);
        assert_eq!(
            Store::new([a.clone(), b]).climb(&a),
            Err(OwnerChainError::TooDeep)
        );
        // Climbing from something that is not a workload kind.
        let cm = object("v1", "ConfigMap", "conf", 12);
        assert!(matches!(
            Store::new([]).climb(&cm),
            Err(OwnerChainError::NotAWorkload { .. })
        ));
    }

    #[test]
    fn kinds_spelled_as_the_schema() {
        let spelled: Vec<_> = WorkloadKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(
            spelled,
            [
                "deployment",
                "statefulset",
                "daemonset",
                "cronjob",
                "job",
                "replicaset",
                "pod"
            ]
        );
        for kind in WorkloadKind::ALL {
            assert_eq!(WorkloadKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(
            WorkloadKind::of_api("apps/v1beta2", "Deployment"),
            Some(WorkloadKind::Deployment)
        );
        assert_eq!(WorkloadKind::of_api("v1", "Deployment"), None);
        assert_eq!(WorkloadKind::of_api("apps/v1", "Pod"), None);
    }
}
