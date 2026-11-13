//! The reads of the objects the connector mirrors (task 019-8b; REQ-019-38, 39; design
//! « Routes », « Données »): `GET /v1/kube-nodes`, `/v1/kube-namespaces`, `/v1/workloads`,
//! `/v1/kube-services` and `/v1/ingresses`, and `/{id}` of each, without writes (they are
//! written by `kube.sync` in a changeset). Lists by name, filtered by cluster, namespace,
//! name, kind, tracked label, machine and absence; reads dated (`valid_at`, `as_of`), under
//! `kube.<type>.read` on the domain the object is anchored in or its owner and site, and
//! journaled. An object the caller may not read answers as an absent one, 404.

use std::collections::BTreeMap;

use axum::Extension;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use kartenn_rest::ModuleRouter;
use kartenn_rest::kit::{
    self, Api, Dates, DatesParams, ListQuery, Page, PageParams, Problem, ProblemKind, documented,
    listing_of, next_href, op, presented, queryable, respond_page, schemas_of,
};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::Principal;
use kartenn_store_pg::rbac::readable;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use sqlx::postgres::PgRow;
use time::OffsetDateTime;
use utoipa::openapi::path::HttpMethod;
use utoipa::openapi::schema::Type;
use utoipa::openapi::{ObjectBuilder, RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};
use uuid::Uuid;

use super::IdRef;
use super::reading::{criteria, dated, dated_where, listed};
use crate::module::{INGRESS, NAMESPACE, NODE, SERVICE, WORKLOAD, type_of};
use crate::observations::rfc3339;

/// The filter of the objects absent from their cluster (`true`) or present (`false`), in
/// the grammar of the lists of the core (`{field}:exists`, spec 006, design « Filtres »).
const ABSENT: &str = "absent_since:exists";

// ---------------------------------------------------------------- representations

/// An address of a node, as the connector reads it (`status.addresses`).
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[schema(as = KubeNodeAddress)]
struct NodeAddress {
    /// `InternalIP`, `ExternalIP`, `Hostname`…
    r#type: String,
    /// The address or the name.
    value: String,
}

/// A node of a cluster (REQ-019-6).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeNode)]
struct NodeDto {
    /// Identifier.
    id: Uuid,
    /// Its cluster.
    cluster: IdRef,
    /// Its name, unique in its cluster at every instant.
    name: String,
    /// Its `uid` in Kubernetes.
    uid: Option<Uuid>,
    /// The machine (`machines.machine`, physical or virtual) it is.
    machine: Option<IdRef>,
    /// Its addresses as read.
    addresses: Vec<NodeAddress>,
    /// The networks of the core of its pod ranges.
    ranges: Vec<IdRef>,
    /// Its pod ranges as read (`spec.podCIDRs`).
    pod_cidrs: Vec<String>,
    /// Its zone.
    zone: Option<String>,
    /// Since when it is absent from its cluster, RFC 3339; `null` while present.
    absent_since: Option<String>,
    /// Its URL.
    href: String,
}

/// A namespace of a cluster (REQ-019-7).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeNamespace)]
struct NamespaceDto {
    /// Identifier.
    id: Uuid,
    /// Its cluster.
    cluster: IdRef,
    /// Its name, unique in its cluster at every instant.
    name: String,
    /// Its `uid` in Kubernetes.
    uid: Option<Uuid>,
    /// Its tracked labels only.
    labels: BTreeMap<String, String>,
    /// Since when it is absent from its cluster, RFC 3339; `null` while present.
    absent_since: Option<String>,
    /// Its URL.
    href: String,
}

/// A port declared by the containers of a workload.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[schema(as = KubeWorkloadPort)]
struct WorkloadPort {
    /// 1 to 65535.
    port: u16,
    /// `tcp`, `udp` or `sctp`.
    protocol: String,
    /// Its name (`IANA_SVC_NAME`).
    #[serde(default)]
    name: Option<String>,
}

