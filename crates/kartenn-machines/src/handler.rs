//! The handler of the module in the write pipeline (feature `pg`; ADR 0026, clause 6):
//! the operations on clusters and machines (REQ-009-2 to 5). `owner` and `site` are
//! assignments of `core.owner` and `core.site` that the handler adds to the changeset,
//! ordinary operations of the core checked under their own rights; the name of a machine
//! is unique per resolved owner, checked on the final state (M3). An address of the core
//! is attached to an interface by a dated row of `machines.attachment`, a write of the
//! interface that needs only the reading of the address (M16).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use kartenn_core::objects::Slug;
use kartenn_core::period::Period;
use kartenn_core::rules::{Rule, Violation};
use kartenn_service::ObjectKind;
use kartenn_service::module::{
    BoxFuture, CheckContext, DiffContext, DiffSection, Findings, ModuleHandler, ModuleObjectRow,
    ModulePlan, ModuleTx, Reference,
};
use kartenn_service::operations::{ModuleKind, ModuleOperation, Operation, RawInput, Ref};
use kartenn_service::port::{StoreError, WriteError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::hosting::{Host, MachineKind, check_host, check_membership};
use crate::mac::Mac;
use crate::module::{ADDRESS, CLUSTER, INTERFACE, MACHINE, VLAN};
use crate::name::HostName;
use crate::pg::{self, Hosting};
use crate::rules::{
    ADDRESS_CLOSED, ADDRESS_TAKEN, ADDRESS_VLAN_MISMATCH, HOST_INVALID, HOST_SITE_MISMATCH,
    MAC_DUPLICATE, NAME_DUPLICATE, OPERATION_INVALID, VLAN_MODE,
};
use crate::vlan_mode::VlanMode;

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

fn machine_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(MACHINE, &KIND)
}

fn interface_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(INTERFACE, &KIND)
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
}

fn invalid(message: impl Into<String>) -> Violation {
    Violation::new(&OPERATION_INVALID, message.into())
}

/// What [`Handler::prepare`] reads for the plan: the properties of the core it assigns,
/// and the resolved owner of a machine it changes (its lock key).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Completed {
    #[serde(default)]
    owner_property: Option<Uuid>,
    #[serde(default)]
    site_property: Option<Uuid>,
    #[serde(default)]
    owner: Option<Uuid>,
    /// The machine of an interface a patch changes.
    #[serde(default)]
    machine: Option<Uuid>,
}

/// The host of a machine, as received.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum HostInput {
    Machine(Ref),
    Cluster(Ref),
}

/// `machines.create_cluster`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCluster {
    #[serde(default)]
    local: Option<u32>,
    slug: String,
    #[serde(default)]
    owner: Option<Ref>,
    #[serde(default)]
    site: Option<Ref>,
    #[serde(default)]
    completed: Completed,
}

/// `machines.create_machine`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateMachine {
    #[serde(default)]
    local: Option<u32>,
    name: String,
    kind: String,
    #[serde(default)]
    owner: Option<Ref>,
    #[serde(default)]
    site: Option<Ref>,
    #[serde(default)]
    host: Option<HostInput>,
    #[serde(default)]
    member_of: Option<Ref>,
    #[serde(default)]
    completed: Completed,
}

/// The fields a Merge Patch (RFC 7396) of a cluster or a machine may carry; `null`
/// removes the site, the host or the membership.
const CLUSTER_FIELDS: &[&str] = &["id", "slug", "owner", "site", "completed"];
const MACHINE_FIELDS: &[&str] = &["id", "name", "kind", "host", "member_of", "completed"];
/// The machine of an interface is fixed (REQ-009-6): it is not among the fields of a patch.
const INTERFACE_FIELDS: &[&str] = &["id", "name", "mac", "enabled", "vlan", "completed"];

/// The permission a VLAN cited by an interface needs (REQ-009-18).
const VLAN_READ: &str = "vlan.vlan.read";
/// The permission an address attached or detached needs, rather than its writing (M16).
const ADDRESS_READ: &str = "core.address.read";

/// `machines.attach_address` (REQ-009-9, 11).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attach {
    interface: Ref,
    address: Ref,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    completed: Completed,
}

/// `machines.detach_address`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Detach {
    interface: Ref,
    address: Ref,
    #[serde(default)]
    completed: Completed,
}

/// The 802.1Q mode of an interface, as received (REQ-009-12): `null` or absent is `none`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VlanInput {
    mode: String,
    #[serde(default)]
    access: Option<Ref>,
    #[serde(default)]
    native: Option<Ref>,
    #[serde(default)]
    tagged: Vec<Ref>,
}

