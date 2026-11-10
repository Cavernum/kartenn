//! The handler of the module in the write pipeline (feature `pg`; ADR 0026, clause 6):
//! the declaration of a cluster by an administrator (`create_cluster`, `update_cluster`,
//! REQ-019-5) and the batch of the connector (`sync`, 019/X9), one node, namespace,
//! workload, service or ingress of a cluster per operation, found by its identity in the
//! cluster, or in its namespace for a workload, a service or an ingress (REQ-019-6 to 10).
//! `owner` and `site` of a cluster are assignments of `core.owner` and `core.site` the
//! handler adds to the changeset; its other objects inherit them (tree `kube`) and are
//! anchored in one of its domains (design « Types d'objets »). The pod ranges of a node
//! (`pod_cidrs`, REQ-019-13) are networks of its pod domain, the cluster IPs and external
//! addresses of a service and the addresses of an ingress (REQ-019-14) addresses of usage
//! `vip` of its service and external domains, which the `sync` creates and closes as
//! operations of the core, cited by their owner once their identifiers come back
//! (`planned`, ADR 0026 amendment 8).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family;
use kartenn_core::objects::{AddressUsage, Nature, Slug};
use kartenn_core::period::Period;
use kartenn_core::rules::Violation;
use kartenn_service::ObjectKind;
use kartenn_service::module::{
    BoxFuture, CheckContext, Findings, ModuleHandler, ModuleObjectRow, ModulePlan, ModuleTx,
    Reference,
};
use kartenn_service::operations::{ModuleKind, ModuleOperation, Operation, RawInput, Ref};
use kartenn_service::port::{StoreError, WriteError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::module::{
    ADDRESS, CLUSTER, INGRESS, MACHINE, NAMESPACE, NODE, PREFIX, SERVICE, WORKLOAD,
};
use crate::names::{DnsLabel, DnsSubdomain, LabelKey, LabelValue, parse_labels};
use crate::owner::WorkloadKind;
use crate::pg::{
    self, CLUSTER_TABLE, INGRESS_TABLE, NAMESPACE_TABLE, NODE_TABLE, SERVICE_TABLE, WORKLOAD_TABLE,
};
use crate::rules::{
    CLUSTER_CONNECTOR_GROUP, CLUSTER_RANGE_DOMAIN, INGRESS_BACKEND_MISSING, INGRESS_DUPLICATE,
    NAME_INVALID, NODE_MACHINE_KIND, NODE_RANGE_OUTSIDE, OPERATION_INVALID, SERVICE_DUPLICATE,
    WORKLOAD_DUPLICATE,
};
use crate::tracked::{PathType, Protocol, ServiceType};

/// The handler of the module.
#[derive(Debug, Clone, Copy, Default)]
pub struct Handler;

fn kind(name: &'static str, cell: &'static OnceLock<ModuleKind>) -> ModuleKind {
    *cell.get_or_init(|| ModuleKind::parse(name).expect("a valid kind"))
}

fn cluster_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(CLUSTER, &KIND)
}

fn node_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(NODE, &KIND)
}

fn namespace_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(NAMESPACE, &KIND)
}

fn workload_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(WORKLOAD, &KIND)
}

fn service_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(SERVICE, &KIND)
}

fn ingress_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(INGRESS, &KIND)
}

/// The table of the objects of `kind`.
fn table(kind: ModuleKind) -> &'static str {
    if kind == node_kind() {
        NODE_TABLE
    } else if kind == namespace_kind() {
        NAMESPACE_TABLE
    } else if kind == workload_kind() {
        WORKLOAD_TABLE
    } else if kind == service_kind() {
        SERVICE_TABLE
    } else if kind == ingress_kind() {
        INGRESS_TABLE
    } else {
        CLUSTER_TABLE
    }
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
}

fn invalid(message: impl Into<String>) -> Violation {
    Violation::new(&OPERATION_INVALID, message.into())
}

/// The lock key of the writes of a cluster: two batches of a cluster do not cross
/// (design « Opérations »).
fn cluster_key(cluster: Uuid) -> String {
    format!("kube.cluster:{cluster}")
}

/// What [`Handler::prepare`] reads for the plan, which reads no database.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Completed {
    /// `core.owner` and `core.site`, which a cluster is assigned.
    #[serde(default)]
    owner_property: Option<Uuid>,
    #[serde(default)]
    site_property: Option<Uuid>,
    /// The living object of the identity a `sync` names (019/X9).
    #[serde(default)]
    existing: Option<Uuid>,
    /// The living namespace a `sync` of a workload or a service names, by its name in the
    /// cluster.
    #[serde(default)]
    namespace: Option<Uuid>,
    /// The last closed workload or service of the identity a `sync` names, when none
    /// lives: the one an `upsert` opens again (019/X3).
    #[serde(default)]
    reopened: Option<Uuid>,
    /// The service domain and the external domain of the cluster of a `sync` of a
    /// service or an ingress: where its cluster IPs and its external addresses live
    /// (019/X6); the external addresses of a cluster without an external domain live in
    /// its service domain.
    #[serde(default)]
    service_domain: Option<Uuid>,
    #[serde(default)]
    external_domain: Option<Uuid>,
    /// The living addresses the service or the ingress of a `sync` cites.
    #[serde(default)]
    addresses: Vec<pg::HeldAddress>,
    /// The living services that select the workload a `sync` closes: it leaves their
    /// selection in the same batch (design « Références »).
    #[serde(default)]
    selecting: Vec<Uuid>,
    /// The service, the workload and the domain the service is anchored in, of an
    /// `unselect`, which only the handler plans (a `completed` sent by a client is
    /// replaced by `prepare`).
    #[serde(default)]
    unselect: Option<(Uuid, Uuid, Uuid)>,
    /// The living workloads of the namespace among those a `sync` of a service selects:
    /// kind, name, identifier.
    #[serde(default)]
    selection: Vec<(String, String, Uuid)>,
    /// The pod domain of the cluster of a `sync`: the anchor of its objects.
    #[serde(default)]
    pod_domain: Option<Uuid>,
    /// The pod containers of the cluster of a `sync`, as prefixes (task 019-3c).
    #[serde(default)]
    pod_containers: Vec<String>,
    /// The networks the node of a `sync` cites, with their prefixes.
    #[serde(default)]
    ranges: Vec<(String, Uuid)>,
}

fn decode<T: serde::de::DeserializeOwned>(value: Value, field: &str) -> Result<T, Violation> {
    serde_json::from_value(value).map_err(|e| invalid(format!("{field}: {e}")))
}

/// The body of `operation`, refused if it carries a field out of `fields`.
fn body(operation: &ModuleOperation, fields: &[&str]) -> Result<Map<String, Value>, Violation> {
    if let Some(unknown) = operation
        .body
        .keys()
        .find(|k| !fields.contains(&k.as_str()) && k.as_str() != "completed")
    {
        return Err(invalid(format!(
            "{}: unknown field `{unknown}`",
            operation.tag()
        )));
    }
    Ok(operation.body.clone())
}

fn completed(body: &Map<String, Value>) -> Completed {
    body.get("completed")
        .cloned()
        .and_then(|c| serde_json::from_value(c).ok())
        .unwrap_or_default()
}

/// The reference `value` resolved.
fn reference(
    value: &Value,
    field: &str,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<Uuid, Violation> {
    let r: Ref = decode(value.clone(), field)?;
    resolve(r).ok_or_else(|| invalid(format!("{field}: unknown object")))
}

/// The reference `value` resolved, `null` for none.
fn optional_reference(
    value: &Value,
    field: &str,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<Option<Uuid>, Violation> {
    if value.is_null() {
        Ok(None)
    } else {
        reference(value, field, resolve).map(Some)
    }
}

/// The references of the array `value`, as a set.
fn references(
    value: &Value,
    field: &str,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<Vec<Uuid>, Violation> {
    let refs: Vec<Ref> = decode(value.clone(), field)?;
    let ids: BTreeSet<Uuid> = refs
        .into_iter()
        .map(|r| resolve(r).ok_or_else(|| invalid(format!("{field}: unknown object"))))
        .collect::<Result<_, _>>()?;
    Ok(ids.into_iter().collect())
}

/// An integer of `field` within `bounds`.
fn bounded(value: &Value, field: &str, bounds: (i64, i64)) -> Result<i64, Violation> {
    value
        .as_i64()
        .filter(|n| (bounds.0..=bounds.1).contains(n))
        .ok_or_else(|| {
            invalid(format!(
                "{field}: an integer from {} to {}",
                bounds.0, bounds.1
            ))
        })
}

/// The assignment of the reference `value` to the property `property` of `object`.
fn assign(kind: ModuleKind, object: Uuid, property: Uuid, value: Uuid) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(object),
        kind: ObjectKind::Module(kind),
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.to_string())],
    }
}

