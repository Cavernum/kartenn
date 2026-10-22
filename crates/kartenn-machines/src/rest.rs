//! The routes of the module under `/v1` (feature `rest`; spec 009, REQ-009-15 to 18;
//! design « Routes »), mounted by kartenn-server: clusters, machines and interfaces as
//! collections, dated, paged, filtered and readable as the routes of the core; the card of
//! a machine with its interfaces and addresses; the addresses of a machine, the machines
//! hosted by a machine or a cluster; writes by changeset, Merge Patch under `If-Match`;
//! the attachment and the detachment of an address under the `If-Match` of the interface.
//!
//! An object the caller may not read answers as an absent one (ADR 0021); a list keeps
//! the objects of the read scope of the caller, by the predicate of the core, and so does
//! every address shown.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Extension, Json};
use kartenn_rest::ModuleRouter;
use kartenn_rest::kit::{
    self, Api, Dates, DatesParams, ETag, ListQuery, MERGE_PATCH, Moment, Page, PageParams, Problem,
    ProblemKind, documented, listing_of, next_href, op, queryable, respond, respond_page,
    schemas_of,
};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::Principal;
use kartenn_service::module::ObjectType;
use kartenn_store_pg::rbac::{ReadScope, readable};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa::openapi::path::HttpMethod;
use utoipa::openapi::{RefOr, Schema};
use uuid::Uuid;

use crate::mac::Mac;
use crate::module::{CLUSTER, INTERFACE, MACHINE};

/// The members a creation of a cluster may carry.
const CLUSTER_CREATE: &[&str] = &["slug", "owner", "site"];
/// The members a patch of a cluster may carry; `site` may be `null`.
const CLUSTER_PATCH: &[&str] = &["slug", "owner", "site"];
/// The members a creation of a machine may carry.
const MACHINE_CREATE: &[&str] = &["name", "kind", "owner", "site", "host", "member_of"];
/// The members a patch of a machine may carry: its owner and site change by the core.
const MACHINE_PATCH: &[&str] = &["name", "kind", "host", "member_of"];
/// The members a creation of an interface may carry.
const INTERFACE_CREATE: &[&str] = &["machine", "name", "mac", "enabled", "vlan"];
/// The members a patch of an interface may carry: its machine is fixed (REQ-009-6).
const INTERFACE_PATCH: &[&str] = &["name", "mac", "enabled", "vlan"];

// ---------------------------------------------------------------- representations

/// The identifier of an object, as the operations name it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[schema(as = MachinesObjectRef)]
pub struct IdRef {
    /// Identifier.
    pub id: Uuid,
}

fn id_ref(id: Option<Uuid>) -> Option<IdRef> {
    id.map(|id| IdRef { id })
}

/// The host of a virtual machine or a container (M5): a machine or a cluster.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = MachineHost)]
pub enum HostRef {
    /// A machine.
    Machine(IdRef),
    /// A cluster.
    Cluster(IdRef),
}

/// A cluster of machines (REQ-009-3).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Cluster)]
pub struct ClusterDto {
    /// Identifier.
    pub id: Uuid,
    /// Slug, unique among the living clusters.
    pub slug: String,
    /// Its resolved owner.
    pub owner: Option<IdRef>,
    /// Its resolved site.
    pub site: Option<IdRef>,
    /// Its URL.
    pub href: String,
}

/// A machine (REQ-009-2).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Machine)]
pub struct MachineDto {
    /// Identifier.
    pub id: Uuid,
    /// Host name, lower case, unique per resolved owner.
    pub name: String,
    /// `physical`, `virtual`, `container` or `appliance`.
    pub kind: String,
    /// Its host.
    pub host: Option<HostRef>,
    /// The cluster it is a member of.
    pub member_of: Option<IdRef>,
    /// Its resolved owner.
    pub owner: Option<IdRef>,
    /// Its resolved site.
    pub site: Option<IdRef>,
    /// Its URL.
    pub href: String,
}

/// The 802.1Q mode of an interface (REQ-009-12).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = InterfaceVlan)]
pub struct VlanDto {
    /// `none`, `access` or `trunk`.
    pub mode: String,
    /// The VLAN of an access port.
    pub access: Option<IdRef>,
    /// The untagged VLAN of a trunk.
    pub native: Option<IdRef>,
    /// The tagged VLANs of a trunk.
    pub tagged: Vec<IdRef>,
}

/// An interface of a machine (REQ-009-6).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Interface)]
pub struct InterfaceDto {
    /// Identifier.
    pub id: Uuid,
    /// Its machine.
    pub machine: IdRef,
    /// Its name, unique on its machine.
    pub name: String,
    /// Its EUI-48, canonical (`aa:bb:cc:dd:ee:ff`).
    pub mac: Option<String>,
    /// Whether it is enabled.
    pub enabled: bool,
    /// Its 802.1Q mode.
    pub vlan: VlanDto,
    /// Its URL.
    pub href: String,
}

/// An address attached to an interface (REQ-009-16).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = MachineAddress)]
pub struct AddressDto {
    /// The interface that holds it.
    pub interface: IdRef,
    /// The name of the interface.
    pub interface_name: String,
    /// The address of the core.
    pub address: IdRef,
    /// Its value, without prefix length.
    pub value: String,
    /// Whether it is the primary address of its family on the interface.
    pub primary: bool,
    /// The URL of the address.
    pub href: String,
}

/// An interface on the card of its machine, with the addresses the caller may read.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = MachineCardInterface)]
pub struct CardInterface {
    /// Identifier.
    pub id: Uuid,
    /// Its name.
    pub name: String,
    /// Its EUI-48, canonical.
    pub mac: Option<String>,
    /// Whether it is enabled.
    pub enabled: bool,
    /// Its addresses, `(identifier, value)`.
    pub addresses: Vec<CardAddress>,
    /// Its URL.
    pub href: String,
}