/// The 802.1Q columns of the mode `value` (`null`: none), and the VLANs it cites.
fn vlan_columns(
    value: &Value,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<(Map<String, Value>, Vec<Uuid>), Violation> {
    let input: VlanInput = if value.is_null() {
        VlanInput {
            mode: "none".into(),
            access: None,
            native: None,
            tagged: Vec::new(),
        }
    } else {
        decode(value.clone(), "vlan")?
    };
    let vlan = |r: Ref| resolve(r).ok_or_else(|| invalid("vlan: unknown VLAN"));
    let access = input.access.map(vlan).transpose()?;
    let native = input.native.map(vlan).transpose()?;
    let tagged = input
        .tagged
        .into_iter()
        .map(vlan)
        .collect::<Result<Vec<_>, _>>()?;
    let mode = VlanMode::from_parts(&input.mode, access, native, &tagged)?;
    let (access, native, tagged): (Option<Uuid>, Option<Uuid>, Vec<Uuid>) = match &mode {
        VlanMode::None => (None, None, Vec::new()),
        VlanMode::Access(vlan) => (Some(*vlan), None, Vec::new()),
        VlanMode::Trunk { tagged, native } => (None, *native, tagged.iter().copied().collect()),
    };
    let mut columns = Map::new();
    columns.insert("vlan_mode".into(), mode.name().into());
    columns.insert("access_vlan_id".into(), json!(access));
    columns.insert("native_vlan_id".into(), json!(native));
    columns.insert("tagged_vlan_ids".into(), json!(tagged));
    Ok((columns, mode.vlans().into_iter().collect()))
}

/// The rights an operation needs on the objects it cites: `[permission, object]` pairs,
/// read back by [`ModuleHandler::reads`].
fn reads(permission: &str, objects: &[Uuid]) -> Value {
    objects.iter().map(|o| json!([permission, o])).collect()
}

/// `machines.create_interface`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInterface {
    #[serde(default)]
    local: Option<u32>,
    machine: Ref,
    name: String,
    #[serde(default)]
    mac: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    vlan: Option<Value>,
    #[serde(default)]
    completed: Completed,
}

/// The name of an interface: 1 to 64 printable characters without space (REQ-009-6).
fn interface_name(name: &str) -> Result<&str, Violation> {
    let length = name.chars().count();
    if (1..=64).contains(&length) && !name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        Ok(name)
    } else {
        Err(invalid(format!(
            "the name of an interface has 1 to 64 printable characters without space: {name:?}"
        )))
    }
}

/// The lock key of the interfaces of a machine.
fn machine_key(machine: Uuid) -> String {
    format!("machines.machine:{machine}")
}

/// The lock key of the attachments of an address (REQ-009-10).
fn address_key(address: Uuid) -> String {
    format!("machines.address:{address}")
}

/// The lock key of the attachments of an interface (its primary addresses, REQ-009-11).
fn interface_key(interface: Uuid) -> String {
    format!("machines.interface:{interface}")
}

fn decode<T: serde::de::DeserializeOwned>(value: Value, tag: &str) -> Result<T, Violation> {
    serde_json::from_value(value).map_err(|e| invalid(format!("{tag}: {e}")))
}

fn body(operation: &ModuleOperation, fields: &[&str]) -> Result<Map<String, Value>, Violation> {
    if let Some(unknown) = operation
        .body
        .keys()
        .find(|k| !fields.contains(&k.as_str()))
    {
        return Err(invalid(format!(
            "{}: unknown field `{unknown}`",
            operation.tag()
        )));
    }
    Ok(operation.body.clone())
}

/// The reference of `field` resolved, when given.
fn resolved(
    value: Option<&Value>,
    field: &str,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<Option<Uuid>, Violation> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let r: Ref = decode(v.clone(), field)?;
            resolve(r)
                .map(Some)
                .ok_or_else(|| invalid(format!("{field}: unknown object")))
        }
    }
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

/// What an operation does to the site of its object.
#[derive(Debug, Clone, Copy)]
enum Site {
    /// Nothing.
    Keep,
    /// Assigns it.
    Set(Uuid),
    /// Removes it (`null` in a Merge Patch).
    Remove,
}

/// The operations of the core for `owner` and `site`: an assignment for a value, a
/// removal for a `null` site.
fn anchors(
    kind: ModuleKind,
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
        core.push(assign(kind, object, property, owner));
    }
    match site {
        Site::Set(site) => {
            let property = completed
                .site_property
                .ok_or_else(|| undeclared("core.site"))?;
            core.push(assign(kind, object, property, site));
        }
        Site::Remove => {
            let property = completed
                .site_property
                .ok_or_else(|| undeclared("core.site"))?;
            core.push(Operation::UnassignProperty {
                object: Ref::Id(object),
                kind: ObjectKind::Module(kind),
                property: Ref::Id(property),
            });
        }
        Site::Keep => {}
    }
    Ok(core)
}

