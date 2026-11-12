//! The English of the codes of the module (spec 018, R8 b): its permissions and rules
//! beside their French definitions, its kinds.

use kartenn_core::code_labels::English;

/// The English of the module.
pub(crate) static ENGLISH: English = English {
    permissions: &[
        ("kube.cluster.read", "Read the Kubernetes clusters."),
        (
            "kube.cluster.edit",
            "Create, change or close a Kubernetes cluster in a changeset and submit it.",
        ),
        (
            "kube.cluster.apply",
            "Apply or reject the changeset of someone else that touches a Kubernetes cluster.",
        ),
        (
            "kube.cluster.approval.self",
            "Apply one's own changeset that touches a Kubernetes cluster.",
        ),
        ("kube.node.read", "Read the Kubernetes nodes."),
        (
            "kube.node.edit",
            "Create, change or close a Kubernetes node in a changeset and submit it.",
        ),
        (
            "kube.node.apply",
            "Apply or reject the changeset of someone else that touches a Kubernetes node.",
        ),
        (
            "kube.node.approval.self",
            "Apply one's own changeset that touches a Kubernetes node.",
        ),
        ("kube.namespace.read", "Read the Kubernetes namespaces."),
        (
            "kube.namespace.edit",
            "Create, change or close a Kubernetes namespace in a changeset and submit it.",
        ),
        (
            "kube.namespace.apply",
            "Apply or reject the changeset of someone else that touches a Kubernetes namespace.",
        ),
        (
            "kube.namespace.approval.self",
            "Apply one's own changeset that touches a Kubernetes namespace.",
        ),
        (
            "kube.observation.record",
            "Write a batch of observations to the pod journal of a Kubernetes cluster (its connector only).",
        ),
        (
            "kube.observation.read",
            "Read the pod journal of a Kubernetes cluster: which pod had an address, where a workload ran.",
        ),
        (
            "kube.settings.edit",
            "Change in a changeset and submit the settings of the Kubernetes module: mappings of labels to values of axes, admitted values by namespace, tracked labels.",
        ),
        (
            "kube.settings.apply",
            "Apply or reject the changeset of someone else that touches the settings of the Kubernetes module.",
        ),
        (
            "kube.settings.approval.self",
            "Apply one's own changeset that touches the settings of the Kubernetes module.",
        ),
        ("kube.workload.read", "Read the Kubernetes workloads."),
        (
            "kube.workload.edit",
            "Create, change or close a Kubernetes workload in a changeset and submit it.",
        ),
        (
            "kube.workload.apply",
            "Apply or reject the changeset of someone else that touches a Kubernetes workload.",
        ),
        (
            "kube.workload.approval.self",
            "Apply one's own changeset that touches a Kubernetes workload.",
        ),
        ("kube.service.read", "Read the Kubernetes services."),
        (
            "kube.service.edit",
            "Create, change or close a Kubernetes service in a changeset and submit it.",
        ),
        (
            "kube.service.apply",
            "Apply or reject the changeset of someone else that touches a Kubernetes service.",
        ),
        (
            "kube.service.approval.self",
            "Apply one's own changeset that touches a Kubernetes service.",
        ),
        ("kube.ingress.read", "Read the Kubernetes ingresses."),
        (
            "kube.ingress.edit",
            "Create, change or close a Kubernetes ingress in a changeset and submit it.",
        ),
        (
            "kube.ingress.apply",
            "Apply or reject the changeset of someone else that touches a Kubernetes ingress.",
        ),
        (
            "kube.ingress.approval.self",
            "Apply one's own changeset that touches a Kubernetes ingress.",
        ),
    ],
    rules: &[
        (
            "kube.operation.invalid",
            "An operation of the module carries the fields it expects, with admitted values and objects that exist.",
        ),
        (
            "kube.object.managed",
            "The tracked fields of a Kubernetes object are written only by a service account member of the connector group of its cluster.",
        ),
        (
            "kube.connector.direct",
            "A member of a connector group writes only through the kube.sync operation.",
        ),
        (
            "kube.cluster.range_domain",
            "A range of a cluster is in the domain the cluster designates for it.",
        ),
        (
            "kube.cluster.connector_group",
            "The connector group of a cluster exists and lives over the whole period.",
        ),
        (
            "kube.node.duplicate",
            "Two living nodes of a cluster do not carry the same name over overlapping periods.",
        ),
        (
            "kube.namespace.duplicate",
            "Two living namespaces of a cluster do not carry the same name over overlapping periods.",
        ),
        (
            "kube.workload.duplicate",
            "Two living workloads of a namespace do not carry the same kind and name over overlapping periods.",
        ),
        (
            "kube.service.duplicate",
            "Two living services of a namespace do not carry the same name over overlapping periods.",
        ),
        (
            "kube.ingress.duplicate",
            "Two living ingresses of a namespace do not carry the same name over overlapping periods.",
        ),
        (
            "kube.node.machine_kind",
            "A node designates a physical or virtual machine that no other living node designates.",
        ),
        (
            "kube.name.invalid",
            "The name of a Kubernetes object is a lower-case DNS label or subdomain.",
        ),
        (
            "kube.label.invalid",
            "A label key, its value and a selector follow the syntax of Kubernetes.",
        ),
        (
            "kube.node.range_outside",
            "The pod range of a node is inside a pod range container of its cluster.",
        ),
        (
            "kube.node.unmatched",
            "A node designates a machine found without ambiguity.",
        ),
        (
            "kube.object.absent",
            "A Kubernetes object cited by a dependent is present in its cluster.",
        ),
        (
            "kube.namespace.held",
            "A namespace gone from its cluster is no longer cited by admitted values.",
        ),
        (
            "kube.ingress.backend_missing",
            "An ingress rule targets a service present in its namespace.",
        ),
        (
            "kube.label.not_admitted",
            "The axis value of a label is admitted in the namespace of the workload that carries it.",
        ),
        (
            "kube.machine.looks_like_pod",
            "A container machine hosted by the machine of a node does not carry the name of an observed pod.",
        ),
        (
            "kube.sync.stale",
            "A cluster receives a batch or a bookmark from its connector at least every 10 minutes.",
        ),
    ],
    kinds: &[
        ("kube.cluster", "Cluster Kubernetes", "Kubernetes cluster"),
        ("kube.node", "Nœud Kubernetes", "Kubernetes node"),
        (
            "kube.namespace",
            "Namespace Kubernetes",
            "Kubernetes namespace",
        ),
        (
            "kube.workload",
            "Workload Kubernetes",
            "Kubernetes workload",
        ),
        ("kube.service", "Service Kubernetes", "Kubernetes service"),
        ("kube.ingress", "Ingress Kubernetes", "Kubernetes ingress"),
        (
            "kube.label_mapping",
            "Correspondance de label Kubernetes",
            "Kubernetes label mapping",
        ),
        (
            "kube.admitted_values",
            "Valeurs admises d'un namespace Kubernetes",
            "Admitted values of a Kubernetes namespace",
        ),
        (
            "kube.tracked_label",
            "Label Kubernetes suivi",
            "Tracked Kubernetes label",
        ),
    ],
    states: &[],
};