/// A workload of a namespace (REQ-019-8).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeWorkload)]
struct WorkloadDto {
    /// Identifier.
    id: Uuid,
    /// The cluster of its namespace.
    cluster: IdRef,
    /// Its namespace.
    namespace: IdRef,
    /// `deployment`, `statefulset`, `daemonset`, `cronjob`, `job`, `replicaset` or `pod`.
    kind: String,
    /// Its name, unique by kind in its namespace at every instant.
    name: String,
    /// Its `uid` in Kubernetes.
    uid: Option<Uuid>,
    /// The tracked labels of its pod template only.
    labels: BTreeMap<String, String>,
    /// Its service account.
    service_account: Option<String>,
    /// The ports its containers declare, a sorted set.
    ports: Vec<WorkloadPort>,
    /// Whether its pods share the network of their node (`hostNetwork`).
    host_network: bool,
    /// Since when it is absent from its cluster, RFC 3339; `null` while present.
    absent_since: Option<String>,
    /// Its URL.
    href: String,
}

/// The port a service or a route targets: a number or a port name.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(untagged)]
#[schema(as = KubeTargetPort)]
enum TargetPort {
    /// A number, 1 to 65535.
    Number(u16),
    /// A port name.
    Name(String),
}

/// A port of a service (`spec.ports`).
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[schema(as = KubeServicePort)]
struct ServicePort {
    /// 1 to 65535.
    port: u16,
    /// The port it targets on its pods (`targetPort`).
    #[serde(default)]
    target_port: Option<TargetPort>,
    /// `tcp`, `udp` or `sctp`.
    protocol: String,
    /// `appProtocol`.
    #[serde(default)]
    app_protocol: Option<String>,
    /// `nodePort`.
    #[serde(default)]
    node_port: Option<u16>,
    /// The name an ingress may target.
    #[serde(default)]
    name: Option<String>,
}

/// A service of a namespace (REQ-019-9).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeService)]
struct ServiceDto {
    /// Identifier.
    id: Uuid,
    /// The cluster of its namespace.
    cluster: IdRef,
    /// Its namespace.
    namespace: IdRef,
    /// Its name, unique in its namespace at every instant.
    name: String,
    /// Its `uid` in Kubernetes.
    uid: Option<Uuid>,
    /// `cluster_ip`, `node_port`, `load_balancer` or `external_name`.
    r#type: String,
    /// Without a cluster address (`clusterIP: None`).
    headless: bool,
    /// The name it stands for, of an `external_name` service.
    external_name: Option<String>,
    /// Its ports, a sorted set.
    ports: Vec<ServicePort>,
    /// Its addresses of the core in the service domain of its cluster.
    cluster_addresses: Vec<IdRef>,
    /// Its `LoadBalancer` addresses and `externalIPs`, addresses of the core.
    external_addresses: Vec<IdRef>,
    /// The workloads it selects, as the connector computed them.
    workloads: Vec<IdRef>,
    /// Since when it is absent from its cluster, RFC 3339; `null` while present.
    absent_since: Option<String>,
    /// Its URL.
    href: String,
}

/// A route of an ingress; the default backend is the route without host, path nor type.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[schema(as = KubeIngressRoute)]
struct IngressRoute {
    /// Its host.
    #[serde(default)]
    host: Option<String>,
    /// Its path.
    #[serde(default)]
    path: Option<String>,
    /// `exact`, `prefix` or `implementation_specific` (`pathType`).
    #[serde(default)]
    path_type: Option<String>,
    /// The name of a service of the namespace, which may be absent.
    service: String,
    /// The port of that service, by number or by name.
    port: TargetPort,
}

/// An ingress of a namespace (REQ-019-10).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeIngress)]
struct IngressDto {
    /// Identifier.
    id: Uuid,
    /// The cluster of its namespace.
    cluster: IdRef,
    /// Its namespace.
    namespace: IdRef,
    /// Its name, unique in its namespace at every instant.
    name: String,
    /// Its `uid` in Kubernetes.
    uid: Option<Uuid>,
    /// Its class (`ingressClassName`).
    class: Option<String>,
    /// Its routes, a sorted set.
    rules: Vec<IngressRoute>,
    /// The hosts of its TLS.
    tls_hosts: Vec<String>,
    /// Its addresses of the core, in the external domain of its cluster, else its service
    /// domain.
    addresses: Vec<IdRef>,
    /// Since when it is absent from its cluster, RFC 3339; `null` while present.
    absent_since: Option<String>,
    /// Its URL.
    href: String,
}

// ---------------------------------------------------------------- the collections

/// A filter of a collection on a closed set of values of a column.
struct Sort {
    /// The name of the filter.
    filter: &'static str,
    /// Its column.
    column: &'static str,
    /// Its values.
    values: &'static [&'static str],
}