fn machine_kind_of(name: &str) -> Result<MachineKind, Violation> {
    MachineKind::from_name(name).ok_or_else(|| invalid(format!("unknown kind of machine {name}")))
}

/// The lock key of the names of an owner (M3).
fn name_key(owner: Uuid) -> String {
    format!("machines.name:{owner}")
}

/// The global lock key of the hosting, taken when a host changes (M5).
const HOSTING_KEY: &str = "machines.hosting";

/// The section of the differential of the attachments (REQ-009-23).
pub const ATTACHMENTS_SECTION: &str = "machines.attachments";

impl Handler {
    fn plan_create_cluster(
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let create: CreateCluster =
            decode(Value::Object(operation.body.clone()), &operation.tag())?;
        let _ = create.local;
        let slug = Slug::parse(&create.slug)?;
        let owner = match create.owner {
            Some(r) => Some(resolve(r).ok_or_else(|| invalid("owner: unknown object"))?),
            None => None,
        };
        let site = match create.site {
            Some(r) => Site::Set(resolve(r).ok_or_else(|| invalid("site: unknown object"))?),
            None => Site::Keep,
        };
        Ok(ModulePlan {
            kind: cluster_kind(),
            object,
            creates: true,
            domains: Vec::new(),
            keys: Vec::new(),
            core: anchors(cluster_kind(), object, owner, site, &create.completed)?,
            data: json!({ "create": { "slug": slug.as_str() } }),
        })
    }

    fn plan_update_cluster(
        operation: &ModuleOperation,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let body = body(operation, CLUSTER_FIELDS)?;
        let cluster =
            resolved(body.get("id"), "id", resolve)?.ok_or_else(|| invalid("id: required"))?;
        let completed: Completed = decode(
            body.get("completed").cloned().unwrap_or_default(),
            "completed",
        )
        .unwrap_or_default();
        let mut patch = Map::new();
        if let Some(slug) = body.get("slug") {
            let slug = slug.as_str().ok_or_else(|| invalid("slug: a string"))?;
            patch.insert("slug".into(), Slug::parse(slug)?.as_str().into());
        }
        let owner = match body.get("owner") {
            None => None,
            Some(Value::Null) => return Err(invalid("owner: a cluster keeps an owner")),
            Some(_) => resolved(body.get("owner"), "owner", resolve)?,
        };
        let site = match resolved(body.get("site"), "site", resolve)? {
            Some(site) => Site::Set(site),
            None if body.contains_key("site") => Site::Remove,
            None => Site::Keep,
        };
        Ok(ModulePlan {
            kind: cluster_kind(),
            object: cluster,
            creates: false,
            domains: Vec::new(),
            keys: Vec::new(),
            core: anchors(cluster_kind(), cluster, owner, site, &completed)?,
            data: json!({ "update": patch }),
        })
    }

    fn plan_create_machine(
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let create: CreateMachine =
            decode(Value::Object(operation.body.clone()), &operation.tag())?;
        let _ = create.local;
        let name = HostName::parse(&create.name)?;
        let kind = machine_kind_of(&create.kind)?;
        let owner = match create.owner {
            Some(r) => Some(resolve(r).ok_or_else(|| invalid("owner: unknown object"))?),
            None => None,
        };
        let site = match create.site {
            Some(r) => Site::Set(resolve(r).ok_or_else(|| invalid("site: unknown object"))?),
            None => Site::Keep,
        };
        let (host_machine, host_cluster) = match create.host {
            Some(HostInput::Machine(r)) => (
                Some(resolve(r).ok_or_else(|| invalid("host: unknown machine"))?),
                None,
            ),
            Some(HostInput::Cluster(r)) => {
                check_host(kind, Host::Cluster)?;
                (
                    None,
                    Some(resolve(r).ok_or_else(|| invalid("host: unknown cluster"))?),
                )
            }
            None => (None, None),
        };
        let member = match create.member_of {
            Some(r) => {
                check_membership(kind)?;
                Some(resolve(r).ok_or_else(|| invalid("member_of: unknown cluster"))?)
            }
            None => None,
        };
        let mut keys: Vec<String> = owner.map(name_key).into_iter().collect();
        if host_machine.is_some() {
            keys.push(HOSTING_KEY.into());
        }
        Ok(ModulePlan {
            kind: machine_kind(),
            object,
            creates: true,
            domains: Vec::new(),
            keys,
            core: anchors(machine_kind(), object, owner, site, &create.completed)?,
            data: json!({
                "create": {
                    "name": name.as_str(),
                    "kind": kind.as_str(),
                    "host_machine_id": host_machine,
                    "host_cluster_id": host_cluster,
                    "member_cluster_id": member,
                }
            }),
        })
    }

