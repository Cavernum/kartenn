//! Projection of an observed Kubernetes object on its tracked fields (019/X4): only the
//! fields that serve the network or the flows are versioned, so that a deployment that
//! changes an image, a replica count or an annotation projects to the same value and makes
//! no changeset (REQ-019-20).
//!
//! The `Observed*` types are what the connector reads, already typed for what kartenn does
//! not check (addresses, ports, enumerations); the `Tracked*` types are what kartenn
//! versions, names and labels checked. Projecting drops every other field, keeps only the
//! tracked label keys (019/X4: those of the axis mappings, of the module list and of the
//! cluster list, gathered by the caller into [`TrackedKeys`]), and orders the sets
//! (ports, addresses, routes) so that a reordering in the cluster changes nothing.
//!
//! The UID is versioned too: a workload recreated under its name keeps its identity and a
//! new version notes its new UID (019/X3). The references kartenn computes (designated
//! machine of a node, addresses of the core, selected workloads stored by identifier) are
//! not projections and belong to the handler and the connector; [`ObservedService::selects`]
//! gives the selection they store.

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::rules::Violation;
use uuid::Uuid;

use crate::names::{DnsLabel, DnsSubdomain, LabelKey, LabelValue, Labels, parse_labels};
use crate::owner::WorkloadKind;
use crate::selector::Selector;

/// Labels as read from the cluster, all of them.
pub type ReadLabels = BTreeMap<String, String>;

/// Well-known label of a node's zone (Kubernetes, « Well-Known Labels, Annotations and
/// Taints »).
pub const ZONE_LABEL: &str = "topology.kubernetes.io/zone";

/// The label keys kartenn tracks for a cluster (019/X4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackedKeys(BTreeSet<LabelKey>);

impl TrackedKeys {
    /// The union of the given keys.
    #[must_use]
    pub fn new(keys: impl IntoIterator<Item = LabelKey>) -> Self {
        Self(keys.into_iter().collect())
    }

    /// Whether `key` is tracked.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains(key)
    }

    /// The tracked labels among `labels`; the others are dropped unchecked.
    ///
    /// # Errors
    /// `kube.label.invalid` for a tracked label whose value is out of syntax.
    pub fn project(&self, labels: &ReadLabels) -> Result<Labels, Violation> {
        parse_labels(
            labels
                .iter()
                .filter(|(k, _)| self.contains(k))
                .map(|(k, v)| (k.as_str(), v.as_str())),
        )
    }
}

/// Transport protocol of a port (TCP by default in the API).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Protocol {
    /// TCP.
    #[default]
    Tcp,
    /// UDP.
    Udp,
    /// SCTP.
    Sctp,
}

impl Protocol {
    /// Every protocol.
    pub const ALL: [Self; 3] = [Self::Tcp, Self::Udp, Self::Sctp];

    /// The kartenn spelling (`tcp`, `udp`, `sctp`), as the flows write it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::Sctp => "sctp",
        }
    }

    /// Reads the kartenn spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == text)
    }
}

/// A port declared by a container (`containers[].ports`): declared, never proven
/// listening (REQ-019-8).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclaredPort {
    /// `containerPort`.
    pub port: u16,
    /// `protocol`.
    pub protocol: Protocol,
    /// `name`, which a Service may target.
    pub name: Option<String>,
}

/// A container of a pod template, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    /// Name of the container (not tracked).
    pub name: String,
    /// Image (not tracked).
    pub image: String,
    /// Declared ports.
    pub ports: Vec<DeclaredPort>,
}

/// The pod template of a workload (its pod spec, for a bare pod), as read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PodTemplate {
    /// Labels of the template.
    pub labels: ReadLabels,
    /// Annotations (not tracked).
    pub annotations: ReadLabels,
    /// `serviceAccountName`; empty reads as none.
    pub service_account: Option<String>,
    /// `hostNetwork`.
    pub host_network: bool,
    /// Containers.
    pub containers: Vec<Container>,
}