/// An address on the card of a machine.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = MachineCardAddress)]
pub struct CardAddress {
    /// Identifier.
    pub id: Uuid,
    /// Its value.
    pub value: String,
    /// Whether it is primary for its family.
    pub primary: bool,
}

/// The card of a machine: the machine, its interfaces and their addresses (REQ-009-15).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = MachineCard)]
pub struct MachineCard {
    /// Identifier.
    pub id: Uuid,
    /// Host name.
    pub name: String,
    /// Its kind.
    pub kind: String,
    /// Its host.
    pub host: Option<HostRef>,
    /// The cluster it is a member of.
    pub member_of: Option<IdRef>,
    /// Its resolved owner.
    pub owner: Option<IdRef>,
    /// Its resolved site.
    pub site: Option<IdRef>,
    /// Its interfaces the caller may read, by name.
    pub interfaces: Vec<CardInterface>,
    /// Its URL.
    pub href: String,
}

/// The body of the creation of a cluster.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = ClusterCreation)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct ClusterCreation {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Slug.
    slug: String,
    /// The owner (`core.owner`).
    owner: IdRef,
    /// The site (`core.site`).
    site: Option<IdRef>,
}

/// The body of the modification of a cluster (JSON Merge Patch); `site: null` removes it.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = ClusterPatch)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct ClusterPatch {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Slug.
    slug: Option<String>,
    /// The owner.
    owner: Option<IdRef>,
    /// The site.
    site: Option<IdRef>,
}

/// The body of the creation of a machine.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = MachineCreation)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct MachineCreation {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Host name.
    name: String,
    /// `physical`, `virtual`, `container` or `appliance`.
    kind: String,
    /// The owner (`core.owner`).
    owner: IdRef,
    /// The site (`core.site`).
    site: Option<IdRef>,
    /// The host of a virtual machine or a container.
    host: Option<HostRef>,
    /// The cluster of a physical machine.
    member_of: Option<IdRef>,
}

/// The body of the modification of a machine (JSON Merge Patch); `host: null` and
/// `member_of: null` remove them. Its owner and site change by the operations of the core.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = MachinePatch)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct MachinePatch {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Host name.
    name: Option<String>,
    /// Kind.
    kind: Option<String>,
    /// Host.
    host: Option<HostRef>,
    /// Cluster.
    member_of: Option<IdRef>,
}

/// The 802.1Q mode of an interface, as written.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = InterfaceVlanInput)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct VlanInput {
    /// `none`, `access` or `trunk`.
    mode: String,
    /// The VLAN of an access port.
    access: Option<IdRef>,
    /// The untagged VLAN of a trunk.
    native: Option<IdRef>,
    /// The tagged VLANs of a trunk.
    tagged: Option<Vec<IdRef>>,
}

/// The body of the creation of an interface.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = InterfaceCreation)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct InterfaceCreation {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Its machine, fixed.
    machine: IdRef,
    /// Its name.
    name: String,
    /// Its EUI-48, any usual notation.
    mac: Option<String>,
    /// Enabled, by default.
    enabled: Option<bool>,
    /// Its 802.1Q mode, `none` by default.
    vlan: Option<VlanInput>,
}

/// The body of the modification of an interface (JSON Merge Patch); `mac: null` and
/// `vlan: null` remove them.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = InterfacePatch)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct InterfacePatch {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// Its name.
    name: Option<String>,
    /// Its EUI-48.
    mac: Option<String>,
    /// Whether it is enabled.
    enabled: Option<bool>,
    /// Its 802.1Q mode.
    vlan: Option<VlanInput>,
}

/// The body of the attachment of an address to an interface (REQ-009-9).
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = AttachmentCreation)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct AttachmentCreation {
    /// Why; required by the rules.
    motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// The address of the core.
    address: IdRef,
    /// The primary address of its family on the interface.
    primary: Option<bool>,
}

/// The optional body of the closure of an object of the module, or of a detachment.
#[derive(Debug, Deserialize, ToSchema)]
#[schema(as = MachinesClosure)]
#[allow(dead_code)] // the contract of the body; the route reads it as members
struct Closure {
    /// Why; required by the rules.
    motive: String,
    /// The start of the closure, RFC 3339; now by default.
    valid_from: Option<String>,
    /// Also close or detach what cites the object (REQ-009-14).
    cascade: Option<bool>,
}

// ---------------------------------------------------------------- reading

/// The rows of `table` (a table of the module or of the core) valid at `$1`, as asserted at
/// `$2` (now when null): the relation of a dated read.
fn dated(table: &str, columns: &str) -> String {
    format!(
        "(SELECT {columns} FROM {table}
           WHERE valid_period @> $1::timestamptz
             AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz)
          UNION ALL
          SELECT {columns} FROM {table}_history
           WHERE $2::timestamptz IS NOT NULL AND valid_period @> $1::timestamptz
             AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz)"
    )
}

/// [`dated`] restricted by `condition` in each branch: correlated in a `LATERAL` join, it
/// keeps the read on the indexes of `condition` even in a generic plan, where a join on
/// the whole relation would scan every row valid at `$1`.
fn dated_where(table: &str, columns: &str, condition: &str) -> String {
    format!(
        "(SELECT {columns} FROM {table}
           WHERE {condition} AND valid_period @> $1::timestamptz
             AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz)
          UNION ALL
          SELECT {columns} FROM {table}_history
           WHERE {condition} AND $2::timestamptz IS NOT NULL
             AND valid_period @> $1::timestamptz
             AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz)"
    )
}

/// The condition that the row `x` of the type `kind` of the module is readable at `$1`
/// under the scope `s`.
fn visible(alias: &str, kind: &str) -> String {
    readable(
        &format!("{alias}.id"),
        &format!("'{kind}'"),
        "NULL::uuid",
        "NULL::inet",
        "$1",
    )
}