    fn plan_update_machine(
        operation: &ModuleOperation,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let body = body(operation, MACHINE_FIELDS)?;
        let machine =
            resolved(body.get("id"), "id", resolve)?.ok_or_else(|| invalid("id: required"))?;
        let completed: Completed = decode(
            body.get("completed").cloned().unwrap_or_default(),
            "completed",
        )
        .unwrap_or_default();
        let mut patch = Map::new();
        let mut keys = Vec::new();
        if let Some(name) = body.get("name") {
            let name = name.as_str().ok_or_else(|| invalid("name: a string"))?;
            patch.insert("name".into(), HostName::parse(name)?.as_str().into());
            keys.extend(completed.owner.map(name_key));
        }
        if let Some(kind) = body.get("kind") {
            let kind = kind.as_str().ok_or_else(|| invalid("kind: a string"))?;
            patch.insert("kind".into(), machine_kind_of(kind)?.as_str().into());
        }
        if let Some(host) = body.get("host") {
            let (machine_host, cluster_host) = if host.is_null() {
                (None, None)
            } else {
                match decode::<HostInput>(host.clone(), "host")? {
                    HostInput::Machine(r) => (
                        Some(resolve(r).ok_or_else(|| invalid("host: unknown machine"))?),
                        None,
                    ),
                    HostInput::Cluster(r) => (
                        None,
                        Some(resolve(r).ok_or_else(|| invalid("host: unknown cluster"))?),
                    ),
                }
            };
            if machine_host == Some(machine) {
                return Err(
                    Violation::new(&HOST_INVALID, "a machine does not host itself")
                        .with_subjects([machine]),
                );
            }
            patch.insert("host_machine_id".into(), json!(machine_host));
            patch.insert("host_cluster_id".into(), json!(cluster_host));
            keys.push(HOSTING_KEY.into());
        }
        if body.contains_key("member_of") {
            let member = resolved(body.get("member_of"), "member_of", resolve)?;
            patch.insert("member_cluster_id".into(), json!(member));
        }
        Ok(ModulePlan {
            kind: machine_kind(),
            object: machine,
            creates: false,
            domains: Vec::new(),
            keys,
            core: Vec::new(),
            data: json!({ "update": patch }),
        })
    }
}

impl Handler {
    fn plan_create_interface(
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let create: CreateInterface =
            decode(Value::Object(operation.body.clone()), &operation.tag())?;
        let _ = (create.local, create.completed);
        let machine = resolve(create.machine).ok_or_else(|| invalid("machine: unknown object"))?;
        let name = interface_name(&create.name)?;
        let mac = create.mac.as_deref().map(Mac::parse).transpose()?;
        let (vlan, vlans) = vlan_columns(create.vlan.as_ref().unwrap_or(&Value::Null), resolve)?;
        let mut columns = json!({
            "machine_id": machine,
            "name": name,
            "mac": mac.map(|m| m.to_string()),
            "enabled": create.enabled.unwrap_or(true),
        });
        columns.as_object_mut().expect("an object").extend(vlan);
        Ok(ModulePlan {
            kind: interface_kind(),
            object,
            creates: true,
            domains: Vec::new(),
            keys: vec![machine_key(machine)],
            core: Vec::new(),
            data: json!({ "create": columns, "reads": reads(VLAN_READ, &vlans) }),
        })
    }

    fn plan_update_interface(
        operation: &ModuleOperation,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let body = body(operation, INTERFACE_FIELDS)?;
        let interface =
            resolved(body.get("id"), "id", resolve)?.ok_or_else(|| invalid("id: required"))?;
        let completed: Completed = decode(
            body.get("completed").cloned().unwrap_or_default(),
            "completed",
        )
        .unwrap_or_default();
        let mut patch = Map::new();
        if let Some(name) = body.get("name") {
            let name = name.as_str().ok_or_else(|| invalid("name: a string"))?;
            patch.insert("name".into(), interface_name(name)?.into());
        }
        match body.get("mac") {
            None => {}
            Some(Value::Null) => {
                patch.insert("mac".into(), Value::Null);
            }
            Some(mac) => {
                let mac = mac.as_str().ok_or_else(|| invalid("mac: a string"))?;
                patch.insert("mac".into(), Mac::parse(mac)?.to_string().into());
            }
        }
        if let Some(enabled) = body.get("enabled") {
            let enabled = enabled
                .as_bool()
                .ok_or_else(|| invalid("enabled: a boolean"))?;
            patch.insert("enabled".into(), enabled.into());
        }
        let mut vlans = Vec::new();
        if let Some(vlan) = body.get("vlan") {
            let (columns, cited) = vlan_columns(vlan, resolve)?;
            patch.extend(columns);
            vlans = cited;
        }
        Ok(ModulePlan {
            kind: interface_kind(),
            object: interface,
            creates: false,
            domains: Vec::new(),
            keys: completed.machine.map(machine_key).into_iter().collect(),
            core: Vec::new(),
            data: json!({ "update": patch, "reads": reads(VLAN_READ, &vlans) }),
        })
    }
}