/// A collection of mirrored objects: its type, table and filters.
trait Mirror: Serialize + ToSchema + for<'r> sqlx::FromRow<'r, PgRow> + Send + Unpin + 'static {
    /// Its type.
    const KIND: &'static str;
    /// Its table.
    const TABLE: &'static str;
    /// The columns it reads of its table; `cluster_id` among them unless it lies in a
    /// namespace.
    const COLUMNS: &'static str;
    /// Whether it lies in a namespace (`namespace_id`), not directly in its cluster.
    const NAMESPACED: bool;
    /// Its filter on a closed set of values (its kind), if any.
    const SORT: Option<Sort> = None;
    /// Whether it carries tracked labels (`labels`).
    const LABELED: bool = false;
    /// Whether it designates a machine (`machine_id`).
    const MACHINE: bool = false;
    /// One of it in an operation identifier (`kube_node`).
    const ONE: &'static str;
    /// Several of it in an operation identifier (`kube_nodes`).
    const MANY: &'static str;
    /// Its natural key, in the filters of its list.
    const KEY: &'static [&'static str];
    /// The members a table shows by default.
    const SHOWN: &'static [&'static str];
    /// What its list answers.
    const SUMMARY: &'static str;

    /// Its identifier.
    fn id(&self) -> Uuid;
    /// Its name, the order of a page.
    fn name(&self) -> &str;
}

impl Mirror for NodeDto {
    const KIND: &'static str = NODE;
    const TABLE: &'static str = "kube.node";
    const COLUMNS: &'static str = "id, cluster_id, name, uid, machine_id, addresses, range_ids, \
         pod_cidrs::text[] AS pod_cidrs, zone, absent_since";
    const NAMESPACED: bool = false;
    const MACHINE: bool = true;
    const ONE: &'static str = "kube_node";
    const MANY: &'static str = "kube_nodes";
    const KEY: &'static [&'static str] = &["cluster", "name"];
    const SHOWN: &'static [&'static str] = &["name", "cluster", "machine", "zone", "absent_since"];
    const SUMMARY: &'static str = "The nodes the caller may read (REQ-019-38), by name.";

    fn id(&self) -> Uuid {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl Mirror for NamespaceDto {
    const KIND: &'static str = NAMESPACE;
    const TABLE: &'static str = "kube.namespace";
    const COLUMNS: &'static str = "id, cluster_id, name, uid, labels, absent_since";
    const NAMESPACED: bool = false;
    const LABELED: bool = true;
    const ONE: &'static str = "kube_namespace";
    const MANY: &'static str = "kube_namespaces";
    const KEY: &'static [&'static str] = &["cluster", "name"];
    const SHOWN: &'static [&'static str] = &["name", "cluster", "labels", "absent_since"];
    const SUMMARY: &'static str = "The namespaces the caller may read (REQ-019-38), by name.";

    fn id(&self) -> Uuid {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl Mirror for WorkloadDto {
    const KIND: &'static str = WORKLOAD;
    const TABLE: &'static str = "kube.workload";
    const COLUMNS: &'static str = "id, namespace_id, kind, name, uid, labels, service_account, \
         ports, host_network, absent_since";
    const NAMESPACED: bool = true;
    const SORT: Option<Sort> = Some(Sort {
        filter: "kind",
        column: "kind",
        values: &[
            "deployment",
            "statefulset",
            "daemonset",
            "cronjob",
            "job",
            "replicaset",
            "pod",
        ],
    });
    const LABELED: bool = true;
    const ONE: &'static str = "kube_workload";
    const MANY: &'static str = "kube_workloads";
    const KEY: &'static [&'static str] = &["namespace", "kind", "name"];
    const SHOWN: &'static [&'static str] = &["name", "kind", "namespace", "absent_since"];
    const SUMMARY: &'static str = "The workloads the caller may read (REQ-019-38), by name.";

    fn id(&self) -> Uuid {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl Mirror for ServiceDto {
    const KIND: &'static str = SERVICE;
    const TABLE: &'static str = "kube.service";
    const COLUMNS: &'static str = "id, namespace_id, name, uid, type, headless, external_name, \
         ports, cluster_address_ids, external_address_ids, workload_ids, absent_since";
    const NAMESPACED: bool = true;
    const SORT: Option<Sort> = Some(Sort {
        filter: "type",
        column: "type",
        values: &["cluster_ip", "node_port", "load_balancer", "external_name"],
    });
    const ONE: &'static str = "kube_service";
    const MANY: &'static str = "kube_services";
    const KEY: &'static [&'static str] = &["namespace", "name"];
    const SHOWN: &'static [&'static str] = &["name", "type", "namespace", "absent_since"];
    const SUMMARY: &'static str = "The services the caller may read (REQ-019-38), by name.";

    fn id(&self) -> Uuid {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl Mirror for IngressDto {
    const KIND: &'static str = INGRESS;
    const TABLE: &'static str = "kube.ingress";
    const COLUMNS: &'static str =
        "id, namespace_id, name, uid, class, rules, tls_hosts, address_ids, absent_since";
    const NAMESPACED: bool = true;
    const ONE: &'static str = "kube_ingress";
    const MANY: &'static str = "kube_ingresses";
    const KEY: &'static [&'static str] = &["namespace", "name"];
    const SHOWN: &'static [&'static str] = &["name", "class", "namespace", "absent_since"];
    const SUMMARY: &'static str = "The ingresses the caller may read (REQ-019-38), by name.";

    fn id(&self) -> Uuid {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------- rows

/// The cells of a row of a mirrored object.
struct Cells<'r>(&'r PgRow);