/// The condition that the address `x` of the core is readable at `$1` under `s`.
fn address_visible(alias: &str) -> String {
    readable(
        &format!("{alias}.id"),
        "'address'",
        &format!("{alias}.domain_id"),
        &format!("{alias}.address"),
        "$1",
    )
}

const CLUSTER_COLUMNS: &str = "id, slug";
const MACHINE_COLUMNS: &str = "id, name, kind, host_machine_id, host_cluster_id, member_cluster_id";
const INTERFACE_COLUMNS: &str = "id, machine_id, name, mac, enabled, vlan_mode, access_vlan_id, native_vlan_id, tagged_vlan_ids";

type ClusterRow = (Uuid, String, Option<Uuid>, Option<Uuid>);
type MachineRow = (
    Uuid,
    String,
    String,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
);
#[derive(Debug, sqlx::FromRow)]
struct InterfaceRow {
    id: Uuid,
    machine_id: Uuid,
    name: String,
    mac: Option<String>,
    enabled: bool,
    vlan_mode: String,
    access_vlan_id: Option<Uuid>,
    native_vlan_id: Option<Uuid>,
    tagged_vlan_ids: Vec<Uuid>,
}

/// The select list of a machine: its columns, then its resolved owner and site at `$1`.
const MACHINE_SELECT: &str = "x.id, x.name, x.kind, x.host_machine_id, x.host_cluster_id, \
     x.member_cluster_id, kartenn_owner(x.id, $1), kartenn_site(x.id, $1)";
const CLUSTER_SELECT: &str = "x.id, x.slug, kartenn_owner(x.id, $1), kartenn_site(x.id, $1)";
const INTERFACE_SELECT: &str = "x.id, x.machine_id, x.name, x.mac::text AS mac, x.enabled, \
     x.vlan_mode, x.access_vlan_id, x.native_vlan_id, x.tagged_vlan_ids";

fn cluster_dto((id, slug, owner, site): ClusterRow) -> ClusterDto {
    ClusterDto {
        id,
        slug,
        owner: id_ref(owner),
        site: id_ref(site),
        href: format!("/v1/clusters/{id}"),
    }
}

fn host(machine: Option<Uuid>, cluster: Option<Uuid>) -> Option<HostRef> {
    match (machine, cluster) {
        (Some(id), _) => Some(HostRef::Machine(IdRef { id })),
        (None, Some(id)) => Some(HostRef::Cluster(IdRef { id })),
        (None, None) => None,
    }
}

fn machine_dto(row: MachineRow) -> MachineDto {
    let (id, name, kind, host_machine, host_cluster, member, owner, site) = row;
    MachineDto {
        id,
        name,
        kind,
        host: host(host_machine, host_cluster),
        member_of: id_ref(member),
        owner: id_ref(owner),
        site: id_ref(site),
        href: format!("/v1/machines/{id}"),
    }
}

fn interface_dto(row: InterfaceRow) -> InterfaceDto {
    InterfaceDto {
        id: row.id,
        machine: IdRef { id: row.machine_id },
        name: row.name,
        mac: row.mac,
        enabled: row.enabled,
        vlan: VlanDto {
            mode: row.vlan_mode,
            access: id_ref(row.access_vlan_id),
            native: id_ref(row.native_vlan_id),
            tagged: row
                .tagged_vlan_ids
                .into_iter()
                .map(|id| IdRef { id })
                .collect(),
        },
        href: format!("/v1/interfaces/{}", row.id),
    }
}

/// The filters of a list, each given once, among `names`.
fn filters<'a>(
    page: &'a Page,
    names: &[&str],
) -> Result<std::collections::BTreeMap<&'a str, &'a str>, Problem> {
    let mut found = std::collections::BTreeMap::new();
    for (name, value) in &page.filters {
        if !names.contains(&name.as_str()) {
            return Err(kit::bad(format!("unknown parameter {name}")));
        }
        if found.insert(name.as_str(), value.as_str()).is_some() {
            return Err(kit::bad(format!("{name} is given more than once")));
        }
    }
    Ok(found)
}

fn uuid(name: &str, text: &str) -> Result<Uuid, Problem> {
    text.parse()
        .map_err(|_| kit::bad(format!("{name} is not an identifier")))
}

fn optional_uuid(
    wanted: &std::collections::BTreeMap<&str, &str>,
    name: &str,
) -> Result<Option<Uuid>, Problem> {
    wanted.get(name).map(|v| uuid(name, v)).transpose()
}

/// The dates and the read scope of a list.
async fn reading(
    api: &Api,
    principal: &Principal,
    page: &Page,
) -> Result<(Moment, ReadScope), Problem> {
    let now = kit::now(api).await?;
    let moment = page.dates.at(now, api.bound)?;
    let scope = kit::scope(api, principal, now).await?;
    Ok((moment, scope))
}

fn criteria(collection: &str, page: &Page, moment: &Moment) -> Value {
    json!({
        "collection": collection,
        "filters": page.filters,
        "valid_at": rfc3339(moment.valid_at),
        "as_of": rfc3339(moment.as_of()),
    })
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&time::format_description::well_known::Rfc3339)
        .expect("an instant formats")
}

/// The cursor after which a page starts, `(sort key, identifier)`.
fn after<T: serde::de::DeserializeOwned>(page: &Page) -> Result<Option<T>, Problem> {
    page.after()
        .map(|key| serde_json::from_value::<T>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))
}