impl Handler {
    /// `attach_address` (`attach`) or `detach_address`: a write of the interface, keyed by
    /// the address and the interface.
    fn plan_attachment(
        operation: &ModuleOperation,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
        attach: bool,
    ) -> Result<ModulePlan, Violation> {
        let body = Value::Object(operation.body.clone());
        let (interface, address, primary) = if attach {
            let a: Attach = decode(body, &operation.tag())?;
            let _ = a.completed;
            (a.interface, a.address, a.primary)
        } else {
            let d: Detach = decode(body, &operation.tag())?;
            let _ = d.completed;
            (d.interface, d.address, false)
        };
        let interface = resolve(interface).ok_or_else(|| invalid("interface: unknown object"))?;
        let address = resolve(address).ok_or_else(|| invalid("address: unknown object"))?;
        let verb = if attach { "attach" } else { "detach" };
        Ok(ModulePlan {
            kind: interface_kind(),
            object: interface,
            creates: false,
            domains: Vec::new(),
            keys: vec![address_key(address), interface_key(interface)],
            core: Vec::new(),
            data: json!({
                verb: { "address": address, "primary": primary },
                "reads": reads(ADDRESS_READ, &[address]),
            }),
        })
    }
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
            let mut completed = Completed {
                owner_property: pg::property(conn, "core.owner", at).await?,
                site_property: pg::property(conn, "core.site", at).await?,
                owner: None,
                machine: None,
            };
            if operation.name == "update_machine"
                && let Some(Ok(Ref::Id(machine))) = operation
                    .body
                    .get("id")
                    .cloned()
                    .map(serde_json::from_value::<Ref>)
            {
                completed.owner = pg::owner(conn, machine, at).await?;
            }
            if operation.name == "update_interface"
                && let Some(Ok(Ref::Id(interface))) = operation
                    .body
                    .get("id")
                    .cloned()
                    .map(serde_json::from_value::<Ref>)
            {
                completed.machine = pg::machine_of(conn, interface).await?;
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
            "create_machine" => Self::plan_create_machine(operation, object, resolve),
            "update_machine" => Self::plan_update_machine(operation, resolve),
            "create_interface" => Self::plan_create_interface(operation, object, resolve),
            "update_interface" => Self::plan_update_interface(operation, resolve),
            "attach_address" => Self::plan_attachment(operation, resolve, true),
            "detach_address" => Self::plan_attachment(operation, resolve, false),
            other => Err(invalid(format!("machines.{other}: unknown operation"))),
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
            let id = plan.object;
            let attachment = ["attach", "detach"]
                .into_iter()
                .find_map(|verb| plan.data.get(verb).map(|a| (verb == "attach", a)));
            if let Some((attach, attachment)) = attachment {
                return apply_attachment(tx, id, attach, attachment, period, changeset).await;
            }
            if let Some(target) = plan.data.get("unlink") {
                return apply_unlink(tx, plan, target, period, changeset).await;
            }
            let conn = connection(tx)?;
            if plan.kind == interface_kind() {
                let machine = if let Some(create) = plan.data.get("create") {
                    let machine: Uuid = create["machine_id"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| StoreError("a planned machine".into()))?;
                    pg::insert_interface(conn, id, machine, create, period, changeset).await?;
                    machine
                } else {
                    let machine = pg::machine_of(conn, id)
                        .await?
                        .ok_or(WriteError::NoVersion(id))?;
                    pg::patch_interface(conn, id, machine, &plan.data["update"], period, changeset)
                        .await?;
                    machine
                };
                // Under its machine in the tree of the module: its anchors are inherited.
                return tx
                    .register(
                        ModuleObjectRow {
                            id,
                            kind: plan.kind,
                            parent: Some(machine),
                            domain: None,
                        },
                        period,
                        changeset,
                    )
                    .await;
            }
            match (plan.kind == cluster_kind(), plan.data.get("create")) {
                (true, Some(create)) => {
                    let slug = create["slug"].as_str().unwrap_or_default();
                    pg::insert_cluster(conn, id, slug, period, changeset).await?;
                }
                (true, None) => {
                    pg::patch_cluster(conn, id, &plan.data["update"], period, changeset).await?;
                }
                (false, Some(create)) => {
                    let uuid = |name: &str| create[name].as_str().and_then(|s| s.parse().ok());
                    let hosting = Hosting {
                        kind: create["kind"].as_str().unwrap_or_default().to_owned(),
                        host_machine_id: uuid("host_machine_id"),
                        host_cluster_id: uuid("host_cluster_id"),
                        member_cluster_id: uuid("member_cluster_id"),
                    };
                    let name = create["name"].as_str().unwrap_or_default();
                    pg::insert_machine(conn, id, name, &hosting, period, changeset).await?;
                }
                (false, None) => {
                    pg::patch_machine(conn, id, &plan.data["update"], period, changeset).await?;
                }
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
        Box::pin(async move {
            tx.restore_versioned(table(kind), id, before, period, changeset)
                .await?;
            // The attachments of an interface that `before` wrote.
            if kind == interface_kind() {
                let written = pg::attachments_written(connection(tx)?, id, before).await?;
                for attachment in written {
                    tx.restore_versioned(
                        pg::ATTACHMENT_TABLE,
                        attachment,
                        before,
                        period,
                        changeset,
                    )
                    .await?;
                }
            }
            Ok(())
        })
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

    fn nullify(&self, reference: &Reference, source: Uuid, target: Uuid) -> Option<ModulePlan> {
        let detach = |interface: Uuid, address: Uuid| ModulePlan {
            kind: interface_kind(),
            object: interface,
            creates: false,
            domains: Vec::new(),
            keys: vec![address_key(address), interface_key(interface)],
            core: Vec::new(),
            data: json!({
                "detach": { "address": address, "primary": false },
                "reads": reads(ADDRESS_READ, &[address]),
            }),
        };
        let unlink = |kind: ModuleKind, keys: Vec<String>| ModulePlan {
            kind,
            object: source,
            creates: false,
            domains: Vec::new(),
            keys,
            core: Vec::new(),
            data: json!({ "unlink": target }),
        };
        match (reference.from, reference.to) {
            // The sources of the attachments of an interface are its addresses.
            (INTERFACE, INTERFACE) => Some(detach(target, source)),
            (INTERFACE, ADDRESS) => Some(detach(source, target)),
            (INTERFACE, VLAN) => Some(unlink(interface_kind(), vec![interface_key(source)])),
            (MACHINE, MACHINE | CLUSTER) => {
                Some(unlink(machine_kind(), vec![HOSTING_KEY.to_owned()]))
            }
            _ => None,
        }
    }

    /// `machines.attachments` (REQ-009-23): each attachment of an address to an interface
    /// that the changeset makes or ends, with its machine and period.
    fn diff<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a DiffContext<'a>,
    ) -> BoxFuture<'a, Result<Option<DiffSection>, StoreError>> {
        Box::pin(async move {
            let _ = context;
            let rows = pg::attachment_changes(connection(tx)?).await?;
            Ok((!rows.is_empty()).then(|| DiffSection {
                name: ATTACHMENTS_SECTION.into(),
                rows,
            }))
        })
    }

