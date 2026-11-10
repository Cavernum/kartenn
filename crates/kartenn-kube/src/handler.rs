//! The handler of the module in the write pipeline (feature `pg`; ADR 0026, clause 6):
//! the declaration of a cluster by an administrator (`create_cluster`, `update_cluster`,
//! REQ-019-5) and the batch of the connector (`sync`, 019/X9), one node, namespace or
//! workload of a cluster per operation, found by its identity in the cluster, or in its
//! namespace for a workload (REQ-019-6 to 8). `owner` and `site` of a cluster are
//! assignments of `core.owner` and `core.site` the handler adds to the changeset; its
//! nodes, namespaces and workloads inherit them (tree `kube`) and are anchored in its pod
//! domain. The pod ranges of a node (`pod_cidrs`, REQ-019-13) are networks of that
//! domain the `sync` creates and closes as operations of the core, cited by the node
//! (`range_ids`) once their identifiers come back (`planned`, ADR 0026 amendment 8).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use kartenn_core::addr::Prefix;
use kartenn_core::family;
use kartenn_core::objects::{Nature, Slug};
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

use crate::module::{CLUSTER, MACHINE, NAMESPACE, NODE, PREFIX, WORKLOAD};
use crate::names::{DnsLabel, DnsSubdomain, LabelKey, LabelValue, parse_labels};
use crate::owner::WorkloadKind;
use crate::pg::{self, CLUSTER_TABLE, NAMESPACE_TABLE, NODE_TABLE, WORKLOAD_TABLE};
use crate::rules::{
    CLUSTER_CONNECTOR_GROUP, CLUSTER_RANGE_DOMAIN, NAME_INVALID, NODE_MACHINE_KIND,
    NODE_RANGE_OUTSIDE, OPERATION_INVALID,
};
use crate::tracked::Protocol;

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