impl Cells<'_> {
    fn id(&self) -> Result<Uuid, sqlx::Error> {
        self.0.try_get("id")
    }

    fn get<T>(&self, name: &str) -> Result<T, sqlx::Error>
    where
        for<'a> T: sqlx::Decode<'a, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
    {
        self.0.try_get(name)
    }

    fn one(&self, name: &str) -> Result<IdRef, sqlx::Error> {
        self.get(name).map(|id| IdRef { id })
    }

    fn optional(&self, name: &str) -> Result<Option<IdRef>, sqlx::Error> {
        Ok(self.get::<Option<Uuid>>(name)?.map(|id| IdRef { id }))
    }

    fn many(&self, name: &str) -> Result<Vec<IdRef>, sqlx::Error> {
        Ok(self
            .get::<Vec<Uuid>>(name)?
            .into_iter()
            .map(|id| IdRef { id })
            .collect())
    }

    /// A JSON cell, as the handler wrote it.
    fn json<T: DeserializeOwned>(&self, name: &str) -> Result<T, sqlx::Error> {
        let value: Value = self.get(name)?;
        serde_json::from_value(value).map_err(|e| sqlx::Error::ColumnDecode {
            index: name.to_owned(),
            source: Box::new(e),
        })
    }

    fn absent_since(&self) -> Result<Option<String>, sqlx::Error> {
        Ok(self
            .get::<Option<OffsetDateTime>>("absent_since")?
            .map(rfc3339))
    }

    fn href<M: Mirror>(id: Uuid) -> String {
        format!("/v1/{}/{id}", type_of(M::KIND).collection)
    }
}

impl<'r> sqlx::FromRow<'r, PgRow> for NodeDto {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let c = Cells(row);
        let id = c.id()?;
        Ok(Self {
            id,
            cluster: c.one("cluster_id")?,
            name: c.get("name")?,
            uid: c.get("uid")?,
            machine: c.optional("machine_id")?,
            addresses: c.json("addresses")?,
            ranges: c.many("range_ids")?,
            pod_cidrs: c.get("pod_cidrs")?,
            zone: c.get("zone")?,
            absent_since: c.absent_since()?,
            href: Cells::href::<Self>(id),
        })
    }
}

impl<'r> sqlx::FromRow<'r, PgRow> for NamespaceDto {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let c = Cells(row);
        let id = c.id()?;
        Ok(Self {
            id,
            cluster: c.one("cluster_id")?,
            name: c.get("name")?,
            uid: c.get("uid")?,
            labels: c.json("labels")?,
            absent_since: c.absent_since()?,
            href: Cells::href::<Self>(id),
        })
    }
}

impl<'r> sqlx::FromRow<'r, PgRow> for WorkloadDto {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let c = Cells(row);
        let id = c.id()?;
        Ok(Self {
            id,
            cluster: c.one("cluster_id")?,
            namespace: c.one("namespace_id")?,
            kind: c.get("kind")?,
            name: c.get("name")?,
            uid: c.get("uid")?,
            labels: c.json("labels")?,
            service_account: c.get("service_account")?,
            ports: c.json("ports")?,
            host_network: c.get("host_network")?,
            absent_since: c.absent_since()?,
            href: Cells::href::<Self>(id),
        })
    }
}