    #[allow(clippy::too_many_lines)] // one rule after the other, on the final state
    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        Box::pin(async move {
            let conn = connection(tx)?;
            let mut machines: BTreeSet<Uuid> = context
                .touched
                .iter()
                .filter(|(kind, _)| *kind == machine_kind())
                .map(|(_, id)| *id)
                .collect();
            let of = |wanted: ModuleKind| -> Vec<Uuid> {
                context
                    .touched
                    .iter()
                    .filter(|(kind, _)| *kind == wanted)
                    .map(|(_, id)| *id)
                    .collect()
            };
            let (clusters, interfaces) = (of(cluster_kind()), of(interface_kind()));
            // Addresses of the core whose usage may have changed (REQ-009-10).
            let addresses: Vec<Uuid> = context
                .objects
                .iter()
                .filter(|(kind, _)| matches!(kind, ObjectKind::Address))
                .map(|(_, id)| *id)
                .collect();
            // Networks whose `vlan.vlan` may have changed (REQ-009-13).
            let prefixes: Vec<Uuid> = context
                .objects
                .iter()
                .filter(|(kind, _)| matches!(kind, ObjectKind::Prefix))
                .map(|(_, id)| *id)
                .collect();
            // Machines whose properties the core assigned (`core.site`…).
            let assigned: Vec<Uuid> = context
                .objects
                .iter()
                .filter(|(kind, _)| *kind == ObjectKind::Module(machine_kind()))
                .map(|(_, id)| *id)
                .collect();
            // A machine moved to another owner by an assignment of the core (M3).
            machines.extend(pg::owner_changed(conn).await?);
            if machines.is_empty()
                && clusters.is_empty()
                && interfaces.is_empty()
                && addresses.is_empty()
                && prefixes.is_empty()
                && assigned.is_empty()
            {
                return Ok(Findings::default());
            }
            let mut findings = warnings(
                conn,
                &machines,
                &assigned,
                &interfaces,
                &addresses,
                &prefixes,
            )
            .await?;
            let machines: Vec<Uuid> = machines.into_iter().collect();
            let mut found = Vec::new();
            for (interface, machine) in pg::orphan_interfaces(conn, &interfaces).await? {
                found.push(
                    invalid(format!(
                        "the machine {machine} does not live over the period of the interface"
                    ))
                    .with_subjects([interface, machine]),
                );
            }
            found.extend(check_interfaces(conn, &interfaces, &addresses, context.changeset).await?);
            for (machine, other, name) in pg::name_duplicates(conn, &machines).await? {
                found.push(
                    Violation::new(
                        &NAME_DUPLICATE,
                        format!("the machine {other} of the same owner is already named {name}"),
                    )
                    .with_subjects([machine, other]),
                );
            }
            // M5 on the final state: the kind of a host machine, for the machines written and
            // the machines they host (a host whose kind changes).
            for (guest, guest_kind, host, host_kind) in pg::host_kinds(conn, &machines).await? {
                let (Some(guest_kind), Some(host_kind)) = (
                    MachineKind::from_name(&guest_kind),
                    MachineKind::from_name(&host_kind),
                ) else {
                    continue;
                };
                if let Err(v) = check_host(guest_kind, Host::Machine(host_kind)) {
                    found.push(v.with_subjects([guest, host]));
                }
            }
            let written: Vec<Uuid> = machines.iter().chain(&clusters).copied().collect();
            for (machine, host) in pg::dead_hosts(conn, &written).await? {
                found.push(
                    Violation::new(
                        &HOST_INVALID,
                        format!("the host or cluster {host} does not live over the period of the machine"),
                    )
                    .with_subjects([machine, host]),
                );
            }
            for machine in pg::hosting_cycles(conn, &machines).await? {
                found.push(
                    Violation::new(
                        &HOST_INVALID,
                        format!("the machine {machine} would host itself"),
                    )
                    .with_subjects([machine]),
                );
            }
            findings.violations.extend(found);
            Ok(findings)
        })
    }
}