/// What an operation does to the site of a cluster.
#[derive(Debug, Clone, Copy)]
enum Site {
    /// Nothing.
    Keep,
    /// Assigns it.
    Set(Uuid),
    /// Removes it (`null`).
    Remove,
}

impl Site {
    /// The site of `body`.
    fn of(
        body: &Map<String, Value>,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<Self, Violation> {
        Ok(match body.get("site") {
            None => Self::Keep,
            Some(v) => optional_reference(v, "site", resolve)?.map_or(Self::Remove, Self::Set),
        })
    }
}

/// The columns an operation writes, and the rights it needs on what it cites.
type Columns = (Map<String, Value>, Vec<(&'static str, Uuid)>);

/// The operations of the core for the `owner` and `site` of a cluster: an assignment for
/// a value, a removal for a `null` site.
fn anchors(
    object: Uuid,
    owner: Option<Uuid>,
    site: Site,
    completed: &Completed,
) -> Result<Vec<Operation>, Violation> {
    let undeclared = |key: &str| invalid(format!("{key} is not declared"));
    let mut core = Vec::new();
    if let Some(owner) = owner {
        let property = completed
            .owner_property
            .ok_or_else(|| undeclared("core.owner"))?;
        core.push(assign(cluster_kind(), object, property, owner));
    }
    if !matches!(site, Site::Keep) {
        let property = completed
            .site_property
            .ok_or_else(|| undeclared("core.site"))?;
        core.push(match site {
            Site::Set(site) => assign(cluster_kind(), object, property, site),
            _ => Operation::UnassignProperty {
                object: Ref::Id(object),
                kind: ObjectKind::Module(cluster_kind()),
                property: Ref::Id(property),
            },
        });
    }
    Ok(core)
}

/// The rights a plan needs on the objects it cites (`plan.data.reads`).
fn reads(pairs: &[(&str, Uuid)]) -> Value {
    Value::Array(
        pairs
            .iter()
            .map(|(permission, id)| json!([permission, id]))
            .collect(),
    )
}

/// The fields of a cluster an administrator writes, beside `owner` and `site`.
const CLUSTER_FIELDS: &[&str] = &[
    "slug",
    "pod_domain",
    "service_domain",
    "external_domain",
    "pod_containers",
    "service_networks",
    "machines_cluster",
    "retention_days",
    "tracked_label_keys",
    "batch_window_seconds",
    "batch_max_operations",
    "create_node_machines",
    "connector_group",
];

/// The columns of the cluster fields of `body` (REQ-019-5), and the rights they need.
fn cluster_columns(
    body: &Map<String, Value>,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<Columns, Violation> {
    let mut columns = Map::new();
    let mut cited = Vec::new();
    for (field, value) in body {
        let (column, value) = match field.as_str() {
            "slug" => {
                let slug = value.as_str().ok_or_else(|| invalid("slug: a string"))?;
                ("slug", json!(Slug::parse(slug)?.as_str()))
            }
            "pod_domain" | "service_domain" => {
                if value.is_null() {
                    return Err(invalid(format!("{field}: a cluster keeps it")));
                }
                let domain = reference(value, field, resolve)?;
                cited.push(("core.domain.read", domain));
                let column = if field == "pod_domain" {
                    "pod_domain_id"
                } else {
                    "service_domain_id"
                };
                (column, json!(domain))
            }
            "external_domain" => {
                let domain = optional_reference(value, field, resolve)?;
                cited.extend(domain.map(|d| ("core.domain.read", d)));
                ("external_domain_id", json!(domain))
            }
            "pod_containers" | "service_networks" => {
                let ranges = references(value, field, resolve)?;
                cited.extend(ranges.iter().map(|r| ("core.prefix.read", *r)));
                let column = if field == "pod_containers" {
                    "pod_container_ids"
                } else {
                    "service_network_ids"
                };
                (column, json!(ranges))
            }
            "machines_cluster" => {
                let cluster = optional_reference(value, field, resolve)?;
                cited.extend(cluster.map(|c| ("machines.cluster.read", c)));
                ("machines_cluster_id", json!(cluster))
            }
            "connector_group" => (
                "connector_group_id",
                json!(optional_reference(value, field, resolve)?),
            ),
            "retention_days" => ("retention_days", json!(bounded(value, field, (1, 400))?)),
            "batch_window_seconds" => (
                "batch_window_seconds",
                json!(bounded(value, field, (1, 300))?),
            ),
            "batch_max_operations" => (
                "batch_max_operations",
                json!(bounded(value, field, (1, 10_000))?),
            ),
            "tracked_label_keys" => {
                let keys: Vec<String> = decode(value.clone(), field)?;
                let keys: BTreeSet<String> = keys
                    .iter()
                    .map(|k| LabelKey::parse(k).map(|k| k.as_str().to_owned()))
                    .collect::<Result<_, _>>()?;
                ("tracked_label_keys", json!(keys))
            }
            "create_node_machines" => (
                "create_node_machines",
                json!(
                    value
                        .as_bool()
                        .ok_or_else(|| invalid("create_node_machines: a boolean"))?
                ),
            ),
            _ => continue,
        };
        columns.insert(column.into(), value);
    }
    Ok((columns, cited))
}

/// The defaults of a new cluster (design « Données »).
fn cluster_defaults() -> Map<String, Value> {
    let Value::Object(defaults) = json!({
        "external_domain_id": null,
        "pod_container_ids": [],
        "service_network_ids": [],
        "machines_cluster_id": null,
        "retention_days": 30,
        "tracked_label_keys": [],
        "batch_window_seconds": 10,
        "batch_max_operations": 500,
        "create_node_machines": false,
        "connector_group_id": null,
    }) else {
        unreachable!("an object literal")
    };
    defaults
}

/// The fields `sync` takes for each type, beside the identity.
const NODE_FIELDS: &[&str] = &["uid", "addresses", "zone", "machine", "pod_cidrs"];
const NAMESPACE_FIELDS: &[&str] = &["uid", "labels"];
/// A workload is named in its namespace (`namespace`, by name) with its `kind`.
const WORKLOAD_FIELDS: &[&str] = &[
    "namespace",
    "kind",
    "uid",
    "labels",
    "service_account",
    "ports",
    "host_network",
];

/// A service is named in its namespace (`namespace`, by name); its Kubernetes type is
/// `service_type` (`type` names the type of object of the `sync`), its addresses are lists
/// of IP (`cluster_ips`, `external_ips`), its selection a list of workloads of the
/// namespace by kind and name (`workloads`).
const SERVICE_FIELDS: &[&str] = &[
    "namespace",
    "uid",
    "service_type",
    "headless",
    "external_name",
    "ports",
    "cluster_ips",
    "external_ips",
    "workloads",
];

/// An ingress is named in its namespace (`namespace`, by name); `class` is its
/// `ingressClassName`, `rules` its routes (the default backend is the route without host
/// nor path), `addresses` the IP of its `status.loadBalancer.ingress`.
const INGRESS_FIELDS: &[&str] = &[
    "namespace",
    "uid",
    "class",
    "rules",
    "tls_hosts",
    "addresses",
];

/// The fields of the types `sync` takes, beside the identity.
const ALL_FIELDS: [&[&str]; 5] = [
    NODE_FIELDS,
    NAMESPACE_FIELDS,
    WORKLOAD_FIELDS,
    SERVICE_FIELDS,
    INGRESS_FIELDS,
];

/// The fields `sync` takes for the type `kind`.
fn fields_of(kind: ModuleKind) -> &'static [&'static str] {
    if kind == node_kind() {
        NODE_FIELDS
    } else if kind == workload_kind() {
        WORKLOAD_FIELDS
    } else if kind == service_kind() {
        SERVICE_FIELDS
    } else if kind == ingress_kind() {
        INGRESS_FIELDS
    } else {
        NAMESPACE_FIELDS
    }
}

/// A port declared by the containers of a workload (`containers[].ports`, REQ-019-8):
/// declared, never proven listening.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkloadPort {
    port: u16,
    /// `tcp` when absent, as in Kubernetes.
    #[serde(default = "tcp")]
    protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

fn tcp() -> String {
    Protocol::Tcp.as_str().to_owned()
}

/// Whether `name` is a port name of Kubernetes (`IANA_SVC_NAME`, RFC 6335 § 5.1, in lower
/// case): 1 to 15 letters, digits and hyphens, at least one letter, no hyphen at either
/// end nor two in a row.
fn port_name(name: &str) -> bool {
    (1..=15).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && name.bytes().any(|b| b.is_ascii_lowercase())
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

/// A port number of `field`, from 1 to 65535.
fn port_number(port: u16, field: &str) -> Result<(), Violation> {
    if port == 0 {
        return Err(invalid(format!("{field}: a port from 1 to 65535")));
    }
    Ok(())
}

/// A port name of `field` (`IANA_SVC_NAME`).
fn checked_port_name(name: &str, field: &str) -> Result<(), Violation> {
    if !port_name(name) {
        return Err(Violation::new(
            &NAME_INVALID,
            format!("{field}: {name} is not a port name (IANA_SVC_NAME)"),
        ));
    }
    Ok(())
}

/// The number, protocol and name of a port of a workload or a service.
fn checked_port(port: u16, protocol: &str, name: Option<&str>) -> Result<(), Violation> {
    port_number(port, "ports")?;
    if Protocol::parse(protocol).is_none() {
        return Err(invalid(format!(
            "ports: {protocol} is not tcp, udp or sctp"
        )));
    }
    name.map_or(Ok(()), |n| checked_port_name(n, "ports"))
}

/// The declared ports of `value`, a sorted set.
fn workload_ports(value: &Value) -> Result<Vec<WorkloadPort>, Violation> {
    let ports: Vec<WorkloadPort> = decode(value.clone(), "ports")?;
    for port in &ports {
        checked_port(port.port, &port.protocol, port.name.as_deref())?;
    }
    // A set: a reordering makes no version (019/X4).
    let ports: BTreeSet<WorkloadPort> = ports.into_iter().collect();
    Ok(ports.into_iter().collect())
}

/// The port a service targets on its pods (`targetPort`): a number or a port name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(untagged)]
enum TargetPort {
    Number(u16),
    Name(String),
}

/// A port of a service (`spec.ports`, REQ-019-9).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ServicePort {
    port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_port: Option<TargetPort>,
    /// `tcp` when absent, as in Kubernetes.
    #[serde(default = "tcp")]
    protocol: String,
    /// `appProtocol`, in the syntax of a label key (an IANA service name, or a prefixed
    /// name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    app_protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node_port: Option<u16>,
    /// The name an ingress may target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// The ports of a service in `value`, a sorted set.
fn service_ports(value: &Value) -> Result<Vec<ServicePort>, Violation> {
    let ports: Vec<ServicePort> = decode(value.clone(), "ports")?;
    for port in &ports {
        checked_port(port.port, &port.protocol, port.name.as_deref())?;
        match &port.target_port {
            Some(TargetPort::Number(n)) => port_number(*n, "target_port")?,
            Some(TargetPort::Name(n)) => checked_port_name(n, "target_port")?,
            None => {}
        }
        if let Some(n) = port.node_port {
            port_number(n, "node_port")?;
        }
        if let Some(app) = &port.app_protocol {
            LabelKey::parse(app)?;
        }
    }
    // A set: a reordering makes no version (019/X4).
    let ports: BTreeSet<ServicePort> = ports.into_iter().collect();
    Ok(ports.into_iter().collect())
}

/// A workload a `sync` of a service selects, by its kind and name in the namespace of the
/// service: a workload of the pods of its `EndpointSlice`s (design « Connecteur »).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selected {
    kind: String,
    name: String,
}

/// The workloads a `sync` of a service selects (`workloads`), by kind and name, as
/// [`Handler::prepare`] looks for them; none if malformed (refused by the plan).
fn selected(body: &Map<String, Value>) -> Vec<(String, String)> {
    body.get("workloads")
        .cloned()
        .and_then(|w| serde_json::from_value::<Vec<Selected>>(w).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|s| (s.kind, s.name))
        .collect()
}

/// The selection of `value`: the living workloads of the namespace it names, among those
/// `prepare` found (`completed.selection`), as a sorted set.
fn selection(value: &Value, completed: &Completed) -> Result<Vec<Uuid>, Violation> {
    let wanted: Vec<Selected> = decode(value.clone(), "workloads")?;
    let ids: BTreeSet<Uuid> = wanted
        .iter()
        .map(|s| {
            completed
                .selection
                .iter()
                .find(|(kind, name, _)| *kind == s.kind && *name == s.name)
                .map(|(_, _, id)| *id)
                .ok_or_else(|| {
                    invalid(format!(
                        "workloads: no living {} {} in the namespace of the service",
                        s.kind, s.name
                    ))
                })
        })
        .collect::<Result<_, _>>()?;
    Ok(ids.into_iter().collect())
}

/// A route of an ingress (`spec.rules[].http.paths[]`, `spec.defaultBackend`, REQ-019-10):
/// the host and the path of a rule to a service of the namespace, named, and one of its
/// ports, by number or by name; the default backend is the route without host, path nor
/// path type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Route {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// `exact`, `prefix` or `implementation_specific` (`pathType`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path_type: Option<String>,
    /// The name of a service of the namespace: not a reference, the service may be absent
    /// (`kube.ingress.backend_missing` warns).
    service: String,
    port: TargetPort,
}

/// A host of an ingress in `field` (a rule, a TLS host): a DNS subdomain whose first label
/// may be `*` (a wildcard), not an IP address (Kubernetes, `IngressRule.host`).
fn checked_host(name: &str, field: &str) -> Result<(), Violation> {
    if Addr::parse_any(&family::ALL, name).is_ok() {
        return Err(Violation::new(
            &NAME_INVALID,
            format!("{field}: {name} is an IP address, not a host"),
        ));
    }
    DnsSubdomain::parse(name.strip_prefix("*.").unwrap_or(name)).map(drop)
}

/// The class of an ingress in `value` (`ingressClassName`, the name of an `IngressClass`),
/// `null` for none.
fn ingress_class(value: &Value) -> Result<Option<String>, Violation> {
    match value.as_str() {
        Some(class) => Ok(Some(DnsSubdomain::parse(class)?.as_str().to_owned())),
        None if value.is_null() => Ok(None),
        None => Err(invalid("class: a string")),
    }
}

/// The TLS hosts of an ingress in `value` (`spec.tls[].hosts`), a sorted set.
fn tls_hosts(value: &Value) -> Result<BTreeSet<String>, Violation> {
    let hosts: Vec<String> = decode(value.clone(), "tls_hosts")?;
    for host in &hosts {
        checked_host(host, "tls_hosts")?;
    }
    // A set: a reordering makes no version (019/X4).
    Ok(hosts.into_iter().collect())
}

/// The routes of an ingress in `value`, a sorted set: a path begins with `/`, a path of
/// type `exact` or `prefix` is given, only the default backend has no path type.
fn routes(value: &Value) -> Result<Vec<Route>, Violation> {
    let routes: Vec<Route> = decode(value.clone(), "rules")?;
    for route in &routes {
        if let Some(host) = &route.host {
            checked_host(host, "rules")?;
        }
        let path_type = match route.path_type.as_deref() {
            Some(text) => Some(PathType::parse(text).ok_or_else(|| {
                invalid(format!(
                    "rules: {text} is not exact, prefix or implementation_specific"
                ))
            })?),
            None => None,
        };
        match (route.path.as_deref(), path_type) {
            (Some(path), Some(_)) if !path.starts_with('/') => {
                return Err(invalid(format!(
                    "rules: the path {path} does not begin with /"
                )));
            }
            (None, Some(PathType::Exact | PathType::Prefix)) => {
                return Err(invalid("rules: an exact or prefix path type takes a path"));
            }
            (Some(_), None) => return Err(invalid("rules: a path takes its path type")),
            (None, None) if route.host.is_some() => {
                return Err(invalid(
                    "rules: a route of a host takes a path type; only the default backend has none",
                ));
            }
            _ => {}
        }
        DnsLabel::parse(&route.service)?;
        match &route.port {
            TargetPort::Number(n) => port_number(*n, "rules")?,
            TargetPort::Name(n) => checked_port_name(n, "rules")?,
        }
    }
    // A set: a reordering makes no version (019/X4).
    let routes: BTreeSet<Route> = routes.into_iter().collect();
    Ok(routes.into_iter().collect())
}

/// One sort of addresses of a service or an ingress (REQ-019-14): its field in a `sync`,
/// its column, and the domain its addresses live in.
struct Sort {
    field: &'static str,
    column: &'static str,
    cluster_ip: bool,
    domain: Option<Uuid>,
}

/// An address a service cites, as [`vips`] reads it.
struct Held {
    cluster_ip: bool,
    external: bool,
    domain: Uuid,
    value: Addr,
    id: Uuid,
}

/// The sorts of addresses of an object of `kind`: the cluster IPs and the external
/// addresses of a service, the addresses of an ingress, external (REQ-019-14). Without an
/// external domain, the external addresses live in the service domain (design « Données »,
/// 2026-11-10).
fn sorts(kind: ModuleKind, completed: &Completed) -> Vec<Sort> {
    let external = completed.external_domain.or(completed.service_domain);
    if kind == service_kind() {
        vec![
            Sort {
                field: "cluster_ips",
                column: "cluster_address_ids",
                cluster_ip: true,
                domain: completed.service_domain,
            },
            Sort {
                field: "external_ips",
                column: "external_address_ids",
                cluster_ip: false,
                domain: external,
            },
        ]
    } else if kind == ingress_kind() {
        vec![Sort {
            field: "addresses",
            column: "address_ids",
            cluster_ip: false,
            domain: external,
        }]
    } else {
        Vec::new()
    }
}

/// What a `sync` does to the addresses of a service or an ingress, into `columns`: the
/// addresses of each of its `sorts` it gives (`cluster_ips`, `external_ips`, `addresses`,
/// lists of IP) against those the object cites (`completed.addresses`). A value unchanged
/// creates nothing; a new value gets one address of usage `vip` in the domain of its
/// sort, cited by every sort that gives it (a cluster IP that is also an external address,
/// when the two domains are one, is one address); an address no sort cites any more
/// closes. Returns the operations of the core and, for each, the columns that cite what it
/// creates (`data.slots`).
fn vips(
    body: &Map<String, Value>,
    completed: &Completed,
    sorts: &[Sort],
    columns: &mut Map<String, Value>,
) -> Result<(Vec<Operation>, Vec<Value>), Violation> {
    let held: Vec<Held> = completed
        .addresses
        .iter()
        .filter_map(|(cluster_ip, external, domain, text, id)| {
            Some(Held {
                cluster_ip: *cluster_ip,
                external: *external,
                domain: *domain,
                value: Addr::parse_any(&family::ALL, text).ok()?,
                id: *id,
            })
        })
        .collect();
    let found = |key: &(Uuid, Addr)| {
        held.iter()
            .find(|h| (h.domain, h.value) == *key)
            .map(|h| h.id)
    };
    // The (domain, value) each sort cites after the write: those it gives, or those it
    // cites already.
    let mut wanted: Vec<(&Sort, BTreeSet<(Uuid, Addr)>)> = Vec::new();
    for sort in sorts {
        let Some(value) = body.get(sort.field) else {
            let cited = held
                .iter()
                .filter(|h| {
                    if sort.cluster_ip {
                        h.cluster_ip
                    } else {
                        h.external
                    }
                })
                .map(|h| (h.domain, h.value))
                .collect();
            wanted.push((sort, cited));
            continue;
        };
        let field = sort.field;
        let texts: Vec<String> = decode(value.clone(), field)?;
        let set: BTreeSet<(Uuid, Addr)> = texts
            .iter()
            .map(|t| {
                let value = Addr::parse_any(&family::ALL, t)
                    .map_err(|e| invalid(format!("{field}: {t}: {e}")))?;
                let domain = sort.domain.ok_or_else(|| {
                    invalid(format!(
                        "{field}: the cluster designates no domain for them"
                    ))
                })?;
                Ok((domain, value))
            })
            .collect::<Result<_, Violation>>()?;
        let mut kept: Vec<Uuid> = set.iter().filter_map(found).collect();
        kept.sort_unstable();
        columns.insert(sort.column.into(), json!(kept));
        wanted.push((sort, set));
    }
    let all: BTreeSet<(Uuid, Addr)> = wanted.iter().flat_map(|(_, s)| s.iter().copied()).collect();
    let mut core = Vec::new();
    let mut slots = Vec::new();
    for key in all.iter().filter(|k| found(k).is_none()) {
        let citing: Vec<&str> = wanted
            .iter()
            .filter(|(_, s)| s.contains(key))
            .map(|(sort, _)| sort.column)
            .collect();
        core.push(Operation::CreateAddress {
            local: None,
            domain: Ref::Id(key.0),
            address: key.1.to_string(),
            usage: AddressUsage::Vip,
        });
        slots.push(json!(citing));
    }
    for h in held.iter().filter(|h| !all.contains(&(h.domain, h.value))) {
        core.push(close_core(ObjectKind::Address, h.id));
        slots.push(Value::Null);
    }
    Ok((core, slots))
}

/// The columns of the object of a plan that cite what `operation` creates, if it creates
/// something (`data.slots`, read by [`ModuleHandler::planned`]).
fn slot(operation: &Operation, columns: &[&str]) -> Value {
    match operation {
        Operation::CreatePrefix { .. } | Operation::CreateAddress { .. } => json!(columns),
        _ => Value::Null,
    }
}

/// What a `sync` does to the pod ranges of a node (REQ-019-13).
struct PodRanges {
    /// The ranges as read, a sorted set (`pod_cidrs`).
    cidrs: Vec<String>,
    /// The networks the node keeps citing: those of the ranges it still holds.
    kept: Vec<Uuid>,
    /// A network created in the pod domain for each new range inside a pod container of
    /// the cluster, a closure for each network of a range the node no longer holds; the
    /// identifiers of the created ones come back by [`ModuleHandler::planned`].
    core: Vec<Operation>,
}

/// A prefix of `field`, strictly parsed (no host bits, REQ-002-5).
fn cidr(text: &str, field: &str) -> Result<Prefix, Violation> {
    Prefix::parse_any(&family::ALL, text).map_err(|e| invalid(format!("{field}: {text}: {e}")))
}

/// The ranges of `value` (`spec.podCIDRs`) against the networks the node cites and the pod
/// containers of its cluster (`completed`): a range unchanged creates nothing, a new one
/// gets a network of `domain` if a container strictly contains it (else none, and
/// `kube.node.range_outside` warns), a range gone closes its network.
fn pod_ranges(value: &Value, completed: &Completed, domain: Uuid) -> Result<PodRanges, Violation> {
    let texts: Vec<String> = decode(value.clone(), "pod_cidrs")?;
    let wanted: BTreeSet<Prefix> = texts
        .iter()
        .map(|t| cidr(t, "pod_cidrs"))
        .collect::<Result<_, _>>()?;
    let held: Vec<(Prefix, Uuid)> = completed
        .ranges
        .iter()
        .filter_map(|(t, id)| Some((cidr(t, "ranges").ok()?, *id)))
        .collect();
    let containers: Vec<Prefix> = completed
        .pod_containers
        .iter()
        .filter_map(|t| cidr(t, "pod_containers").ok())
        .collect();
    let mut kept: Vec<Uuid> = held
        .iter()
        .filter(|(p, _)| wanted.contains(p))
        .map(|(_, id)| *id)
        .collect();
    kept.sort_unstable();
    let mut core: Vec<Operation> = wanted
        .iter()
        .filter(|p| !held.iter().any(|(h, _)| h == *p))
        .filter(|p| containers.iter().any(|c| c.strictly_contains(**p)))
        .map(|p| Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(domain),
            prefix: p.to_string(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        })
        .collect();
    core.extend(
        held.iter()
            .filter(|(p, _)| !wanted.contains(p))
            .map(|(_, id)| close_core(ObjectKind::Prefix, *id)),
    );
    Ok(PodRanges {
        cidrs: wanted.iter().map(ToString::to_string).collect(),
        kept,
        core,
    })
}

/// The closure of the object `id` of the core: the network of a pod range, an address of
/// a service.
fn close_core(kind: ObjectKind, id: Uuid) -> Operation {
    Operation::Close {
        kind,
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    }
}

/// An address of a node, as read (`status.addresses`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeAddress {
    #[serde(rename = "type")]
    kind: String,
    value: String,
}

/// The columns of the tracked fields of a node or a namespace in `body`.
fn mirror_columns(
    kind: ModuleKind,
    body: &Map<String, Value>,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<Columns, Violation> {
    let fields = fields_of(kind);
    let mut columns = Map::new();
    let mut cited = Vec::new();
    for (field, value) in body {
        if !fields.contains(&field.as_str()) {
            continue;
        }
        let (column, value) = match field.as_str() {
            "uid" => {
                let uid: Option<Uuid> = decode(value.clone(), field)?;
                ("uid", json!(uid))
            }
            "addresses" if kind == node_kind() => {
                let addresses: Vec<NodeAddress> = decode(value.clone(), field)?;
                if addresses
                    .iter()
                    .any(|a| a.kind.is_empty() || a.value.is_empty())
                {
                    return Err(invalid("addresses: a type and a value each"));
                }
                // A set: a reordering makes no version (019/X4).
                let addresses: BTreeSet<NodeAddress> = addresses.into_iter().collect();
                ("addresses", json!(addresses))
            }
            "zone" => {
                let zone = match value.as_str() {
                    Some(zone) => Some(LabelValue::parse(zone)?.as_str().to_owned()),
                    None if value.is_null() => None,
                    None => return Err(invalid("zone: a string")),
                };
                ("zone", json!(zone))
            }
            "machine" => {
                let machine = optional_reference(value, field, resolve)?;
                cited.extend(machine.map(|m| ("machines.read", m)));
                ("machine_id", json!(machine))
            }
            "labels" => {
                let labels: std::collections::BTreeMap<String, String> =
                    decode(value.clone(), field)?;
                let labels = parse_labels(labels.iter().map(|(k, v)| (k.as_str(), v.as_str())))?;
                let labels: Map<String, Value> = labels
                    .iter()
                    .map(|(k, v)| (k.as_str().to_owned(), json!(v.as_str())))
                    .collect();
                ("labels", Value::Object(labels))
            }
            "service_account" => {
                let account = match value.as_str() {
                    Some(account) => Some(DnsSubdomain::parse(account)?.as_str().to_owned()),
                    None if value.is_null() => None,
                    None => return Err(invalid("service_account: a string")),
                };
                ("service_account", json!(account))
            }
            "ports" if kind == service_kind() => ("ports", json!(service_ports(value)?)),
            "ports" => ("ports", json!(workload_ports(value)?)),
            "service_type" => {
                let text = value.as_str().unwrap_or_default();
                let service = ServiceType::parse(text).ok_or_else(|| {
                    invalid(format!(
                        "service_type: {value} is not cluster_ip, node_port, load_balancer or \
                         external_name"
                    ))
                })?;
                ("type", json!(service.as_str()))
            }
            "headless" => {
                let headless = value
                    .as_bool()
                    .ok_or_else(|| invalid("headless: a boolean"))?;
                ("headless", json!(headless))
            }
            "external_name" => {
                // A DNS name, absolute (a final dot) or not.
                let name = match value.as_str() {
                    Some(name) => {
                        DnsSubdomain::parse(name.strip_suffix('.').unwrap_or(name))?;
                        Some(name)
                    }
                    None if value.is_null() => None,
                    None => return Err(invalid("external_name: a string")),
                };
                ("external_name", json!(name))
            }
            "host_network" => {
                let host = value
                    .as_bool()
                    .ok_or_else(|| invalid("host_network: a boolean"))?;
                ("host_network", json!(host))
            }
            "class" => ("class", json!(ingress_class(value)?)),
            "rules" => ("rules", json!(routes(value)?)),
            "tls_hosts" => ("tls_hosts", json!(tls_hosts(value)?)),
            // The identity of a workload (`namespace`, `kind`), not a column it changes; the
            // addresses of an ingress, written by `owned_columns`.
            _ => continue,
        };
        columns.insert(column.into(), value);
    }
    Ok((columns, cited))
}

/// The kind of the type a `sync` names.
fn mirror_kind(body: &Map<String, Value>) -> Result<ModuleKind, Violation> {
    match body.get("type").and_then(Value::as_str) {
        Some(NODE) => Ok(node_kind()),
        Some(NAMESPACE) => Ok(namespace_kind()),
        Some(WORKLOAD) => Ok(workload_kind()),
        Some(SERVICE) => Ok(service_kind()),
        Some(INGRESS) => Ok(ingress_kind()),
        Some(other) => Err(invalid(format!("type: {other} is not synced yet"))),
        None => Err(invalid("type: required")),
    }
}

/// The name a `sync` names, checked for its type: a DNS subdomain for a node, a workload or
/// an ingress, a DNS label for a namespace or a service (réf. `kubernetes-calcul.md`
/// § Namespace, labels, noms et UID).
fn mirror_name(kind: ModuleKind, body: &Map<String, Value>) -> Result<String, Violation> {
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("name: required"))?;
    Ok(
        if kind == node_kind() || kind == workload_kind() || kind == ingress_kind() {
            DnsSubdomain::parse(name)?.as_str().to_owned()
        } else {
            DnsLabel::parse(name)?.as_str().to_owned()
        },
    )
}

/// Where the object of a `sync` lives: in its cluster (a node, a namespace), or in a
/// namespace (a workload, of its kind, a service or an ingress).
#[derive(Debug, Clone, Copy)]
enum Home {
    Cluster(Uuid),
    Workload(Uuid, WorkloadKind),
    Service(Uuid),
    Ingress(Uuid),
}

/// The row of a new object `object` of `kind` named `name` in `home`, every tracked field
/// at its default.
fn mirror_row(kind: ModuleKind, object: Uuid, home: Home, name: &str) -> Map<String, Value> {
    let mut row = Map::new();
    row.insert("id".into(), json!(object));
    match home {
        Home::Cluster(cluster) => row.insert("cluster_id".into(), json!(cluster)),
        Home::Workload(namespace, _) | Home::Service(namespace) | Home::Ingress(namespace) => {
            row.insert("namespace_id".into(), json!(namespace))
        }
    };
    row.insert("name".into(), json!(name));
    row.insert("uid".into(), Value::Null);
    row.insert("absent_since".into(), Value::Null);
    if let Home::Ingress(_) = home {
        row.insert("class".into(), Value::Null);
        row.insert("rules".into(), json!([]));
        row.insert("tls_hosts".into(), json!([]));
        row.insert("address_ids".into(), json!([]));
    } else if let Home::Service(_) = home {
        row.insert("type".into(), json!(ServiceType::ClusterIp.as_str()));
        row.insert("headless".into(), json!(false));
        row.insert("external_name".into(), Value::Null);
        row.insert("ports".into(), json!([]));
        row.insert("cluster_address_ids".into(), json!([]));
        row.insert("external_address_ids".into(), json!([]));
        row.insert("workload_ids".into(), json!([]));
    } else if let Home::Workload(_, workload) = home {
        row.insert("kind".into(), json!(workload.as_str()));
        row.insert("labels".into(), json!({}));
        row.insert("service_account".into(), Value::Null);
        row.insert("ports".into(), json!([]));
        row.insert("host_network".into(), json!(false));
    } else if kind == node_kind() {
        row.insert("machine_id".into(), Value::Null);
        row.insert("addresses".into(), json!([]));
        row.insert("range_ids".into(), json!([]));
        row.insert("pod_cidrs".into(), json!([]));
        row.insert("zone".into(), Value::Null);
    } else {
        row.insert("labels".into(), json!({}));
    }
    row
}

/// Where the object of `kind` a `sync` names lives (019/X3: the identity of a workload is
/// (namespace, kind, name), that of a service or an ingress (namespace, name)): the
/// namespace by its name in `cluster`, living then.
fn home_of(
    kind: ModuleKind,
    body: &Map<String, Value>,
    completed: &Completed,
    cluster: Uuid,
) -> Result<Home, Violation> {
    if kind != workload_kind() && kind != service_kind() && kind != ingress_kind() {
        return Ok(Home::Cluster(cluster));
    }
    let namespace = body
        .get("namespace")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("namespace: required, the name of the namespace"))?;
    let namespace = DnsLabel::parse(namespace)?;
    let workload = (kind == workload_kind())
        .then(|| {
            let text = body
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("kind: required"))?;
            WorkloadKind::parse(text).ok_or_else(|| {
                invalid(format!(
                    "kind: {text} is not deployment, statefulset, daemonset, cronjob, job, \
                     replicaset or pod"
                ))
            })
        })
        .transpose()?;
    let id = completed.namespace.ok_or_else(|| {
        invalid(format!(
            "namespace: no living namespace {} in the cluster {cluster}",
            namespace.as_str()
        ))
    })?;
    Ok(match workload {
        Some(w) => Home::Workload(id, w),
        None if kind == ingress_kind() => Home::Ingress(id),
        None => Home::Service(id),
    })
}