/// `GET`/`QUERY /v1/clusters`: by slug, filtered by `slug`.
async fn list_clusters(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    const NAME: &str = "clusters";
    let page = Page::from_query(&query, &api.ring, NAME)?;
    let wanted = filters(&page, &["slug"])?;
    let after = after::<(String, Uuid)>(&page)?;
    let (moment, scope) = reading(&api, &principal, &page).await?;
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT {CLUSTER_SELECT}
           FROM {} x
          WHERE ($4::text IS NULL OR x.slug = $4)
            AND ($5::text IS NULL OR (x.slug, x.id) > ($5, $6::uuid))
            AND ($3::jsonb IS NULL OR {})
          ORDER BY x.slug, x.id
          LIMIT $7",
        dated("machines.cluster", CLUSTER_COLUMNS),
        visible("x", CLUSTER),
    );
    let mut rows: Vec<ClusterRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(scope.to_json())
        .bind(wanted.get("slug").copied())
        .bind(after.as_ref().map(|(slug, _)| slug.as_str()))
        .bind(after.as_ref().map(|(_, id)| *id))
        .bind(i64::from(page.size) + 1)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    let size = usize::from(page.size);
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1];
        let cursor = page.next(&api.ring, NAME, &moment, json!([last.1, last.0]));
        next_href(&format!("/v1/{NAME}"), &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    kit::record_read(
        &api,
        &context,
        CLUSTER,
        &ids,
        criteria(NAME, &page, &moment),
    )
    .await?;
    let items: Vec<ClusterDto> = rows.into_iter().map(cluster_dto).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// What restricts a list of machines.
#[derive(Debug, Default)]
struct MachineFilter<'a> {
    name: Option<&'a str>,
    name_prefix: Option<&'a str>,
    kind: Option<&'a str>,
    /// Hosted by this machine or this cluster.
    host: Option<Uuid>,
    /// Hosted by or member of this cluster.
    cluster: Option<Uuid>,
    owner: Option<Uuid>,
    site: Option<Uuid>,
    /// Holding an address of this network.
    network: Option<Uuid>,
}

/// A page of the machines of `filter` the caller may read, by name, answered under the
/// path `path` (`machines`, `machines/{id}/hosted`, `clusters/{id}/machines`).
async fn machines_page(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    headers: &HeaderMap,
    page: &Page,
    path: &str,
    filter: &MachineFilter<'_>,
) -> Result<Response, Problem> {
    let after = after::<(String, Uuid)>(page)?;
    let (moment, scope) = reading(api, principal, page).await?;
    // Correlated in `LATERAL` joins from the network down: the generic plan the pool's
    // prepared statement ends on keeps to the indexes (each address valid at `$1` scanned
    // otherwise, task 010-19).
    let network = format!(
        "x.id IN (SELECT i.machine_id FROM {p} p
                    CROSS JOIN LATERAL {a} a
                    CROSS JOIN LATERAL {t} t
                    CROSS JOIN LATERAL {i} i)",
        p = dated_where("public.prefix", "id, domain_id, prefix", "id = $13"),
        a = dated_where(
            "public.address",
            "id",
            "domain_id = p.domain_id AND address <<= p.prefix"
        ),
        t = dated_where("machines.attachment", "interface_id", "address_id = a.id"),
        i = dated_where("machines.interface", "machine_id", "id = t.interface_id"),
    );
    // A machine is the root of its tree and inherits no owner (`module::TYPES`): its
    // resolved owner is its own assignment of `core.owner`. Read from the owner, a hashed
    // semi-join; `kartenn_owner` on each row (a function that is not inlined) cost 0.7 s
    // in the custom plan and 2 s in the generic one at 100 000 machines (task 009-12).
    let owned = "SELECT a.object_id FROM public.property_assignment a
                   JOIN public.property_declaration d
                     ON d.id = a.property_id AND d.key = 'core.owner'
                    AND d.valid_period @> $1::timestamptz
                  WHERE a.v_uuid = $9 AND a.object_type = 'machines.machine'
                    AND a.kind = 'value' AND a.valid_period @> $1::timestamptz";
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT {MACHINE_SELECT}
           FROM {} x
          WHERE ($4::text IS NULL OR x.name = $4)
            AND ($5::text IS NULL OR starts_with(x.name, $5))
            AND ($6::text IS NULL OR x.kind = $6)
            AND ($7::uuid IS NULL OR x.host_machine_id = $7 OR x.host_cluster_id = $7)
            AND ($8::uuid IS NULL OR x.host_cluster_id = $8 OR x.member_cluster_id = $8)
            AND ($9::uuid IS NULL OR x.id IN ({owned}))
            AND ($10::uuid IS NULL OR kartenn_site(x.id, $1) = $10)
            AND ($11::text IS NULL OR (x.name, x.id) > ($11, $12::uuid))
            AND ($13::uuid IS NULL OR {network})
            AND ($3::jsonb IS NULL OR {})
          ORDER BY x.name, x.id
          LIMIT $14",
        dated("machines.machine", MACHINE_COLUMNS),
        visible("x", MACHINE),
    );
    let mut rows: Vec<MachineRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(scope.to_json())
        .bind(filter.name)
        .bind(filter.name_prefix)
        .bind(filter.kind)
        .bind(filter.host)
        .bind(filter.cluster)
        .bind(filter.owner)
        .bind(filter.site)
        .bind(after.as_ref().map(|(name, _)| name.as_str()))
        .bind(after.as_ref().map(|(_, id)| *id))
        .bind(filter.network)
        .bind(i64::from(page.size) + 1)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    let size = usize::from(page.size);
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1];
        let cursor = page.next(&api.ring, path, &moment, json!([last.1, last.0]));
        next_href(&format!("/v1/{path}"), &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    kit::record_read(api, context, MACHINE, &ids, criteria(path, page, &moment)).await?;
    let items: Vec<MachineDto> = rows.into_iter().map(machine_dto).collect();
    Ok(respond_page(headers, &moment, &items, next.as_deref()))
}