/// The warnings of REQ-009-13, evaluated on the objects the changeset touches and on those
/// whose anomaly is open, so that a corrected case closes: a universal MAC on two living
/// interfaces, a network carrying a VLAN foreign to the interface holding one of its
/// addresses, a machine hosted on another site than its host.
async fn warnings(
    conn: &mut PgConnection,
    machines: &BTreeSet<Uuid>,
    assigned: &[Uuid],
    interfaces: &[Uuid],
    addresses: &[Uuid],
    prefixes: &[Uuid],
) -> Result<Findings, StoreError> {
    let mut findings = Findings::default();
    let mut warn = |rule: &'static Rule, subject: Uuid, message: String, other: Uuid| {
        findings
            .violations
            .push(Violation::new(rule, message).with_subjects([subject, other]));
    };
    // A MAC follows its interfaces: evaluated with the interfaces that share it.
    let mut evaluated: BTreeSet<Uuid> = interfaces.iter().copied().collect();
    evaluated.extend(pg::open_anomalies(conn, MAC_DUPLICATE.id).await?);
    let mut mac_evaluated = evaluated.clone();
    let candidates: Vec<Uuid> = evaluated.iter().copied().collect();
    for (interface, other, mac) in pg::mac_duplicates(conn, &candidates).await? {
        mac_evaluated.insert(other);
        for (subject, partner) in [(interface, other), (other, interface)] {
            warn(
                &MAC_DUPLICATE,
                subject,
                format!("the universal MAC {mac} is also on the interface {partner}"),
                partner,
            );
        }
    }
    // The VLAN of a network against the interfaces that hold its addresses.
    let mut vlan_evaluated: BTreeSet<Uuid> = interfaces.iter().copied().collect();
    vlan_evaluated.extend(pg::interfaces_of_addresses(conn, addresses).await?);
    vlan_evaluated.extend(pg::interfaces_in_networks(conn, prefixes).await?);
    vlan_evaluated.extend(pg::open_anomalies(conn, ADDRESS_VLAN_MISMATCH.id).await?);
    let candidates: Vec<Uuid> = vlan_evaluated.iter().copied().collect();
    for (interface, address, vlan) in pg::vlan_mismatches(conn, &candidates).await? {
        warn(
            &ADDRESS_VLAN_MISMATCH,
            interface,
            format!(
                "the network of the address {address} carries the VLAN {vlan}, which the interface does not"
            ),
            address,
        );
    }
    // A machine against its host, both ways.
    let mut site_evaluated: BTreeSet<Uuid> = machines.iter().copied().collect();
    site_evaluated.extend(assigned);
    site_evaluated.extend(pg::open_anomalies(conn, HOST_SITE_MISMATCH.id).await?);
    let hosts: Vec<Uuid> = site_evaluated.iter().copied().collect();
    site_evaluated.extend(pg::guests(conn, &hosts).await?);
    let candidates: Vec<Uuid> = site_evaluated.iter().copied().collect();
    for (guest, host) in pg::site_mismatches(conn, &candidates).await? {
        warn(
            &HOST_SITE_MISMATCH,
            guest,
            format!("the machine is hosted by {host}, on another site"),
            host,
        );
    }
    findings.reevaluated.extend(
        mac_evaluated
            .into_iter()
            .map(|o| (o, MAC_DUPLICATE.id))
            .chain(
                vlan_evaluated
                    .into_iter()
                    .map(|o| (o, ADDRESS_VLAN_MISMATCH.id)),
            )
            .chain(
                site_evaluated
                    .into_iter()
                    .map(|o| (o, HOST_SITE_MISMATCH.id)),
            ),
    );
    Ok(findings)
}