/// A workload as read: its root controller and pod template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedWorkload {
    /// Kind.
    pub kind: WorkloadKind,
    /// `metadata.name`.
    pub name: String,
    /// `metadata.uid`.
    pub uid: Uuid,
    /// `spec.replicas` (not tracked).
    pub replicas: Option<u32>,
    /// Labels of the controller itself (not tracked: the template's are, REQ-019-8).
    pub labels: ReadLabels,
    /// Pod template.
    pub template: PodTemplate,
}

/// The tracked fields of a workload (019/X4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedWorkload {
    /// Kind.
    pub kind: WorkloadKind,
    /// Name.
    pub name: DnsSubdomain,
    /// UID (019/X3).
    pub uid: Uuid,
    /// Tracked labels of the pod template.
    pub labels: Labels,
    /// Service account.
    pub service_account: Option<DnsSubdomain>,
    /// Declared ports of every container, as a set.
    pub ports: BTreeSet<DeclaredPort>,
    /// `hostNetwork`.
    pub host_network: bool,
}

impl ObservedWorkload {
    /// The tracked fields.
    ///
    /// # Errors
    /// `kube.name.invalid` (name, service account), `kube.label.invalid` (tracked label).
    pub fn project(&self, keys: &TrackedKeys) -> Result<TrackedWorkload, Violation> {
        let template = &self.template;
        Ok(TrackedWorkload {
            kind: self.kind,
            name: DnsSubdomain::parse(&self.name)?,
            uid: self.uid,
            labels: keys.project(&template.labels)?,
            service_account: template
                .service_account
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(DnsSubdomain::parse)
                .transpose()?,
            ports: template
                .containers
                .iter()
                .flat_map(|c| c.ports.iter().cloned())
                .collect(),
            host_network: template.host_network,
        })
    }
}

/// A namespace as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedNamespace {
    /// `metadata.name`.
    pub name: String,
    /// `metadata.uid`.
    pub uid: Uuid,
    /// Labels.
    pub labels: ReadLabels,
}

/// The tracked fields of a namespace (019/X4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedNamespace {
    /// Name.
    pub name: DnsLabel,
    /// UID.
    pub uid: Uuid,
    /// Tracked labels.
    pub labels: Labels,
}

impl ObservedNamespace {
    /// The tracked fields.
    ///
    /// # Errors
    /// `kube.name.invalid`, `kube.label.invalid`.
    pub fn project(&self, keys: &TrackedKeys) -> Result<TrackedNamespace, Violation> {
        Ok(TrackedNamespace {
            name: DnsLabel::parse(&self.name)?,
            uid: self.uid,
            labels: keys.project(&self.labels)?,
        })
    }
}

/// An address of a node (`status.addresses`), as read: type (`InternalIP`, `Hostname`…)
/// and value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeAddress {
    /// `type`.
    pub kind: String,
    /// `address`.
    pub value: String,
}

/// A node as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedNode {
    /// `metadata.name`.
    pub name: String,
    /// `metadata.uid`.
    pub uid: Uuid,
    /// Labels (only the zone is tracked).
    pub labels: ReadLabels,
    /// `status.addresses`.
    pub addresses: Vec<NodeAddress>,
    /// `spec.podCIDRs`, at most one per family.
    pub pod_cidrs: Vec<Prefix>,
}

/// The tracked fields of a node (019/X4), but its designated machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedNode {
    /// Name.
    pub name: DnsSubdomain,
    /// UID.
    pub uid: Uuid,
    /// Addresses, as a set.
    pub addresses: BTreeSet<NodeAddress>,
    /// Pod ranges, as a set.
    pub pod_ranges: BTreeSet<Prefix>,
    /// Zone, from [`ZONE_LABEL`].
    pub zone: Option<LabelValue>,
}

impl ObservedNode {
    /// The tracked fields.
    ///
    /// # Errors
    /// `kube.name.invalid`, `kube.label.invalid` (zone).
    pub fn project(&self) -> Result<TrackedNode, Violation> {
        Ok(TrackedNode {
            name: DnsSubdomain::parse(&self.name)?,
            uid: self.uid,
            addresses: self.addresses.iter().cloned().collect(),
            pod_ranges: self.pod_cidrs.iter().copied().collect(),
            zone: self
                .labels
                .get(ZONE_LABEL)
                .map(|z| LabelValue::parse(z))
                .transpose()?,
        })
    }
}