/// The refusal of the `close` of an object of `kind` named `name` in `home` that does not
/// live.
fn missing(kind: ModuleKind, home: Home, name: &str) -> Violation {
    match home {
        Home::Workload(namespace, workload) => invalid(format!(
            "no living {workload} {name} in the namespace {namespace}"
        )),
        Home::Service(namespace) => invalid(format!(
            "no living service {name} in the namespace {namespace}"
        )),
        Home::Ingress(namespace) => invalid(format!(
            "no living ingress {name} in the namespace {namespace}"
        )),
        Home::Cluster(cluster) => {
            invalid(format!("no living {kind} {name} in the cluster {cluster}"))
        }
    }
}

/// The plan of an `unselect`: the selection a workload closed by `kube.sync` leaves,
/// planned by its closure (`completed.unselect`), refused from a client.
fn plan_unselect(operation: &ModuleOperation) -> Result<ModulePlan, Violation> {
    let (service, workload, domain) = completed(&operation.body)
        .unselect
        .ok_or_else(|| invalid("action: unselect is not an action of sync"))?;
    let mut plan = unselect(service, workload);
    // Under the lock of the domain of the service, as the closure that adds it.
    plan.domains = vec![domain];
    Ok(plan)
}

/// The write that takes `workload` out of the selection of `service` (applied by
/// `patch_of`).
fn unselect(service: Uuid, workload: Uuid) -> ModulePlan {
    ModulePlan {
        kind: service_kind(),
        object: service,
        creates: false,
        domains: Vec::new(),
        keys: Vec::new(),
        core: Vec::new(),
        data: json!({ "unselect": workload }),
    }
}