impl<'r> sqlx::FromRow<'r, PgRow> for ServiceDto {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let c = Cells(row);
        let id = c.id()?;
        Ok(Self {
            id,
            cluster: c.one("cluster_id")?,
            namespace: c.one("namespace_id")?,
            name: c.get("name")?,
            uid: c.get("uid")?,
            r#type: c.get("type")?,
            headless: c.get("headless")?,
            external_name: c.get("external_name")?,
            ports: c.json("ports")?,
            cluster_addresses: c.many("cluster_address_ids")?,
            external_addresses: c.many("external_address_ids")?,
            workloads: c.many("workload_ids")?,
            absent_since: c.absent_since()?,
            href: Cells::href::<Self>(id),
        })
    }
}

impl<'r> sqlx::FromRow<'r, PgRow> for IngressDto {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let c = Cells(row);
        let id = c.id()?;
        Ok(Self {
            id,
            cluster: c.one("cluster_id")?,
            namespace: c.one("namespace_id")?,
            name: c.get("name")?,
            uid: c.get("uid")?,
            class: c.get("class")?,
            rules: c.json("rules")?,
            tls_hosts: c.get("tls_hosts")?,
            addresses: c.many("address_ids")?,
            absent_since: c.absent_since()?,
            href: Cells::href::<Self>(id),
        })
    }
}

// ---------------------------------------------------------------- reading

/// The select list of `M` from the relation `x`: its columns, and for an object of a
/// namespace the cluster of that namespace at the dates of the read.
fn select<M: Mirror>() -> String {
    if M::NAMESPACED {
        format!(
            "x.*, (SELECT n.cluster_id FROM {} n LIMIT 1) AS cluster_id",
            dated_where("kube.namespace", "cluster_id", "id = x.namespace_id")
        )
    } else {
        "x.*".to_owned()
    }
}

/// What restricts a list, each in conjunction.
#[derive(Debug, Default)]
struct Filter<'a> {
    name: Option<&'a str>,
    cluster: Option<Uuid>,
    namespace: Option<Uuid>,
    sort: Option<&'a str>,
    /// The tracked label `{key: value}` the object carries.
    label: Option<Value>,
    machine: Option<Uuid>,
    /// Absent from its cluster (`true`) or present (`false`).
    absent: Option<bool>,
}

fn uuid(wanted: &BTreeMap<&str, &str>, name: &str) -> Result<Option<Uuid>, Problem> {
    wanted
        .get(name)
        .map(|v| {
            v.parse()
                .map_err(|_| kit::bad(format!("{name} is not an identifier")))
        })
        .transpose()
}

/// The filters of `page` on `M`: each at most once; 400 for an unknown one or a value
/// that does not read.
fn filter_of<M: Mirror>(page: &Page) -> Result<Filter<'_>, Problem> {
    let mut names = vec!["q", "name", "cluster", ABSENT];
    if M::NAMESPACED {
        names.push("namespace");
    }
    if let Some(sort) = &M::SORT {
        names.push(sort.filter);
    }
    if M::LABELED {
        names.push("label");
    }
    if M::MACHINE {
        names.push("machine");
    }
    let mut wanted = BTreeMap::new();
    for (name, value) in &page.filters {
        if !names.contains(&name.as_str()) {
            return Err(kit::bad(format!("unknown parameter {name}")));
        }
        if wanted.insert(name.as_str(), value.as_str()).is_some() {
            return Err(kit::bad(format!("{name} is given more than once")));
        }
    }
    let sort = match &M::SORT {
        Some(sort) => match wanted.get(sort.filter) {
            Some(v) if !sort.values.contains(v) => {
                return Err(kit::bad(format!(
                    "{} must be one of {}",
                    sort.filter,
                    sort.values.join(", ")
                )));
            }
            v => v.copied(),
        },
        None => None,
    };
    let label = wanted
        .get("label")
        .map(|v| match v.split_once('=') {
            Some((key, value)) if !key.is_empty() => Ok(json!({ key: value })),
            _ => Err(kit::bad("label expects key=value")),
        })
        .transpose()?;
    let absent = wanted
        .get(ABSENT)
        .map(|v| match *v {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(kit::bad(format!("{ABSENT} expects true or false"))),
        })
        .transpose()?;
    Ok(Filter {
        name: wanted.get("name").copied(),
        cluster: uuid(&wanted, "cluster")?,
        namespace: uuid(&wanted, "namespace")?,
        sort,
        label,
        machine: uuid(&wanted, "machine")?,
        absent,
    })
}