/// Type of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ServiceType {
    /// `ClusterIP`.
    ClusterIp,
    /// `NodePort`.
    NodePort,
    /// `LoadBalancer`.
    LoadBalancer,
    /// `ExternalName`.
    ExternalName,
}

impl ServiceType {
    /// The kartenn spelling, as the `kube.service.type` column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClusterIp => "cluster_ip",
            Self::NodePort => "node_port",
            Self::LoadBalancer => "load_balancer",
            Self::ExternalName => "external_name",
        }
    }

    /// Reads the kartenn spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::ClusterIp,
            Self::NodePort,
            Self::LoadBalancer,
            Self::ExternalName,
        ]
        .into_iter()
        .find(|t| t.as_str() == text)
    }
}

/// The port a service or an ingress targets: a number or a port name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PortRef {
    /// A port number.
    Number(u16),
    /// A port name.
    Name(String),
}

/// A port of a service.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServicePort {
    /// `port`.
    pub port: u16,
    /// `targetPort`.
    pub target: PortRef,
    /// `protocol`.
    pub protocol: Protocol,
    /// `appProtocol`.
    pub app_protocol: Option<String>,
    /// `nodePort`.
    pub node_port: Option<u16>,
    /// `name`, which an ingress may target.
    pub name: Option<String>,
}

/// A service as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedService {
    /// `metadata.name`.
    pub name: String,
    /// `metadata.uid`.
    pub uid: Uuid,
    /// Labels (not tracked).
    pub labels: ReadLabels,
    /// `spec.type`.
    pub service_type: ServiceType,
    /// `spec.clusterIP` is `None`.
    pub headless: bool,
    /// `spec.selector`; empty when the service has none.
    pub selector: ReadLabels,
    /// `spec.ports`.
    pub ports: Vec<ServicePort>,
    /// `spec.clusterIPs`, headless excluded.
    pub cluster_ips: Vec<Addr>,
    /// `spec.externalIPs` and the IP of `status.loadBalancer.ingress`.
    pub external_ips: Vec<Addr>,
    /// `spec.externalName`.
    pub external_name: Option<String>,
}

/// The tracked fields of a service (019/X4), but its selected workloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedService {
    /// Name.
    pub name: DnsLabel,
    /// UID.
    pub uid: Uuid,
    /// Type.
    pub service_type: ServiceType,
    /// Headless.
    pub headless: bool,
    /// Selector, equality only; empty when the service has none.
    pub selector: Selector,
    /// Ports, as a set.
    pub ports: BTreeSet<ServicePort>,
    /// Cluster IPs, as a set.
    pub cluster_ips: BTreeSet<Addr>,
    /// External addresses, as a set.
    pub external_ips: BTreeSet<Addr>,
    /// External name of an `ExternalName` service.
    pub external_name: Option<String>,
}

impl ObservedService {
    /// The tracked fields.
    ///
    /// # Errors
    /// `kube.name.invalid`, `kube.label.invalid` (selector).
    pub fn project(&self) -> Result<TrackedService, Violation> {
        Ok(TrackedService {
            name: DnsLabel::parse(&self.name)?,
            uid: self.uid,
            service_type: self.service_type,
            headless: self.headless,
            selector: Selector::of_labels(parse_labels(
                self.selector.iter().map(|(k, v)| (k.as_str(), v.as_str())),
            )?),
            ports: self.ports.iter().cloned().collect(),
            cluster_ips: self.cluster_ips.iter().copied().collect(),
            external_ips: self.external_ips.iter().copied().collect(),
            external_name: self.external_name.clone(),
        })
    }

    /// Whether the service selects the workload, on all the labels of its pod template;
    /// a service without selector selects none (its endpoints are managed by hand).
    ///
    /// # Errors
    /// `kube.label.invalid` (selector).
    pub fn selects(&self, workload: &ObservedWorkload) -> Result<bool, Violation> {
        if self.selector.is_empty() {
            return Ok(false);
        }
        let selector = Selector::of_labels(parse_labels(
            self.selector.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        )?);
        Ok(selector.matches_read(&workload.template.labels))
    }
}