/// The operation, added by the closure of `workload` by `kube.sync`, that takes it out of
/// the selection of `service`: a `sync` of action `unselect` whose service and workload
/// are in `completed`, which `prepare` replaces in an operation of a client (refused
/// then).
fn unselect_operation(service: Uuid, workload: Uuid, domain: Uuid) -> Operation {
    let Value::Object(body) = json!({
        "action": "unselect",
        "completed": {"unselect": [service, workload, domain]},
    }) else {
        unreachable!("an object literal")
    };
    Operation::Module(ModuleOperation {
        module: "kube".into(),
        name: "sync".into(),
        body,
    })
}

/// The columns of an `upsert` that cite objects the `sync` creates or reads (the pod ranges
/// of a node, the addresses and the selection of a service, the addresses of an ingress),
/// into `columns`; the
/// operations of the core it adds, and for each the columns that cite what it creates
/// (`data.slots`).
fn owned_columns(
    kind: ModuleKind,
    body: &Map<String, Value>,
    completed: &Completed,
    pod_domain: Uuid,
    columns: &mut Map<String, Value>,
) -> Result<(Vec<Operation>, Vec<Value>), Violation> {
    let mut core = Vec::new();
    let mut slots = Vec::new();
    if let Some(value) = body.get("pod_cidrs") {
        let ranges = pod_ranges(value, completed, pod_domain)?;
        columns.insert("pod_cidrs".into(), json!(ranges.cidrs));
        columns.insert("range_ids".into(), json!(ranges.kept));
        slots.extend(ranges.core.iter().map(|op| slot(op, &["range_ids"])));
        core.extend(ranges.core);
    }
    let sorts = sorts(kind, completed);
    if !sorts.is_empty() {
        let (vips, cited) = vips(body, completed, &sorts, columns)?;
        core.extend(vips);
        slots.extend(cited);
    }
    if let Some(value) = body.get("workloads") {
        columns.insert("workload_ids".into(), json!(selection(value, completed)?));
    }
    Ok((core, slots))
}