/// The statement of a page of `M`: the relation of the dates (`$1`, `$2`), the read scope
/// (`$3`), the filters (`$4`, `$8` to `$13`), the cursor (`$5`, `$6`) and the size (`$7`).
/// A filter `M` does not serve is refused before; its parameter is read all the same.
fn page_sql<M: Mirror>(current: bool, absent: Option<bool>) -> String {
    // Written in the text, not bound: a statement of its own for each.
    let absent = match absent {
        None => "",
        Some(true) => "AND x.absent_since IS NOT NULL",
        Some(false) => "AND x.absent_since IS NULL",
    };
    let cluster = if M::NAMESPACED {
        format!(
            "x.namespace_id IN (SELECT n.id FROM {} n)",
            dated_where("kube.namespace", "id", "cluster_id = $9")
        )
    } else {
        "x.cluster_id = $9".to_owned()
    };
    let namespace = if M::NAMESPACED {
        "x.namespace_id = $10"
    } else {
        "false"
    };
    let sort = M::SORT
        .as_ref()
        .map_or_else(|| "false".to_owned(), |s| format!("x.{} = $11", s.column));
    let label = if M::LABELED {
        "x.labels @> $12"
    } else {
        "false"
    };
    let machine = if M::MACHINE {
        "x.machine_id = $13"
    } else {
        "false"
    };
    // The domain the object is anchored in (ADR 0026, clause 4), as the authorizer reads
    // it (`carrier_object`): a domain scope reaches it.
    let visible = readable(
        "x.id",
        &format!("'{}'", M::KIND),
        "(SELECT m.domain_id FROM public.module_object m
           WHERE m.id = x.id AND m.valid_period @> $1::timestamptz)",
        "NULL::inet",
        "$1",
    );
    format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT {select}
           FROM {relation} x
          WHERE ($4::text IS NULL OR x.name = $4)
            AND ($5::text IS NULL OR (x.name, x.id) > ($5, $6::uuid))
            AND ($8::text IS NULL OR x.name LIKE $8)
            AND ($9::uuid IS NULL OR {cluster})
            AND ($10::uuid IS NULL OR {namespace})
            AND ($11::text IS NULL OR {sort})
            AND ($12::jsonb IS NULL OR {label})
            AND ($13::uuid IS NULL OR {machine})
            {absent}
            AND ($3::jsonb IS NULL OR {visible})
          ORDER BY x.name, x.id
          LIMIT $7",
        select = select::<M>(),
        relation = listed(M::TABLE, M::COLUMNS, current),
    )
}