/// `pathType` of an ingress path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathType {
    /// `Exact`.
    Exact,
    /// `Prefix`.
    Prefix,
    /// `ImplementationSpecific`.
    ImplementationSpecific,
}

impl PathType {
    /// The kartenn spelling, as a route of the `kube.ingress.rules` column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Prefix => "prefix",
            Self::ImplementationSpecific => "implementation_specific",
        }
    }

    /// Reads the kartenn spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [Self::Exact, Self::Prefix, Self::ImplementationSpecific]
            .into_iter()
            .find(|t| t.as_str() == text)
    }
}

/// The service backend of an ingress, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressBackend {
    /// Service name.
    pub service: String,
    /// Service port.
    pub port: PortRef,
}

/// A path of an ingress rule, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressPath {
    /// `path`.
    pub path: Option<String>,
    /// `pathType`.
    pub path_type: PathType,
    /// Backend.
    pub backend: IngressBackend,
}

/// A rule of an ingress, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressRule {
    /// `host`.
    pub host: Option<String>,
    /// `http.paths`.
    pub paths: Vec<IngressPath>,
}

/// An ingress as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedIngress {
    /// `metadata.name`.
    pub name: String,
    /// `metadata.uid`.
    pub uid: Uuid,
    /// Annotations (not tracked).
    pub annotations: ReadLabels,
    /// `spec.ingressClassName`.
    pub class: Option<String>,
    /// `spec.defaultBackend`.
    pub default_backend: Option<IngressBackend>,
    /// `spec.rules`.
    pub rules: Vec<IngressRule>,
    /// Hosts of `spec.tls`.
    pub tls_hosts: Vec<String>,
    /// IP of `status.loadBalancer.ingress`.
    pub addresses: Vec<Addr>,
}

/// One route of an ingress: a rule's host and path to a service port; the default backend
/// is the route without host nor path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IngressRoute {
    /// Host.
    pub host: Option<String>,
    /// Path.
    pub path: Option<String>,
    /// Path type; `None` for the default backend.
    pub path_type: Option<PathType>,
    /// Target service, in the ingress namespace.
    pub service: DnsLabel,
    /// Target port.
    pub port: PortRef,
}

/// The tracked fields of an ingress (019/X4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedIngress {
    /// Name.
    pub name: DnsSubdomain,
    /// UID.
    pub uid: Uuid,
    /// Class.
    pub class: Option<String>,
    /// Routes, as a set.
    pub routes: BTreeSet<IngressRoute>,
    /// TLS hosts, as a set.
    pub tls_hosts: BTreeSet<String>,
    /// Addresses, as a set.
    pub addresses: BTreeSet<Addr>,
}