impl Handler {
    fn plan_create_cluster(
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let body = body(
            operation,
            &[CLUSTER_FIELDS, &["local", "owner", "site"]].concat(),
        )?;
        let completed = completed(&body);
        for required in ["slug", "pod_domain", "service_domain"] {
            if !body.contains_key(required) {
                return Err(invalid(format!("{required}: required")));
            }
        }
        let (columns, cited) = cluster_columns(&body, resolve)?;
        let mut row = cluster_defaults();
        row.extend(columns);
        row.insert("id".into(), json!(object));
        let owner = body
            .get("owner")
            .map(|v| reference(v, "owner", resolve))
            .transpose()?;
        let site = Site::of(&body, resolve)?;
        Ok(ModulePlan {
            kind: cluster_kind(),
            object,
            creates: true,
            domains: Vec::new(),
            keys: Vec::new(),
            core: anchors(object, owner, site, &completed)?,
            data: json!({ "create": row, "reads": reads(&cited) }),
        })
    }

    fn plan_update_cluster(
        operation: &ModuleOperation,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let body = body(
            operation,
            &[CLUSTER_FIELDS, &["id", "owner", "site"]].concat(),
        )?;
        let completed = completed(&body);
        let cluster = reference(
            body.get("id").ok_or_else(|| invalid("id: required"))?,
            "id",
            resolve,
        )?;
        let (patch, cited) = cluster_columns(&body, resolve)?;
        let owner = match body.get("owner") {
            None => None,
            Some(Value::Null) => return Err(invalid("owner: a cluster keeps an owner")),
            Some(v) => Some(reference(v, "owner", resolve)?),
        };
        let site = Site::of(&body, resolve)?;
        Ok(ModulePlan {
            kind: cluster_kind(),
            object: cluster,
            creates: false,
            domains: Vec::new(),
            keys: vec![cluster_key(cluster)],
            core: anchors(cluster, owner, site, &completed)?,
            data: json!({ "update": patch, "reads": reads(&cited) }),
        })
    }