/// `GET`/`QUERY /v1/machines`: by name, filtered by `name`, `name_prefix`, `kind`, `host`,
/// `cluster`, `owner`, `site` and `network`, in conjunction (REQ-009-15, 16).
async fn list_machines(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    const NAME: &str = "machines";
    let page = Page::from_query(&query, &api.ring, NAME)?;
    let wanted = filters(
        &page,
        &[
            "name",
            "name_prefix",
            "kind",
            "host",
            "cluster",
            "owner",
            "site",
            "network",
        ],
    )?;
    if let Some(kind) = wanted.get("kind")
        && crate::hosting::MachineKind::from_name(kind).is_none()
    {
        return Err(kit::bad(
            "kind must be physical, virtual, container or appliance",
        ));
    }
    let filter = MachineFilter {
        name: wanted.get("name").copied(),
        name_prefix: wanted.get("name_prefix").copied(),
        kind: wanted.get("kind").copied(),
        host: optional_uuid(&wanted, "host")?,
        cluster: optional_uuid(&wanted, "cluster")?,
        owner: optional_uuid(&wanted, "owner")?,
        site: optional_uuid(&wanted, "site")?,
        network: optional_uuid(&wanted, "network")?,
    };
    machines_page(&api, &principal, &context, &headers, &page, NAME, &filter).await
}

/// `GET /v1/machines/{id}/hosted`: the machines hosted by the machine.
async fn machine_hosted(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let path = format!("machines/{id}/hosted");
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &path)?;
    page.unfiltered()?;
    kit::gate(&api, &context, crate::module::machine_type(), id).await?;
    let filter = MachineFilter {
        host: Some(id),
        ..MachineFilter::default()
    };
    machines_page(&api, &principal, &context, &headers, &page, &path, &filter).await
}

/// `GET /v1/clusters/{id}/machines`: the members of the cluster and the machines it hosts.
async fn cluster_machines(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let path = format!("clusters/{id}/machines");
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &path)?;
    page.unfiltered()?;
    kit::gate(&api, &context, crate::module::cluster_type(), id).await?;
    let filter = MachineFilter {
        cluster: Some(id),
        ..MachineFilter::default()
    };
    machines_page(&api, &principal, &context, &headers, &page, &path, &filter).await
}