impl ObservedIngress {
    /// The tracked fields.
    ///
    /// # Errors
    /// `kube.name.invalid` (ingress or target service name).
    pub fn project(&self) -> Result<TrackedIngress, Violation> {
        let route =
            |host: Option<&String>, path: Option<&IngressPath>, backend: &IngressBackend| {
                Ok::<_, Violation>(IngressRoute {
                    host: host.cloned(),
                    path: path.and_then(|p| p.path.clone()),
                    path_type: path.map(|p| p.path_type),
                    service: DnsLabel::parse(&backend.service)?,
                    port: backend.port.clone(),
                })
            };
        let mut routes = BTreeSet::new();
        if let Some(backend) = &self.default_backend {
            routes.insert(route(None, None, backend)?);
        }
        for rule in &self.rules {
            for path in &rule.paths {
                routes.insert(route(rule.host.as_ref(), Some(path), &path.backend)?);
            }
        }
        Ok(TrackedIngress {
            name: DnsSubdomain::parse(&self.name)?,
            uid: self.uid,
            class: self.class.clone(),
            routes,
            tls_hosts: self.tls_hosts.iter().cloned().collect(),
            addresses: self.addresses.iter().copied().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use kartenn_core::family::{IPV4, IPV6};

    use super::*;

    fn read(pairs: &[(&str, &str)]) -> ReadLabels {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn keys(keys: &[&str]) -> TrackedKeys {
        TrackedKeys::new(keys.iter().map(|k| LabelKey::parse(k).unwrap()))
    }

    fn port(port: u16, name: Option<&str>) -> DeclaredPort {
        DeclaredPort {
            port,
            protocol: Protocol::Tcp,
            name: name.map(str::to_owned),
        }
    }

    fn deployment(image: &str, replicas: u32, version: &str) -> ObservedWorkload {
        ObservedWorkload {
            kind: WorkloadKind::Deployment,
            name: "pay".into(),
            uid: Uuid::from_u128(1),
            replicas: Some(replicas),
            labels: read(&[("app", "pay"), ("release", version)]),
            template: PodTemplate {
                labels: read(&[("app", "pay"), ("tier", "db"), ("version", version)]),
                annotations: read(&[("deployed-at", version)]),
                service_account: Some("pay".into()),
                host_network: false,
                containers: vec![
                    Container {
                        name: "app".into(),
                        image: image.into(),
                        ports: vec![port(8080, Some("http"))],
                    },
                    Container {
                        name: "metrics".into(),
                        image: format!("{image}-metrics"),
                        ports: vec![port(9090, None)],
                    },
                ],
            },
        }
    }

    #[test]
    fn image_change_projects_to_nothing() {
        let tracked = keys(&["app", "tier"]);
        let before = deployment("registry.example/pay:1.0", 3, "1.0");
        let after = deployment("registry.example/pay:1.1", 5, "1.1");
        assert_eq!(before.project(&tracked), after.project(&tracked));
        let projected = before.project(&tracked).unwrap();
        assert_eq!(
            projected
                .labels
                .keys()
                .map(LabelKey::as_str)
                .collect::<Vec<_>>(),
            ["app", "tier"]
        );
        assert_eq!(projected.ports.len(), 2);
        assert_eq!(
            projected.service_account.as_ref().map(DnsSubdomain::as_str),
            Some("pay")
        );
        // Reordering the containers changes nothing either.
        let mut reordered = deployment("registry.example/pay:1.0", 3, "1.0");
        reordered.template.containers.reverse();
        assert_eq!(reordered.project(&tracked), before.project(&tracked));
        // A tracked field does change the projection: a tracked label, a port, hostNetwork,
        // the UID of a recreation.
        let mut relabelled = deployment("registry.example/pay:1.0", 3, "1.0");
        relabelled
            .template
            .labels
            .insert("tier".into(), "cache".into());
        let mut host = deployment("registry.example/pay:1.0", 3, "1.0");
        host.template.host_network = true;
        let mut recreated = deployment("registry.example/pay:1.0", 3, "1.0");
        recreated.uid = Uuid::from_u128(2);
        let mut ported = deployment("registry.example/pay:1.0", 3, "1.0");
        ported.template.containers[0].ports[0].port = 8443;
        for changed in [relabelled, host, recreated, ported] {
            assert_ne!(changed.project(&tracked), before.project(&tracked));
        }
        // Tracking the version label makes it a change.
        let with_version = keys(&["app", "tier", "version"]);
        assert_ne!(before.project(&with_version), after.project(&with_version));
    }

    #[test]
    fn projection_checks_what_it_keeps() {
        let mut bad = deployment("i", 1, "1.0");
        bad.name = "Pay".into();
        assert!(
            bad.project(&keys(&[]))
                .is_err_and(|v| v.rule.id == "kube.name.invalid")
        );
        let mut bad_label = deployment("i", 1, "1.0");
        bad_label.template.labels.insert("tier".into(), "-x".into());
        assert!(
            bad_label
                .project(&keys(&["tier"]))
                .is_err_and(|v| v.rule.id == "kube.label.invalid")
        );
        // An untracked label is dropped unchecked; an empty service account is none.
        let mut untracked = deployment("i", 1, "1.0");
        untracked
            .template
            .labels
            .insert("other".into(), "-x".into());
        untracked.template.service_account = Some(String::new());
        let projected = untracked.project(&keys(&["tier"])).unwrap();
        assert_eq!(projected.service_account, None);
        // A namespace keeps its tracked labels, `kubernetes.io/metadata.name` among them.
        let ns = ObservedNamespace {
            name: "payments".into(),
            uid: Uuid::from_u128(3),
            labels: read(&[
                ("kubernetes.io/metadata.name", "payments"),
                ("owner", "team-a"),
            ]),
        };
        let tracked = ns.project(&keys(&["kubernetes.io/metadata.name"])).unwrap();
        assert_eq!(tracked.labels.len(), 1);
        assert_eq!(tracked.name.as_str(), "payments");
    }

    #[test]
    fn node_service_ingress_projections() {
        let node = ObservedNode {
            name: "node-1".into(),
            uid: Uuid::from_u128(4),
            labels: read(&[(ZONE_LABEL, "zone-a"), ("kubernetes.io/hostname", "node-1")]),
            addresses: vec![
                NodeAddress {
                    kind: "Hostname".into(),
                    value: "node-1".into(),
                },
                NodeAddress {
                    kind: "InternalIP".into(),
                    value: "192.0.2.11".into(),
                },
            ],
            pod_cidrs: vec![
                Prefix::parse(IPV6, "2001:db8:64:3::/64").unwrap(),
                Prefix::parse(IPV4, "198.51.100.0/24").unwrap(),
            ],
        };
        let mut reordered = node.clone();
        reordered.addresses.reverse();
        reordered.pod_cidrs.reverse();
        assert_eq!(node.project(), reordered.project());
        let tracked = node.project().unwrap();
        assert_eq!(
            tracked.zone.as_ref().map(LabelValue::as_str),
            Some("zone-a")
        );
        assert_eq!(tracked.pod_ranges.len(), 2);

        let service = ObservedService {
            name: "pay".into(),
            uid: Uuid::from_u128(5),
            labels: read(&[("release", "1.0")]),
            service_type: ServiceType::ClusterIp,
            headless: false,
            selector: read(&[("app", "pay")]),
            ports: vec![ServicePort {
                port: 80,
                target: PortRef::Name("http".into()),
                protocol: Protocol::Tcp,
                app_protocol: Some("http".into()),
                node_port: None,
                name: Some("web".into()),
            }],
            cluster_ips: vec![Addr::parse(IPV4, "192.0.2.80").unwrap()],
            external_ips: Vec::new(),
            external_name: None,
        };
        let mut relabelled = service.clone();
        relabelled.labels = read(&[("release", "1.1")]);
        assert_eq!(service.project(), relabelled.project());
        assert_eq!(
            service.project().unwrap().service_type.as_str(),
            "cluster_ip"
        );
        // The selection reads every template label, tracked or not.
        assert_eq!(service.selects(&deployment("i", 1, "1.0")), Ok(true));
        let mut other = deployment("i", 1, "1.0");
        other.template.labels.insert("app".into(), "web".into());
        assert_eq!(service.selects(&other), Ok(false));
        let mut manual = service.clone();
        manual.selector.clear();
        assert_eq!(manual.selects(&deployment("i", 1, "1.0")), Ok(false));

        let backend = IngressBackend {
            service: "pay".into(),
            port: PortRef::Name("web".into()),
        };
        let ingress = ObservedIngress {
            name: "pay".into(),
            uid: Uuid::from_u128(6),
            annotations: read(&[("example.org/rewrite", "1")]),
            class: Some("public".into()),
            default_backend: Some(backend.clone()),
            rules: vec![IngressRule {
                host: Some("pay.example.org".into()),
                paths: vec![IngressPath {
                    path: Some("/api".into()),
                    path_type: PathType::Prefix,
                    backend: backend.clone(),
                }],
            }],
            tls_hosts: vec!["pay.example.org".into()],
            addresses: vec![Addr::parse(IPV4, "203.0.113.10").unwrap()],
        };
        let mut annotated = ingress.clone();
        annotated.annotations.clear();
        assert_eq!(ingress.project(), annotated.project());
        let tracked = ingress.project().unwrap();
        assert_eq!(tracked.routes.len(), 2);
        assert!(
            tracked
                .routes
                .iter()
                .any(|r| r.host.is_none() && r.path_type.is_none())
        );
        let mut bad = ingress.clone();
        bad.rules[0].paths[0].backend.service = "Pay".into();
        assert!(
            bad.project()
                .is_err_and(|v| v.rule.id == "kube.name.invalid")
        );
    }
}