    /// `kube.sync`: `upsert` creates the object of the identity (cluster, name) or changes
    /// the tracked fields given; `close` ends it, by the core (`Close`).
    fn plan_sync(
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let body = body(
            operation,
            &[
                &["action", "type", "cluster", "name"][..],
                &ALL_FIELDS.concat(),
            ]
            .concat(),
        )?;
        let completed = completed(&body);
        let kind = mirror_kind(&body)?;
        let own = fields_of(kind);
        if let Some(foreign) = body.keys().find(|k| {
            let k = k.as_str();
            ALL_FIELDS.iter().any(|fields| fields.contains(&k)) && !own.contains(&k)
        }) {
            return Err(invalid(format!("{kind}: unknown field `{foreign}`")));
        }
        let cluster = reference(
            body.get("cluster")
                .ok_or_else(|| invalid("cluster: required"))?,
            "cluster",
            resolve,
        )?;
        let domain = completed
            .pod_domain
            .ok_or_else(|| invalid(format!("cluster: {cluster} is not a living cluster")))?;
        let name = mirror_name(kind, &body)?;
        let home = home_of(kind, &body, &completed, cluster)?;
        let keys = vec![cluster_key(cluster)];
        // A service is anchored in the service domain of its cluster, an ingress in its
        // external domain, else its service domain (design « Types d'objets »), the other
        // objects in its pod domain: as `pg::placement` registers them.
        let anchor = if kind == service_kind() {
            completed.service_domain.unwrap_or(domain)
        } else if kind == ingress_kind() {
            completed
                .external_domain
                .or(completed.service_domain)
                .unwrap_or(domain)
        } else {
            domain
        };
        match body.get("action").and_then(Value::as_str) {
            Some("upsert") => {
                let (mut columns, cited) = mirror_columns(kind, &body, resolve)?;
                let (core, slots) = owned_columns(kind, &body, &completed, domain, &mut columns)?;
                let (target, creates, data) = if let Some(existing) = completed.existing {
                    (existing, false, json!({ "update": columns }))
                } else {
                    // A workload or a service deleted then created again under the same name
                    // stays the same object (019/X3), closed meanwhile or not (X9): its
                    // identifier opens again, a new version after the gap.
                    let id = completed.reopened.unwrap_or(object);
                    let mut row = mirror_row(kind, id, home, &name);
                    row.extend(columns);
                    (id, true, json!({ "create": row }))
                };
                let mut data = data;
                data["reads"] = reads(&cited);
                data["slots"] = Value::Array(slots);
                Ok(ModulePlan {
                    kind,
                    object: target,
                    creates,
                    domains: vec![anchor],
                    keys,
                    core,
                    data,
                })
            }
            Some("close") => {
                let existing = completed
                    .existing
                    .ok_or_else(|| missing(kind, home, &name))?;
                // A closed workload leaves the selection of its services, without refusal
                // (design « Références », mise à nul): the core clears a reference only in
                // a cascade, which would close other dependents.
                let services = completed.service_domain.unwrap_or(domain);
                let mut core: Vec<Operation> = completed
                    .selecting
                    .iter()
                    .map(|service| unselect_operation(*service, existing, services))
                    .collect();
                // The networks of the pod ranges of a node, the addresses of a service, are
                // owned by it: the core closes them with it (design « Références »,
                // `OnClose::Owned`).
                core.push(Operation::Close {
                    kind: ObjectKind::Module(kind),
                    id: Ref::Id(existing),
                    expected: None,
                    cascade: false,
                });
                Ok(ModulePlan {
                    kind,
                    object: existing,
                    creates: false,
                    domains: vec![anchor],
                    keys,
                    core,
                    data: json!({ "noop": true }),
                })
            }
            Some(other) => Err(invalid(format!("action: {other} is not an action of sync"))),
            None => Err(invalid("action: required")),
        }
    }
}