/// `GET`/`QUERY /v1/interfaces`: by machine then name, filtered by `machine`, `name`,
/// `mac`, `address` (the interface that holds it at the date) and `vlan` (access, native
/// or tagged), in conjunction (REQ-009-15).
async fn list_interfaces(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    const NAME: &str = "interfaces";
    let page = Page::from_query(&query, &api.ring, NAME)?;
    let wanted = filters(&page, &["machine", "name", "mac", "address", "vlan"])?;
    let machine = optional_uuid(&wanted, "machine")?;
    let address = optional_uuid(&wanted, "address")?;
    let vlan = optional_uuid(&wanted, "vlan")?;
    let mac = wanted
        .get("mac")
        .map(|m| {
            Mac::parse(m)
                .map(|m| m.to_string())
                .map_err(|_| kit::bad("mac is not an EUI-48"))
        })
        .transpose()?;
    let after = after::<(Uuid, String, Uuid)>(&page)?;
    let (moment, scope) = reading(&api, &principal, &page).await?;
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT {INTERFACE_SELECT}
           FROM {} x
          WHERE ($4::uuid IS NULL OR x.machine_id = $4)
            AND ($5::text IS NULL OR x.name = $5)
            AND ($6::macaddr IS NULL OR x.mac = $6::macaddr)
            AND ($7::uuid IS NULL OR EXISTS (
                     SELECT 1 FROM {} a WHERE a.interface_id = x.id AND a.address_id = $7))
            AND ($8::uuid IS NULL OR x.access_vlan_id = $8 OR x.native_vlan_id = $8
                 OR $8 = ANY(x.tagged_vlan_ids))
            AND ($9::uuid IS NULL OR (x.machine_id, x.name, x.id) > ($9, $10::text, $11::uuid))
            AND ($3::jsonb IS NULL OR {})
          ORDER BY x.machine_id, x.name, x.id
          LIMIT $12",
        dated("machines.interface", INTERFACE_COLUMNS),
        dated("machines.attachment", "interface_id, address_id"),
        visible("x", INTERFACE),
    );
    let mut rows: Vec<InterfaceRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(scope.to_json())
        .bind(machine)
        .bind(wanted.get("name").copied())
        .bind(mac.as_deref())
        .bind(address)
        .bind(vlan)
        .bind(after.as_ref().map(|a| a.0))
        .bind(after.as_ref().map(|a| a.1.as_str()))
        .bind(after.as_ref().map(|a| a.2))
        .bind(i64::from(page.size) + 1)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    let size = usize::from(page.size);
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1];
        let cursor = page.next(
            &api.ring,
            NAME,
            &moment,
            json!([last.machine_id, last.name, last.id]),
        );
        next_href(&format!("/v1/{NAME}"), &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    kit::record_read(
        &api,
        &context,
        INTERFACE,
        &ids,
        criteria(NAME, &page, &moment),
    )
    .await?;
    let items: Vec<InterfaceDto> = rows.into_iter().map(interface_dto).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// The dates of a read by identifier, once the caller may read the object `id` of `ty`.
async fn dated_read(
    api: &Api,
    context: &RequestContext,
    ty: &'static ObjectType,
    id: Uuid,
    dates: Dates,
) -> Result<Moment, Problem> {
    kit::gate(api, context, ty, id).await?;
    let now = kit::now(api).await?;
    dates.at(now, api.bound)
}

/// The answer of a read by identifier: the object, its version as `ETag`, the read
/// journaled.
async fn answer<T: Serialize>(
    api: &Api,
    context: &RequestContext,
    headers: &HeaderMap,
    kind: &str,
    id: Uuid,
    moment: &Moment,
    body: T,
) -> Result<Response, Problem> {
    let version = kit::version(api, id, moment)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    kit::record_read(api, context, kind, &[id], json!({"id": id})).await?;
    Ok(respond(
        headers,
        moment,
        &ETag::version(version),
        Json(body),
    ))
}

/// `GET /v1/clusters/{id}`.
async fn show_cluster(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = dated_read(&api, &context, crate::module::cluster_type(), id, dates).await?;
    let sql = format!(
        "SELECT {CLUSTER_SELECT} FROM {} x WHERE x.id = $3",
        dated("machines.cluster", CLUSTER_COLUMNS)
    );
    let row: ClusterRow = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(id)
        .fetch_optional(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    answer(
        &api,
        &context,
        &headers,
        CLUSTER,
        id,
        &moment,
        cluster_dto(row),
    )
    .await
}

/// `GET /v1/machines/{id}`: the card of the machine, its interfaces the caller may read and
/// their addresses it may read (REQ-009-15).
async fn show_machine(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = dated_read(&api, &context, crate::module::machine_type(), id, dates).await?;
    let sql = format!(
        "SELECT {MACHINE_SELECT} FROM {} x WHERE x.id = $3",
        dated("machines.machine", MACHINE_COLUMNS)
    );
    let row: MachineRow = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(id)
        .fetch_optional(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let now = kit::now(&api).await?;
    let scope = kit::scope(&api, &principal, now).await?;
    let interfaces: Vec<InterfaceRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT {INTERFACE_SELECT} FROM {} x
          WHERE x.machine_id = $4 AND ($3::jsonb IS NULL OR {})
          ORDER BY x.name, x.id",
        dated("machines.interface", INTERFACE_COLUMNS),
        visible("x", INTERFACE),
    )))
    .bind(moment.valid_at)
    .bind(moment.as_of)
    .bind(scope.to_json())
    .bind(id)
    .fetch_all(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    let held = interface_ids(&interfaces);
    let addresses = attached(&api, &moment, &scope, &held).await?;
    let machine = machine_dto(row);
    let card = MachineCard {
        id: machine.id,
        name: machine.name,
        kind: machine.kind,
        host: machine.host,
        member_of: machine.member_of,
        owner: machine.owner,
        site: machine.site,
        interfaces: interfaces
            .into_iter()
            .map(|i| CardInterface {
                addresses: addresses
                    .iter()
                    .filter(|a| a.interface.id == i.id)
                    .map(|a| CardAddress {
                        id: a.address.id,
                        value: a.value.clone(),
                        primary: a.primary,
                    })
                    .collect(),
                id: i.id,
                name: i.name,
                mac: i.mac,
                enabled: i.enabled,
                href: format!("/v1/interfaces/{}", i.id),
            })
            .collect(),
        href: machine.href,
    };
    answer(&api, &context, &headers, MACHINE, id, &moment, card).await
}

fn interface_ids(rows: &[InterfaceRow]) -> Vec<Uuid> {
    rows.iter().map(|r| r.id).collect()
}

/// The addresses the caller may read attached to `interfaces` at `moment`, by interface
/// then value.
async fn attached(
    api: &Api,
    moment: &Moment,
    scope: &ReadScope,
    interfaces: &[Uuid],
) -> Result<Vec<AddressDto>, Problem> {
    type Row = (Uuid, String, Uuid, String, bool);
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT i.id, i.name, x.id, host(x.address), a.primary_for_family
           FROM unnest($4::uuid[]) AS held(id)
           CROSS JOIN LATERAL {} i
           CROSS JOIN LATERAL {} a
           CROSS JOIN LATERAL {} x
          WHERE $3::jsonb IS NULL OR {}
          ORDER BY i.name, i.id, x.address",
        dated_where("machines.interface", "id, name", "id = held.id"),
        dated_where(
            "machines.attachment",
            "interface_id, address_id, primary_for_family",
            "interface_id = i.id"
        ),
        dated_where(
            "public.address",
            "id, domain_id, address",
            "id = a.address_id"
        ),
        address_visible("x"),
    )))
    .bind(moment.valid_at)
    .bind(moment.as_of)
    .bind(scope.to_json())
    .bind(interfaces)
    .fetch_all(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    Ok(rows
        .into_iter()
        .map(|(interface, name, address, value, primary)| AddressDto {
            interface: IdRef { id: interface },
            interface_name: name,
            address: IdRef { id: address },
            value,
            primary,
            href: format!("/v1/addresses/{address}"),
        })
        .collect())
}

/// `GET /v1/machines/{id}/addresses`: the addresses of the machine at the date, by
/// interface, those the caller may read on interfaces it may read (REQ-009-16).
async fn machine_addresses(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = dated_read(&api, &context, crate::module::machine_type(), id, dates).await?;
    let now = kit::now(&api).await?;
    let scope = kit::scope(&api, &principal, now).await?;
    let interfaces: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($3))
         SELECT x.id FROM {} x
          WHERE x.machine_id = $4 AND ($3::jsonb IS NULL OR {})",
        dated("machines.interface", "id, machine_id"),
        visible("x", INTERFACE),
    )))
    .bind(moment.valid_at)
    .bind(moment.as_of)
    .bind(scope.to_json())
    .bind(id)
    .fetch_all(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    let items = attached(&api, &moment, &scope, &interfaces).await?;
    let ids: Vec<Uuid> = items.iter().map(|a| a.address.id).collect();
    kit::record_read(
        &api,
        &context,
        "address",
        &ids,
        json!({"addresses_of_machine": id}),
    )
    .await?;
    Ok(respond_page(&headers, &moment, &items, None))
}