/// `GET`/`QUERY` of the collection of `M`: by name, filtered in conjunction.
async fn list<M: Mirror>(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let collection = type_of(M::KIND).collection;
    let (page, prefix) = kit::suggestion(Page::from_query(&query, &api.ring, collection)?)?;
    let filter = filter_of::<M>(&page)?;
    let after = page
        .after()
        .map(|key| serde_json::from_value::<(String, Uuid)>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?;
    let now = kit::now(&api).await?;
    let moment = page.dates.at(now, api.bound)?;
    let scope = kit::scope(&api, &principal, now).await?;
    let sql = page_sql::<M>(moment.as_of.is_none(), filter.absent);
    // The rows of the page and their versions are read in one snapshot (task 021-11c).
    let mut snapshot = kit::snapshot(&api).await?;
    let mut rows: Vec<M> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        // A prefix `q` is planned with its value (an unnamed statement).
        .persistent(prefix.is_none())
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(scope.to_json())
        .bind(filter.name)
        .bind(after.as_ref().map(|(name, _)| name.as_str()))
        .bind(after.as_ref().map(|(_, id)| *id))
        .bind(i64::from(page.size) + 1)
        .bind(prefix.as_deref())
        .bind(filter.cluster)
        .bind(filter.namespace)
        .bind(filter.sort)
        .bind(filter.label)
        .bind(filter.machine)
        .fetch_all(&mut *snapshot)
        .await
        .map_err(|e| kit::store(&e))?;
    let size = usize::from(page.size);
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1];
        let cursor = page.next(
            &api.ring,
            collection,
            &moment,
            json!([last.name(), last.id()]),
        );
        next_href(&format!("/v1/{collection}"), &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().map(Mirror::id).collect();
    let versions = kit::versions(&mut *snapshot, &moment, &ids).await?;
    kit::release(snapshot);
    kit::record_read(
        &api,
        &context,
        M::KIND,
        &ids,
        criteria(collection, &page, &moment),
    )
    .await?;
    let items = kit::versioned(&rows, &versions);
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// `GET` of an object of `M`: 404, journaled, for one the caller may not read.
async fn show<M: Mirror>(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    kit::gate(&api, &context, type_of(M::KIND), id).await?;
    let now = kit::now(&api).await?;
    let moment = dates.at(now, api.bound)?;
    let sql = format!(
        "SELECT {} FROM {} x WHERE x.id = $3",
        select::<M>(),
        dated(M::TABLE, M::COLUMNS)
    );
    let (body, version): (M, Uuid) = kit::read_one(&api, sql, id, &moment).await?;
    kit::record_read(&api, &context, M::KIND, &[id], json!({"id": id})).await?;
    Ok(kit::respond_object(&headers, &moment, version, &body))
}

// ---------------------------------------------------------------- routes

/// A filter by the identifier of an object of `collection`.
fn by_id(op: kit::Op, name: &str, description: &str, collection: &str) -> kit::Op {
    op.filter(
        name,
        description,
        String::schema(),
        Some(json!({ "collection": collection })),
    )
}

/// The routes of the collection of `M`.
fn collection<M: Mirror>(router: ModuleRouter) -> ModuleRouter {
    let name = type_of(M::KIND).collection;
    let mut schemas = Vec::new();
    schemas_of::<M>(&mut schemas);
    schemas.push(listing_of::<M>());
    let mut list_op = op(&format!("list_{}", M::MANY), "kube", M::SUMMARY)
        .query("name", "Equal to.")
        .suggestion("the name");
    list_op = by_id(list_op, "cluster", "In this cluster.", "/v1/kube-clusters");
    if M::NAMESPACED {
        list_op = by_id(
            list_op,
            "namespace",
            "In this namespace.",
            "/v1/kube-namespaces",
        );
    }
    if let Some(sort) = &M::SORT {
        let values: RefOr<Schema> = ObjectBuilder::new()
            .schema_type(Type::String)
            .enum_values(Some(sort.values.iter().copied()))
            .into();
        list_op = list_op.filter(sort.filter, "Equal to.", values, None);
    }
    if M::LABELED {
        list_op = list_op.query(
            "label",
            "Carrying the tracked label `key=value` (the text up to the first `=` is the key).",
        );
    }
    if M::MACHINE {
        list_op = by_id(list_op, "machine", "This machine.", "/v1/machines");
    }
    let list_op = list_op
        .filter(
            ABSENT,
            "Absent from its cluster (`true`) or present (`false`).",
            bool::schema(),
            None,
        )
        .params::<DatesParams>()
        .params::<PageParams>()
        .answers(
            "200",
            "A page of the collection.",
            &format!("Listing_{}", <M as ToSchema>::name()),
        )
        .build();
    let the = "The object.";
    let show_op = op(
        &format!("show_{}", M::ONE),
        "kube",
        "An object of the collection; 404 for one the caller may not read.",
    )
    .path("id", the, true)
    .params::<DatesParams>()
    .answers("200", kit::OBJECT_READ, &<M as ToSchema>::name())
    .build();
    let path = format!("/{name}");
    let mut router = router.routes(documented(
        &path,
        vec![(HttpMethod::Get, list_op)],
        schemas,
        kit::collection(list::<M>),
    ));
    router = queryable(router, &path, &format!("query_{}", M::MANY));
    router = router.routes(documented(
        &format!("{path}/{{id}}"),
        vec![(HttpMethod::Get, show_op)],
        Vec::new(),
        get(show::<M>),
    ));
    presented(router, &path, M::KEY, M::SHOWN)
}

/// The routes of the collections of the mirrored objects.
pub(super) fn routes(router: ModuleRouter) -> ModuleRouter {
    let router = collection::<NodeDto>(router);
    let router = collection::<NamespaceDto>(router);
    let router = collection::<WorkloadDto>(router);
    let router = collection::<ServiceDto>(router);
    collection::<IngressDto>(router)
}