/// The columns a write of `plan` changes on `current`: those of an `upsert`, the machine
/// a node no longer designates (`unlink`), the selection a closed workload leaves
/// (`unselect`).
fn patch_of(plan: &ModulePlan, current: &Value) -> Result<Value, StoreError> {
    if let Some(update) = plan.data.get("update") {
        return Ok(update.clone());
    }
    if plan.data.get("unlink").is_some() {
        return Ok(json!({ "machine_id": null }));
    }
    let target: Uuid = plan
        .data
        .get("unselect")
        .cloned()
        .and_then(|t| serde_json::from_value(t).ok())
        .ok_or_else(|| StoreError("a planned write".into()))?;
    let selected: Vec<Uuid> = current
        .get("workload_ids")
        .cloned()
        .and_then(|w| serde_json::from_value(w).ok())
        .unwrap_or_default();
    let kept: Vec<Uuid> = selected.into_iter().filter(|w| *w != target).collect();
    Ok(json!({ "workload_ids": kept }))
}

/// Writes the node, namespace, workload, service or ingress of `plan` and registers it
/// under its cluster (a workload, a service or an ingress under its namespace), anchored in
/// a domain of the cluster (`pg::placement`).
async fn apply_mirror(
    tx: &mut dyn ModuleTx,
    plan: &ModulePlan,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (id, table) = (plan.object, table(plan.kind));
    let conn = connection(tx)?;
    if let Some(row) = plan.data.get("create") {
        // A closed workload, service or ingress opened twice in a batch: the second finds
        // the first.
        let duplicate = match table {
            WORKLOAD_TABLE => Some(&WORKLOAD_DUPLICATE),
            SERVICE_TABLE => Some(&SERVICE_DUPLICATE),
            INGRESS_TABLE => Some(&INGRESS_DUPLICATE),
            _ => None,
        };
        if let Some(rule) = duplicate
            && pg::current(conn, table, id, period).await?.is_some()
        {
            return Err(pg::refused(
                rule,
                format!("the object {id} is opened again twice over the period"),
                &[id],
            ));
        }
        pg::identities(conn, table, id, row, period).await?;
        pg::insert(conn, table, row, period, changeset).await?;
    } else {
        let mut merged = pg::current(conn, table, id, period)
            .await?
            .ok_or(WriteError::NoVersion(id))?;
        let patch = patch_of(plan, &merged)?;
        if let (Value::Object(merged), Value::Object(patch)) = (&mut merged, &patch) {
            merged.extend(patch.clone());
        }
        pg::identities(conn, table, id, &merged, period).await?;
        pg::portion(conn, table, id, &patch, period, changeset).await?;
    }
    let (cluster, domain) = pg::placement(conn, table, id, period)
        .await?
        .ok_or(WriteError::NoVersion(id))?;
    tx.register(
        ModuleObjectRow {
            id,
            kind: plan.kind,
            parent: Some(cluster),
            domain,
        },
        period,
        changeset,
    )
    .await
}