/// The table of the objects of `kind`.
fn table(kind: ModuleKind) -> &'static str {
    if kind == node_kind() {
        NODE_TABLE
    } else if kind == namespace_kind() {
        NAMESPACE_TABLE
    } else if kind == workload_kind() {
        WORKLOAD_TABLE
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
    /// The living namespace a `sync` of a workload names, by its name in the cluster.
    #[serde(default)]
    namespace: Option<Uuid>,
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

/// The fields `sync` takes for the type `kind`.
fn fields_of(kind: ModuleKind) -> &'static [&'static str] {
    if kind == node_kind() {
        NODE_FIELDS
    } else if kind == workload_kind() {
        WORKLOAD_FIELDS
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

/// The declared ports of `value`, a sorted set.
fn workload_ports(value: &Value) -> Result<Vec<WorkloadPort>, Violation> {
    let ports: Vec<WorkloadPort> = decode(value.clone(), "ports")?;
    for port in &ports {
        if port.port == 0 {
            return Err(invalid("ports: a port from 1 to 65535"));
        }
        if Protocol::parse(&port.protocol).is_none() {
            return Err(invalid(format!(
                "ports: {} is not tcp, udp or sctp",
                port.protocol
            )));
        }
        if let Some(name) = &port.name
            && !port_name(name)
        {
            return Err(Violation::new(
                &NAME_INVALID,
                format!("ports: {name} is not a port name (IANA_SVC_NAME)"),
            ));
        }
    }
    // A set: a reordering makes no version (019/X4).
    let ports: BTreeSet<WorkloadPort> = ports.into_iter().collect();
    Ok(ports.into_iter().collect())
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
            .map(|(_, id)| close_prefix(*id)),
    );
    Ok(PodRanges {
        cidrs: wanted.iter().map(ToString::to_string).collect(),
        kept,
        core,
    })
}

/// The closure of the network `id` of a pod range.
fn close_prefix(id: Uuid) -> Operation {
    Operation::Close {
        kind: ObjectKind::Prefix,
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
            "addresses" => {
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
            "ports" => ("ports", json!(workload_ports(value)?)),
            "host_network" => {
                let host = value
                    .as_bool()
                    .ok_or_else(|| invalid("host_network: a boolean"))?;
                ("host_network", json!(host))
            }
            // The identity of a workload (`namespace`, `kind`), not a column it changes.
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
        Some(other) => Err(invalid(format!("type: {other} is not synced yet"))),
        None => Err(invalid("type: required")),
    }
}

/// The name a `sync` names, checked for its type: a DNS subdomain for a node or a
/// workload, a DNS label for a namespace (réf. `kubernetes-calcul.md` § Namespace, labels,
/// noms et UID).
fn mirror_name(kind: ModuleKind, body: &Map<String, Value>) -> Result<String, Violation> {
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("name: required"))?;
    Ok(if kind == node_kind() || kind == workload_kind() {
        DnsSubdomain::parse(name)?.as_str().to_owned()
    } else {
        DnsLabel::parse(name)?.as_str().to_owned()
    })
}

/// The row of a new object `object` of `kind` named `name`, every tracked field at its
/// default: in `cluster`, or in the namespace of `workload` for a workload.
fn mirror_row(
    kind: ModuleKind,
    object: Uuid,
    cluster: Uuid,
    name: &str,
    workload: Option<(Uuid, WorkloadKind)>,
) -> Map<String, Value> {
    let mut row = Map::new();
    row.insert("id".into(), json!(object));
    if let Some((namespace, workload)) = workload {
        row.insert("namespace_id".into(), json!(namespace));
        row.insert("kind".into(), json!(workload.as_str()));
    } else {
        row.insert("cluster_id".into(), json!(cluster));
    }
    row.insert("name".into(), json!(name));
    row.insert("uid".into(), Value::Null);
    row.insert("absent_since".into(), Value::Null);
    if workload.is_some() {
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

/// The namespace and kind of the workload a `sync` names (019/X3: its identity is
/// (namespace, kind, name)): the namespace by its name in `cluster`, living then.
fn workload_identity(
    body: &Map<String, Value>,
    completed: &Completed,
    cluster: Uuid,
) -> Result<(Uuid, WorkloadKind), Violation> {
    let namespace = body
        .get("namespace")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("namespace: required, the name of the namespace"))?;
    let namespace = DnsLabel::parse(namespace)?;
    let kind = body
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("kind: required"))?;
    let kind = WorkloadKind::parse(kind).ok_or_else(|| {
        invalid(format!(
            "kind: {kind} is not deployment, statefulset, daemonset, cronjob, job, replicaset \
             or pod"
        ))
    })?;
    let id = completed.namespace.ok_or_else(|| {
        invalid(format!(
            "namespace: no living namespace {} in the cluster {cluster}",
            namespace.as_str()
        ))
    })?;
    Ok((id, kind))
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
                NODE_FIELDS,
                NAMESPACE_FIELDS,
                WORKLOAD_FIELDS,
            ]
            .concat(),
        )?;
        let completed = completed(&body);
        let kind = mirror_kind(&body)?;
        let own = fields_of(kind);
        if let Some(foreign) = body.keys().find(|k| {
            let k = k.as_str();
            [NODE_FIELDS, NAMESPACE_FIELDS, WORKLOAD_FIELDS]
                .iter()
                .any(|fields| fields.contains(&k))
                && !own.contains(&k)
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
        let workload = (kind == workload_kind())
            .then(|| workload_identity(&body, &completed, cluster))
            .transpose()?;
        let keys = vec![cluster_key(cluster)];
        match body.get("action").and_then(Value::as_str) {
            Some("upsert") => {
                let (mut columns, cited) = mirror_columns(kind, &body, resolve)?;
                let mut core = Vec::new();
                if let Some(value) = body.get("pod_cidrs") {
                    let ranges = pod_ranges(value, &completed, domain)?;
                    columns.insert("pod_cidrs".into(), json!(ranges.cidrs));
                    columns.insert("range_ids".into(), json!(ranges.kept));
                    core = ranges.core;
                }
                let (target, creates, data) = if let Some(existing) = completed.existing {
                    (existing, false, json!({ "update": columns }))
                } else {
                    let mut row = mirror_row(kind, object, cluster, &name, workload);
                    row.extend(columns);
                    (object, true, json!({ "create": row }))
                };
                let mut data = data;
                data["reads"] = reads(&cited);
                Ok(ModulePlan {
                    kind,
                    object: target,
                    creates,
                    domains: vec![domain],
                    keys,
                    core,
                    data,
                })
            }
            Some("close") => {
                let existing = completed.existing.ok_or_else(|| match workload {
                    Some((namespace, workload)) => invalid(format!(
                        "no living {workload} {name} in the namespace {namespace}"
                    )),
                    None => invalid(format!("no living {kind} {name} in the cluster {cluster}")),
                })?;
                // The networks of its pod ranges are owned by the node: the core closes
                // them with it (design « Références », `OnClose::Owned`).
                let core = vec![Operation::Close {
                    kind: ObjectKind::Module(kind),
                    id: Ref::Id(existing),
                    expected: None,
                    cascade: false,
                }];
                Ok(ModulePlan {
                    kind,
                    object: existing,
                    creates: false,
                    domains: vec![domain],
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

/// Writes the node, namespace or workload of `plan` and registers it under its cluster (a
/// workload under its namespace), anchored in the pod domain of the cluster.
async fn apply_mirror(
    tx: &mut dyn ModuleTx,
    plan: &ModulePlan,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (id, table) = (plan.object, table(plan.kind));
    let conn = connection(tx)?;
    if let Some(row) = plan.data.get("create") {
        pg::identities(conn, table, id, row, period).await?;
        pg::insert(conn, table, row, period, changeset).await?;
    } else {
        let patch = plan
            .data
            .get("update")
            .cloned()
            .or_else(|| {
                plan.data
                    .get("unlink")
                    .map(|_| json!({ "machine_id": null }))
            })
            .ok_or_else(|| StoreError("a planned write".into()))?;
        let mut merged = pg::current(conn, table, id, period)
            .await?
            .ok_or(WriteError::NoVersion(id))?;
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
                    _ => None,
                };
                let text = |field: &str| operation.body.get(field).and_then(Value::as_str);
                let name = text("name");
                if let Some(Ref::Id(cluster)) = cluster {
                    completed.pod_domain = pg::pod_domain(conn, cluster, at).await?;
                    if table == Some(WORKLOAD_TABLE) {
                        // A workload is named in its namespace, itself named in the cluster.
                        if let Some(namespace) = text("namespace") {
                            completed.namespace =
                                pg::named(conn, NAMESPACE_TABLE, cluster, namespace, at).await?;
                        }
                        if let (Some(namespace), Some(kind), Some(name)) =
                            (completed.namespace, text("kind"), name)
                        {
                            completed.existing =
                                pg::workload_named(conn, namespace, kind, name, at).await?;
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
            "sync" => Self::plan_sync(operation, object, resolve),
            other => Err(invalid(format!("kube.{other}: unknown operation"))),
        }
    }

    /// A node cites the networks created for its new pod ranges (ADR 0026, amendment 8).
    fn planned(&self, plan: &mut ModulePlan, created: &[Uuid]) {
        let mut new: Vec<Uuid> = plan
            .core
            .iter()
            .zip(created)
            .filter(|(op, _)| matches!(op, Operation::CreatePrefix { .. }))
            .map(|(_, id)| *id)
            .collect();
        if new.is_empty() {
            return;
        }
        let row = if plan.data.get("create").is_some() {
            "create"
        } else {
            "update"
        };
        let Some(ids) = plan.data.get_mut(row).and_then(|r| r.get_mut("range_ids")) else {
            return;
        };
        let mut all: Vec<Uuid> = serde_json::from_value(ids.take()).unwrap_or_default();
        all.append(&mut new);
        // A set: the same ranges make the same column.
        all.sort_unstable();
        all.dedup();
        *ids = json!(all);
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

    /// The networks of the pod ranges a node owns (`range_ids`), closed with it.
    fn targets<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        reference: &'a Reference,
        source: Uuid,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<Vec<Uuid>, StoreError>> {
        Box::pin(async move {
            if (reference.from, reference.to) != (NODE, PREFIX) {
                return Ok(Vec::new());
            }
            let (at, _) = pg::bounds(period);
            let ranges = pg::node_ranges(connection(tx)?, source, at).await?;
            Ok(ranges.into_iter().map(|(_, id)| id).collect())
        })
    }

    /// A closed machine is no longer designated by its node (design « Références »).
    fn nullify(&self, reference: &Reference, source: Uuid, target: Uuid) -> Option<ModulePlan> {
        (reference.from == NODE && reference.to == MACHINE).then(|| ModulePlan {
            kind: node_kind(),
            object: source,
            creates: false,
            domains: Vec::new(),
            keys: Vec::new(),
            core: Vec::new(),
            data: json!({ "unlink": target }),
        })
    }

    /// The rules of the module on the final state of the clusters and nodes the changeset
    /// writes, over all their versions.
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
            if clusters.is_empty() && nodes.is_empty() {
                return Ok(Findings::default());
            }
            let conn = connection(tx)?;
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
            let mut findings = Findings::from(found);
            // A node written anew is checked anew: its anomaly closes once its ranges are
            // inside the containers.
            findings
                .reevaluated
                .extend(nodes.iter().map(|n| (*n, NODE_RANGE_OUTSIDE.id)));
            Ok(findings)
        })
    }
}