/// `GET /v1/interfaces/{id}`.
async fn show_interface(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = dated_read(&api, &context, crate::module::interface_type(), id, dates).await?;
    let sql = format!(
        "SELECT {INTERFACE_SELECT} FROM {} x WHERE x.id = $3",
        dated("machines.interface", INTERFACE_COLUMNS)
    );
    let row: InterfaceRow = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(moment.valid_at)
        .bind(moment.as_of)
        .bind(id)
        .fetch_optional(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    answer(
        &api,
        &context,
        &headers,
        INTERFACE,
        id,
        &moment,
        interface_dto(row),
    )
    .await
}

// ---------------------------------------------------------------- the routes

/// The schemas of the routes.
fn schemas() -> Vec<(String, RefOr<Schema>)> {
    let mut schemas = Vec::new();
    schemas_of::<ClusterDto>(&mut schemas);
    schemas_of::<MachineDto>(&mut schemas);
    schemas_of::<MachineCard>(&mut schemas);
    schemas_of::<InterfaceDto>(&mut schemas);
    schemas_of::<AddressDto>(&mut schemas);
    schemas_of::<ClusterCreation>(&mut schemas);
    schemas_of::<ClusterPatch>(&mut schemas);
    schemas_of::<MachineCreation>(&mut schemas);
    schemas_of::<MachinePatch>(&mut schemas);
    schemas_of::<InterfaceCreation>(&mut schemas);
    schemas_of::<InterfacePatch>(&mut schemas);
    schemas_of::<AttachmentCreation>(&mut schemas);
    schemas_of::<Closure>(&mut schemas);
    schemas.push(listing_of::<ClusterDto>());
    schemas.push(listing_of::<MachineDto>());
    schemas.push(listing_of::<InterfaceDto>());
    schemas.push(listing_of::<AddressDto>());
    schemas
}

/// What a collection of the module writes: its operations, the members of each and those
/// a patch may remove.
struct Writes {
    create: &'static str,
    create_members: &'static [&'static str],
    modify: &'static str,
    modify_members: &'static [&'static str],
    nullable: &'static [&'static str],
}

/// The collection of `ty`, its objects and their writes; `show` is the read of one object
/// and `list` of the collection.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)] // the contract of each route beside its handlers
fn collection_routes(
    router: ModuleRouter,
    ty: &'static ObjectType,
    one: &str,
    dto: (&str, &str),
    bodies: (&str, &str),
    filters: &[(&str, &str)],
    writes: &'static Writes,
    handlers: (
        axum::routing::MethodRouter<Api>,
        axum::routing::MethodRouter<Api>,
    ),
) -> ModuleRouter {
    let name = ty.collection;
    let mut list_op = op(
        &format!("list_{name}"),
        "machines",
        &format!("The {name} the caller may read."),
    );
    for (filter, description) in filters {
        list_op = list_op.query(filter, description);
    }
    let list_op = list_op
        .params::<DatesParams>()
        .params::<PageParams>()
        .answers(
            "200",
            "A page of the collection.",
            &format!("Listing_{}", dto.0),
        )
        .build();
    let create_op = op(
        &format!("create_{one}"),
        "machines",
        &format!("Creates a {one}."),
    )
    .header(
        "Idempotency-Key",
        false,
        "Makes the write idempotent for 24 h.",
    )
    .body("application/json", bodies.0, true)
    .answers("201", "Created.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let (list, show) = handlers;
    let mut router = router.routes(documented(
        &format!("/{name}"),
        vec![(HttpMethod::Get, list_op), (HttpMethod::Post, create_op)],
        schemas(),
        list.post(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  headers: HeaderMap,
                  raw: Bytes| async move {
                kit::create(
                    &api,
                    &principal,
                    &context,
                    &headers,
                    &raw,
                    ty,
                    writes.create,
                    writes.create_members,
                )
                .await
            },
        ),
    ));
    router = queryable(router, &format!("/{name}"), &format!("query_{name}"));
    let the = format!("The {one}.");
    let show_op = op(&format!("show_{one}"), "machines", &the)
        .path("id", &the, true)
        .params::<DatesParams>()
        .answers("200", "The object; its version is its ETag.", dto.1)
        .build();
    let modify_op = op(
        &format!("modify_{one}"),
        "machines",
        &format!("Modifies the {one} (JSON Merge Patch)."),
    )
    .path("id", &the, true)
    .header("If-Match", true, "The version read, as its ETag.")
    .body(MERGE_PATCH, bodies.1, true)
    .answers("200", "Modified.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let close_op = op(
        &format!("close_{one}"),
        "machines",
        &format!("Closes the {one}."),
    )
    .path("id", &the, true)
    .header("If-Match", true, "The version read, as its ETag.")
    .body("application/json", "MachinesClosure", false)
    .answers("200", "Closed.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    router.routes(documented(
        &format!("/{name}/{{id}}"),
        vec![
            (HttpMethod::Get, show_op),
            (HttpMethod::Patch, modify_op),
            (HttpMethod::Delete, close_op),
        ],
        Vec::new(),
        show.patch(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  Path(id): Path<Uuid>,
                  headers: HeaderMap,
                  raw: Bytes| async move {
                kit::modify_nullable(
                    &api,
                    &principal,
                    &context,
                    id,
                    &headers,
                    &raw,
                    ty,
                    (writes.modify, writes.modify_members, writes.nullable),
                )
                .await
            },
        )
        .delete(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  Path(id): Path<Uuid>,
                  headers: HeaderMap,
                  raw: Bytes| async move {
                kit::close(&api, &principal, &context, id, &headers, &raw, ty).await
            },
        ),
    ))
}

static CLUSTER_WRITES: Writes = Writes {
    create: "machines.create_cluster",
    create_members: CLUSTER_CREATE,
    modify: "machines.update_cluster",
    modify_members: CLUSTER_PATCH,
    nullable: &["site"],
};
static MACHINE_WRITES: Writes = Writes {
    create: "machines.create_machine",
    create_members: MACHINE_CREATE,
    modify: "machines.update_machine",
    modify_members: MACHINE_PATCH,
    nullable: &["host", "member_of"],
};
static INTERFACE_WRITES: Writes = Writes {
    create: "machines.create_interface",
    create_members: INTERFACE_CREATE,
    modify: "machines.update_interface",
    modify_members: INTERFACE_PATCH,
    nullable: &["mac", "vlan"],
};