/// Clears in the object of `plan` the reference to `target` (a VLAN of an interface, a
/// host or cluster of a machine), for the cascade of a closure (`ModuleHandler::nullify`).
async fn apply_unlink(
    tx: &mut dyn ModuleTx,
    plan: &ModulePlan,
    target: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let id = plan.object;
    let target: Uuid = target
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| StoreError("a planned target".into()))?;
    let conn = connection(tx)?;
    let parent = if plan.kind == interface_kind() {
        pg::unlink_vlan(conn, id, target, period, changeset).await?;
        pg::machine_of(conn, id).await?
    } else {
        pg::unlink_host(conn, id, target, period, changeset).await?;
        None
    };
    tx.register(
        ModuleObjectRow {
            id,
            kind: plan.kind,
            parent,
            domain: None,
        },
        period,
        changeset,
    )
    .await
}

/// Attaches (`attach`) or detaches the address of `attachment` to or from the interface
/// `id` over `period`: a write of the interface, registered for its history and the undo.
async fn apply_attachment(
    tx: &mut dyn ModuleTx,
    id: Uuid,
    attach: bool,
    attachment: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let conn = connection(tx)?;
    let machine = pg::machine_of(conn, id)
        .await?
        .ok_or(WriteError::NoVersion(id))?;
    let address: Uuid = attachment["address"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| StoreError("a planned address".into()))?;
    if attach {
        let primary = attachment["primary"].as_bool().unwrap_or(false);
        pg::attach(conn, id, address, primary, period, changeset).await?;
    } else {
        let rows = pg::attachments(conn, id, address, period).await?;
        if rows.is_empty() {
            return Err(WriteError::Rule(
                invalid(format!(
                    "the address {address} is not attached to the interface over the period"
                ))
                .with_subjects([id, address]),
            ));
        }
        for row in rows {
            tx.close_versioned(pg::ATTACHMENT_TABLE, row, period, changeset)
                .await?;
        }
    }
    tx.register(
        ModuleObjectRow {
            id,
            kind: interface_kind(),
            parent: Some(machine),
            domain: None,
        },
        period,
        changeset,
    )
    .await
}

/// The rules on the final state of the interfaces `interfaces` and of the attachments of
/// the addresses `addresses` (REQ-009-9, 10, 12), after copying the usage of these
/// addresses into their attachments as `changeset`.
async fn check_interfaces(
    conn: &mut PgConnection,
    interfaces: &[Uuid],
    addresses: &[Uuid],
    changeset: Uuid,
) -> Result<Vec<Violation>, StoreError> {
    let mut found = Vec::new();
    for (address, interface, other) in pg::reshare(conn, addresses, changeset).await? {
        found.push(
            Violation::new(
                &ADDRESS_TAKEN,
                format!(
                    "the address {address} is on the interfaces {interface} and {other}: its usage stays shared"
                ),
            )
            .with_subjects([address, interface, other]),
        );
    }
    for (interface, address) in pg::dead_addresses(conn, interfaces, addresses).await? {
        found.push(
            Violation::new(
                &ADDRESS_CLOSED,
                format!("the address {address} does not live over its attachment to {interface}"),
            )
            .with_subjects([interface, address]),
        );
    }
    for (interface, address) in pg::attachments_outside(conn, interfaces).await? {
        found.push(
            invalid(format!(
                "the interface {interface} does not live over its attachment of {address}"
            ))
            .with_subjects([interface, address]),
        );
    }
    for (interface, vlan) in pg::dead_vlans(conn, interfaces).await? {
        found.push(
            Violation::new(
                &VLAN_MODE,
                format!("the VLAN {vlan} does not live over the period of the interface"),
            )
            .with_subjects([interface, vlan]),
        );
    }
    Ok(found)
}

/// The table of the objects of `kind`.
fn table(kind: ModuleKind) -> &'static str {
    if kind == cluster_kind() {
        pg::CLUSTER_TABLE
    } else if kind == interface_kind() {
        pg::INTERFACE_TABLE
    } else {
        pg::MACHINE_TABLE
    }
}