impl ModuleHandler for Handler {
    fn prepare<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        operation: &'a mut ModuleOperation,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            let (at, _) = pg::bounds(period);
            let conn = connection(tx)?;
            let mut completed = Completed::default();
            if operation.name == "sync" {
                let cluster = operation
                    .body
                    .get("cluster")
                    .cloned()
                    .and_then(|c| serde_json::from_value::<Ref>(c).ok());
                let table = match operation.body.get("type").and_then(Value::as_str) {
                    Some(NODE) => Some(NODE_TABLE),
                    Some(NAMESPACE) => Some(NAMESPACE_TABLE),
                    Some(WORKLOAD) => Some(WORKLOAD_TABLE),
                    Some(SERVICE) => Some(SERVICE_TABLE),
                    Some(INGRESS) => Some(INGRESS_TABLE),
                    _ => None,
                };
                let text = |field: &str| operation.body.get(field).and_then(Value::as_str);
                let name = text("name");
                if let Some(Ref::Id(cluster)) = cluster {
                    completed.pod_domain = pg::pod_domain(conn, cluster, at).await?;
                    if let Some(namespaced @ (WORKLOAD_TABLE | SERVICE_TABLE | INGRESS_TABLE)) =
                        table
                    {
                        // A workload, a service or an ingress is named in its namespace,
                        // itself named in the cluster.
                        if let Some(namespace) = text("namespace") {
                            completed.namespace =
                                pg::named(conn, NAMESPACE_TABLE, cluster, namespace, at).await?;
                        }
                        let kind = text("kind").filter(|_| namespaced == WORKLOAD_TABLE);
                        if let (Some(namespace), Some(name)) = (completed.namespace, name)
                            && (kind.is_some() || namespaced != WORKLOAD_TABLE)
                        {
                            let identity = pg::InNamespace {
                                table: namespaced,
                                namespace,
                                kind,
                                name,
                            };
                            completed.existing = pg::namespaced(conn, identity, at).await?;
                            if completed.existing.is_none() {
                                completed.reopened =
                                    pg::namespaced_closed(conn, identity, at).await?;
                            }
                        }
                        // The services a closed workload leaves.
                        if let (Some(workload), true, Some("close")) = (
                            completed.existing,
                            table == Some(WORKLOAD_TABLE),
                            text("action"),
                        ) {
                            completed.selecting =
                                pg::referrers(conn, SERVICE, WORKLOAD, workload, at).await?;
                            completed.service_domain = pg::service_domains(conn, cluster, at)
                                .await?
                                .map(|(service, _)| service);
                        }
                    } else if let (Some(table), Some(name)) = (table, name) {
                        completed.existing = pg::named(conn, table, cluster, name, at).await?;
                    }
                    if table == Some(NODE_TABLE) {
                        completed.pod_containers = pg::pod_containers(conn, cluster, at).await?;
                        if let Some(node) = completed.existing {
                            completed.ranges = pg::node_ranges(conn, node, at).await?;
                        }
                    }
                    // The domains of the addresses of a service or an ingress, and those it
                    // cites.
                    if let Some(addressed @ (SERVICE_TABLE | INGRESS_TABLE)) = table {
                        if let Some((service, external)) =
                            pg::service_domains(conn, cluster, at).await?
                        {
                            completed.service_domain = Some(service);
                            completed.external_domain = external;
                        }
                        if let Some(object) = completed.existing {
                            completed.addresses = if addressed == SERVICE_TABLE {
                                pg::service_addresses(conn, object, at).await?
                            } else {
                                pg::ingress_addresses(conn, object, at).await?
                            };
                        }
                    }
                    if table == Some(SERVICE_TABLE) {
                        // The workloads it selects, by kind and name in its namespace.
                        let wanted = selected(&operation.body);
                        if let (Some(namespace), false) = (completed.namespace, wanted.is_empty()) {
                            completed.selection =
                                pg::workloads_named(conn, namespace, &wanted, at).await?;
                        }
                    }
                }
            } else {
                let (owner_property, site_property) = pg::owner_and_site(conn, at).await?;
                completed.owner_property = owner_property;
                completed.site_property = site_property;
            }
            operation.body.insert(
                "completed".into(),
                serde_json::to_value(&completed).expect("a JSON object"),
            );
            Ok(())
        })
    }

    fn plan(
        &self,
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        match operation.name.as_str() {
            "create_cluster" => Self::plan_create_cluster(operation, object, resolve),
            "update_cluster" => Self::plan_update_cluster(operation, resolve),
            "sync" if operation.body.get("action").and_then(Value::as_str) == Some("unselect") => {
                plan_unselect(operation)
            }
            "sync" => Self::plan_sync(operation, object, resolve),
            other => Err(invalid(format!("kube.{other}: unknown operation"))),
        }
    }

    /// A node cites the networks created for its new pod ranges, a service the addresses
    /// created for its new cluster IPs and external addresses (ADR 0026, amendment 8): each
    /// in the columns `data.slots` names for its operation.
    fn planned(&self, plan: &mut ModulePlan, created: &[Uuid]) {
        let slots: Vec<Option<Vec<String>>> = plan
            .data
            .get("slots")
            .cloned()
            .and_then(|s| serde_json::from_value(s).ok())
            .unwrap_or_default();
        let row = if plan.data.get("create").is_some() {
            "create"
        } else {
            "update"
        };
        for (columns, id) in slots.iter().zip(created) {
            for column in columns.iter().flatten() {
                let Some(ids) = plan.data.get_mut(row).and_then(|r| r.get_mut(column)) else {
                    continue;
                };
                let mut all: Vec<Uuid> = serde_json::from_value(ids.take()).unwrap_or_default();
                all.push(*id);
                // A set: the same values make the same column.
                all.sort_unstable();
                all.dedup();
                *ids = json!(all);
            }
        }
    }

    fn reads(&self, plan: &ModulePlan) -> Vec<(String, Uuid)> {
        plan.data
            .get("reads")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|pair| {
                let permission = pair.get(0)?.as_str()?.to_owned();
                let object = pair.get(1)?.as_str()?.parse().ok()?;
                Some((permission, object))
            })
            .collect()
    }

    fn apply<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        plan: &'a ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(async move {
            // A closure: the core ends the object, then the module's rows (`close`).
            if plan.data.get("noop").is_some() {
                return Ok(());
            }
            if plan.kind != cluster_kind() {
                return apply_mirror(tx, plan, period, changeset).await;
            }
            let id = plan.object;
            let conn = connection(tx)?;
            if let Some(row) = plan.data.get("create") {
                pg::identities(conn, CLUSTER_TABLE, id, row, period).await?;
                pg::insert(conn, CLUSTER_TABLE, row, period, changeset).await?;
            } else {
                let patch = &plan.data["update"];
                // The objects of a cluster are anchored in its pod domain: moving it under
                // them would leave their rights and locks on the old one (019-3b, reste).
                if let Some(domain) = patch.get("pod_domain_id")
                    && pg::pod_domain_moved(conn, id, domain, period).await?
                {
                    return Err(pg::refused(
                        &OPERATION_INVALID,
                        format!(
                            "pod_domain: the cluster {id} has living nodes or namespaces, \
                             anchored in its pod domain; close them before changing it"
                        ),
                        &[id],
                    ));
                }
                pg::identities(conn, CLUSTER_TABLE, id, patch, period).await?;
                pg::portion(conn, CLUSTER_TABLE, id, patch, period, changeset).await?;
            }
            tx.register(
                ModuleObjectRow {
                    id,
                    kind: plan.kind,
                    parent: None,
                    domain: None,
                },
                period,
                changeset,
            )
            .await
        })
    }

    fn close<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        kind: ModuleKind,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        tx.close_versioned(table(kind), id, period, changeset)
    }

    fn restore<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        kind: ModuleKind,
        id: Uuid,
        before: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        tx.restore_versioned(table(kind), id, before, period, changeset)
    }

    fn referrers<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        reference: &'a Reference,
        target: Uuid,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<Vec<Uuid>, StoreError>> {
        Box::pin(async move {
            let (at, _) = pg::bounds(period);
            pg::referrers(connection(tx)?, reference.from, reference.to, target, at).await
        })
    }

    /// The networks of the pod ranges a node owns (`range_ids`), the addresses a service
    /// owns (`cluster_address_ids`, `external_address_ids`), the addresses an ingress owns
    /// (`address_ids`), closed with it.
    fn targets<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        reference: &'a Reference,
        source: Uuid,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<Vec<Uuid>, StoreError>> {
        Box::pin(async move {
            let (at, _) = pg::bounds(period);
            match (reference.from, reference.to) {
                (NODE, PREFIX) => {
                    let ranges = pg::node_ranges(connection(tx)?, source, at).await?;
                    Ok(ranges.into_iter().map(|(_, id)| id).collect())
                }
                (SERVICE, ADDRESS) => {
                    let addresses = pg::service_addresses(connection(tx)?, source, at).await?;
                    Ok(addresses.into_iter().map(|(.., id)| id).collect())
                }
                (INGRESS, ADDRESS) => {
                    let addresses = pg::ingress_addresses(connection(tx)?, source, at).await?;
                    Ok(addresses.into_iter().map(|(.., id)| id).collect())
                }
                _ => Ok(Vec::new()),
            }
        })
    }

    /// A closed machine is no longer designated by its node, a closed workload no longer
    /// selected by its services (design « Références »).
    fn nullify(&self, reference: &Reference, source: Uuid, target: Uuid) -> Option<ModulePlan> {
        match (reference.from, reference.to) {
            (NODE, MACHINE) => Some(ModulePlan {
                kind: node_kind(),
                object: source,
                creates: false,
                domains: Vec::new(),
                keys: Vec::new(),
                core: Vec::new(),
                data: json!({ "unlink": target }),
            }),
            (SERVICE, WORKLOAD) => Some(unselect(source, target)),
            _ => None,
        }
    }

    /// The rules of the module on the final state of the clusters, nodes and ingresses the
    /// changeset writes, over all their versions, and of the ingresses whose routes name a
    /// service it writes.
    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        Box::pin(async move {
            let of = |wanted: ModuleKind| -> Vec<Uuid> {
                context
                    .touched
                    .iter()
                    .filter(|(kind, _)| *kind == wanted)
                    .map(|(_, id)| *id)
                    .collect()
            };
            let (clusters, nodes) = (of(cluster_kind()), of(node_kind()));
            let (mut ingresses, services) = (of(ingress_kind()), of(service_kind()));
            if clusters.is_empty()
                && nodes.is_empty()
                && ingresses.is_empty()
                && services.is_empty()
            {
                return Ok(Findings::default());
            }
            let conn = connection(tx)?;
            // A service written, created or closed, may be the one an ingress routes to.
            if !services.is_empty() {
                ingresses.extend(pg::ingresses_routing_to(conn, &services).await?);
                ingresses.sort_unstable();
                ingresses.dedup();
            }
            let mut found = Vec::new();
            if !clusters.is_empty() {
                for (cluster, range) in pg::ranges_outside(conn, &clusters).await? {
                    found.push(
                        Violation::new(
                            &CLUSTER_RANGE_DOMAIN,
                            format!(
                                "the range {range} is not a living container of the pod domain, or network of the service domain, of the cluster over its period"
                            ),
                        )
                        .with_subjects([cluster, range]),
                    );
                }
                for (cluster, domain) in pg::dead_domains(conn, &clusters).await? {
                    found.push(
                        invalid(format!(
                            "the domain {domain} does not live over the period of the cluster"
                        ))
                        .with_subjects([cluster, domain]),
                    );
                }
                for (cluster, group) in pg::dead_groups(conn, &clusters).await? {
                    found.push(
                        Violation::new(
                            &CLUSTER_CONNECTOR_GROUP,
                            format!(
                                "the connector group {group} does not live over the period of the cluster"
                            ),
                        )
                        .with_subjects([cluster, group]),
                    );
                }
            }
            for (node, machine) in pg::bad_machines(conn, &nodes).await? {
                found.push(
                    Violation::new(
                        &NODE_MACHINE_KIND,
                        format!(
                            "the machine {machine} is not a physical or virtual machine living over the period of the node"
                        ),
                    )
                    .with_subjects([node, machine]),
                );
            }
            for (node, range) in pg::pod_ranges_outside(conn, &nodes).await? {
                found.push(
                    Violation::new(
                        &NODE_RANGE_OUTSIDE,
                        format!(
                            "the pod range {range} is not inside a pod container of the cluster: no network is created for it"
                        ),
                    )
                    .with_subjects([node]),
                );
            }
            for (ingress, service) in pg::backends_missing(conn, &ingresses).await? {
                found.push(
                    Violation::new(
                        &INGRESS_BACKEND_MISSING,
                        format!(
                            "a route of the ingress targets the service {service}, absent from its namespace"
                        ),
                    )
                    .with_subjects([ingress]),
                );
            }
            let mut findings = Findings::from(found);
            // A node written anew is checked anew: its anomaly closes once its ranges are
            // inside the containers; an ingress too, once its services are present.
            findings
                .reevaluated
                .extend(nodes.iter().map(|n| (*n, NODE_RANGE_OUTSIDE.id)));
            findings
                .reevaluated
                .extend(ingresses.iter().map(|i| (*i, INGRESS_BACKEND_MISSING.id)));
            Ok(findings)
        })
    }
}