/// `POST /v1/interfaces/{id}/attachments`: attaches an address under the `If-Match` of the
/// interface (REQ-009-9, 18).
async fn attach(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    if raw.is_empty() {
        return Err(kit::bad("the body names the address"));
    }
    let mut fixed = Map::new();
    fixed.insert("interface".into(), json!({ "id": id }));
    kit::act(
        &api,
        &principal,
        &context,
        id,
        &headers,
        &raw,
        crate::module::interface_type(),
        ("machines.attach_address", &["address", "primary"], fixed),
        StatusCode::OK,
    )
    .await
}

/// `DELETE /v1/interfaces/{id}/attachments/{address}`: detaches the address under the
/// `If-Match` of the interface.
async fn detach(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path((id, address)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let mut fixed = Map::new();
    fixed.insert("interface".into(), json!({ "id": id }));
    fixed.insert("address".into(), json!({ "id": address }));
    kit::act(
        &api,
        &principal,
        &context,
        id,
        &headers,
        &raw,
        crate::module::interface_type(),
        ("machines.detach_address", &[], fixed),
        StatusCode::OK,
    )
    .await
}

/// The routes of the module, mounted under `/v1` (ADR 0026, clause 9).
#[must_use]
#[allow(clippy::too_many_lines)] // the contract of each route beside its handlers
pub fn routes() -> ModuleRouter {
    let mut router = ModuleRouter::new();
    router = collection_routes(
        router,
        crate::module::cluster_type(),
        "cluster",
        ("Cluster", "Cluster"),
        ("ClusterCreation", "ClusterPatch"),
        &[("slug", "Equal to.")],
        &CLUSTER_WRITES,
        (kit::collection(list_clusters), get(show_cluster)),
    );
    router = collection_routes(
        router,
        crate::module::machine_type(),
        "machine",
        ("Machine", "MachineCard"),
        ("MachineCreation", "MachinePatch"),
        &[
            ("name", "Equal to."),
            ("name_prefix", "Starting with."),
            ("kind", "`physical`, `virtual`, `container` or `appliance`."),
            ("host", "Hosted by this machine or this cluster."),
            ("cluster", "Hosted by or member of this cluster."),
            ("owner", "Whose resolved owner is this entity."),
            ("site", "Whose resolved site is this site."),
            ("network", "Holding an address of this network."),
        ],
        &MACHINE_WRITES,
        (kit::collection(list_machines), get(show_machine)),
    );
    router = collection_routes(
        router,
        crate::module::interface_type(),
        "interface",
        ("Interface", "Interface"),
        ("InterfaceCreation", "InterfacePatch"),
        &[
            ("machine", "Of this machine."),
            ("name", "Equal to."),
            ("mac", "This EUI-48, any usual notation."),
            ("address", "Holding this address at the date."),
            ("vlan", "Carrying this VLAN, access, native or tagged."),
        ],
        &INTERFACE_WRITES,
        (kit::collection(list_interfaces), get(show_interface)),
    );
    let addresses_op = op(
        "machine_addresses",
        "machines",
        "The addresses of the machine by interface, those the caller may read (REQ-009-16).",
    )
    .path("id", "The machine.", true)
    .params::<DatesParams>()
    .answers("200", "The addresses.", "Listing_MachineAddress")
    .build();
    let hosted_op = op(
        "machine_hosted",
        "machines",
        "The machines hosted by the machine, those the caller may read.",
    )
    .path("id", "The machine.", true)
    .params::<DatesParams>()
    .params::<PageParams>()
    .answers("200", "A page of machines.", "Listing_Machine")
    .build();
    let cluster_op = op(
        "cluster_machines",
        "machines",
        "The members of the cluster and the machines it hosts, those the caller may read.",
    )
    .path("id", "The cluster.", true)
    .params::<DatesParams>()
    .params::<PageParams>()
    .answers("200", "A page of machines.", "Listing_Machine")
    .build();
    let attach_op = op(
        "attach_address",
        "machines",
        "Attaches an address to the interface (REQ-009-9); the address is only read.",
    )
    .path("id", "The interface.", true)
    .header(
        "If-Match",
        true,
        "The version of the interface read, as its ETag.",
    )
    .body("application/json", "AttachmentCreation", true)
    .answers("200", "Attached.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let detach_op = op(
        "detach_address",
        "machines",
        "Detaches the address from the interface.",
    )
    .path("id", "The interface.", true)
    .path("address", "The address.", true)
    .header(
        "If-Match",
        true,
        "The version of the interface read, as its ETag.",
    )
    .body("application/json", "MachinesClosure", false)
    .answers("200", "Detached.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    router
        .routes(documented(
            "/machines/{id}/addresses",
            vec![(HttpMethod::Get, addresses_op)],
            Vec::new(),
            get(machine_addresses),
        ))
        .routes(documented(
            "/machines/{id}/hosted",
            vec![(HttpMethod::Get, hosted_op)],
            Vec::new(),
            get(machine_hosted),
        ))
        .routes(documented(
            "/clusters/{id}/machines",
            vec![(HttpMethod::Get, cluster_op)],
            Vec::new(),
            get(cluster_machines),
        ))
        .routes(documented(
            "/interfaces/{id}/attachments",
            vec![(HttpMethod::Post, attach_op)],
            Vec::new(),
            post(attach),
        ))
        .routes(documented(
            "/interfaces/{id}/attachments/{address}",
            vec![(HttpMethod::Delete, detach_op)],
            Vec::new(),
            delete(detach),
        ))
}
