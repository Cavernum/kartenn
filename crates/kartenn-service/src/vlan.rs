//! The VLANs of the core (spec 026, REQ-026-1; spec 008, REQ-008-12 to 18): groups of
//! VLAN and VLANs, objects of the core outside the registry of the modules. Their
//! operations are planned here, without the database; the storage adapter completes them
//! beforehand and writes them (`kartenn-store-pg`), through the same [`ModuleHandler`]
//! contract as the objects of a module, so that one pipeline serves both (decision C2).
//!
//! A group is anchored on an entity, a site or a domain (V3): the plan adds to the
//! changeset the assignments of `core.owner` and, for a site, of `core.site`, ordinary
//! operations of the core checked under their own rights. Every write on a VLAN takes the
//! key of its groups (REQ-008-27) before the uniqueness of its VID and name is checked.
//!
//! [`ModuleHandler`]: crate::module::ModuleHandler

use std::sync::OnceLock;

use kartenn_core::objects::Slug;
use kartenn_core::rules::{VLAN_GROUP_ANCHOR, VLAN_VID_OUT_OF_RANGE, Violation};
use kartenn_core::vlan::{Vid, VidRange, VlanName, VlanStatus};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::module::{ModulePlan, ObjectType, OnClose, OperationType, Reference};
use crate::operations::{ModuleKind, ModuleOperation, ObjectKind, Operation, RawInput, Ref};

/// The kind of a group of VLAN.
pub const GROUP: &str = "vlan_group";
/// The kind of a VLAN.
pub const VLAN: &str = "vlan";
/// The property of the core that gives a network its VLAN (V4, ex-`vlan.vlan`).
pub const PROPERTY: &str = "core.vlan";

/// The types of the VLANs as the transversal routes and the rules serve them: a group
/// carries `core.owner` (required) and, anchored on a site, `core.site`; anchored on a
/// domain, it is in the scope and under the lock of the domain (V3). A VLAN inherits the
/// anchors of its group.
pub static TYPES: &[ObjectType] = &[
    ObjectType {
        kind: GROUP,
        collection: "vlan-groups",
        parent: None,
        owner_required: true,
        carries_site: true,
        domain_anchored: true,
    },
    ObjectType {
        kind: VLAN,
        collection: "vlans",
        parent: Some(GROUP),
        owner_required: false,
        carries_site: false,
        domain_anchored: true,
    },
];

/// The type of the groups.
#[must_use]
pub fn group_type() -> &'static ObjectType {
    &TYPES[0]
}

/// The type of the VLANs.
#[must_use]
pub fn vlan_type() -> &'static ObjectType {
    &TYPES[1]
}

/// The operations of the VLANs, tagged by their name as the other operations of the core.
pub static OPERATIONS: &[OperationType] = &[
    OperationType {
        name: "create_vlan_group",
        defines: Some(GROUP),
    },
    OperationType {
        name: "update_vlan_group",
        defines: None,
    },
    OperationType {
        name: "create_vlan",
        defines: Some(VLAN),
    },
    OperationType {
        name: "update_vlan",
        defines: None,
    },
];

/// Whether `name` is an operation of the VLANs.
#[must_use]
pub fn is_operation(name: &str) -> bool {
    OPERATIONS.iter().any(|o| o.name == name)
}

/// The operation of the VLANs named `name`.
#[must_use]
pub fn operation(name: &str) -> Option<&'static OperationType> {
    OPERATIONS.iter().find(|o| o.name == name)
}

/// The references of the VLANs (REQ-008-10; design 008 « Références »): a VLAN to its
/// group, closed with it under the cascade; a group to its anchor, refused until it moves.
/// The property `core.vlan` of a network is a reference of the core (REQ-003-10): the
/// closure of its VLAN is refused, and the cascade removes the assignment.
pub static REFERENCES: &[Reference] = &[
    Reference {
        from: VLAN,
        to: GROUP,
        on_close: OnClose::Cascade,
    },
    Reference {
        from: GROUP,
        to: "entity",
        on_close: OnClose::Refuse,
    },
    Reference {
        from: GROUP,
        to: "site",
        on_close: OnClose::Refuse,
    },
    Reference {
        from: GROUP,
        to: "domain",
        on_close: OnClose::Refuse,
    },
];

/// The qualified type of the groups in a plan, `core.vlan_group`.
///
/// # Panics
///
/// Never: the name is a valid literal.
#[must_use]
pub fn group_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    *KIND.get_or_init(|| ModuleKind::parse("core.vlan_group").expect("a valid kind"))
}

/// The qualified type of the VLANs in a plan, `core.vlan`.
///
/// # Panics
///
/// Never: the name is a valid literal.
#[must_use]
pub fn vlan_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    *KIND.get_or_init(|| ModuleKind::parse("core.vlan").expect("a valid kind"))
}

/// The key that serialises the writes on the VLANs of `group` (REQ-008-27): the lock of a
/// domain does not cover a group anchored on an entity or a site.
#[must_use]
pub fn group_key(group: Uuid) -> String {
    format!("core.vlan_group:{group}")
}

/// The anchor of a group (V3), as received.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AnchorInput {
    /// An entity.
    Entity(Ref),
    /// A site.
    Site(Ref),
    /// An addressing domain.
    Domain(Ref),
}

impl AnchorInput {
    /// `entity`, `site` or `domain`.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Entity(_) => "entity",
            Self::Site(_) => "site",
            Self::Domain(_) => "domain",
        }
    }

    /// The object of the anchor.
    #[must_use]
    pub const fn object(self) -> Ref {
        match self {
            Self::Entity(r) | Self::Site(r) | Self::Domain(r) => r,
        }
    }
}

/// What the storage reads for the plan of an operation on a group: the properties of the
/// core it assigns, and the anchor a modification replaces.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completed {
    /// `core.owner`, declared at the start of the period.
    #[serde(default)]
    pub owner_property: Option<Uuid>,
    /// `core.site`, declared at the start of the period.
    #[serde(default)]
    pub site_property: Option<Uuid>,
    /// The kind of the anchor the group has before the modification.
    #[serde(default)]
    pub anchor_kind: Option<String>,
}

/// What the storage reads for the plan of an operation on a VLAN: the domains of the
/// groups it writes in, to lock (ADR 0015), and, for a modification, the group the VLAN is
/// in, whose key it takes as well.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VlanCompleted {
    /// The domains of the groups, sorted.
    #[serde(default)]
    pub domains: Vec<Uuid>,
    /// The group of the VLAN before the modification.
    #[serde(default)]
    pub current_group: Option<Uuid>,
}

/// `create_vlan_group`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateGroup {
    #[serde(default)]
    local: Option<u32>,
    slug: String,
    anchor: AnchorInput,
    #[serde(default)]
    vid_min: Option<i64>,
    #[serde(default)]
    vid_max: Option<i64>,
    #[serde(default)]
    owner: Option<Ref>,
    /// Filled by the storage before the plan.
    #[serde(default)]
    completed: Completed,
}

/// `update_vlan_group`: a Merge Patch of the group.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateGroup {
    id: Ref,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    anchor: Option<AnchorInput>,
    #[serde(default)]
    vid_min: Option<i64>,
    #[serde(default)]
    vid_max: Option<i64>,
    #[serde(default)]
    owner: Option<Ref>,
    /// Filled by the storage before the plan.
    #[serde(default)]
    completed: Completed,
}

/// `create_vlan`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateVlan {
    #[serde(default)]
    local: Option<u32>,
    group: Ref,
    vid: i64,
    name: String,
    #[serde(default)]
    status: Option<Status>,
    /// Filled by the storage before the plan.
    #[serde(default)]
    completed: VlanCompleted,
}

/// `update_vlan`: a Merge Patch of the VLAN (REQ-008-16).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateVlan {
    id: Ref,
    #[serde(default)]
    group: Option<Ref>,
    #[serde(default)]
    vid: Option<i64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    status: Option<Status>,
    /// Filled by the storage before the plan.
    #[serde(default)]
    completed: VlanCompleted,
}

/// The status of a VLAN as received (V5).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Active,
    Reserved,
    Deprecated,
}

impl From<Status> for VlanStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Active => Self::Active,
            Status::Reserved => Self::Reserved,
            Status::Deprecated => Self::Deprecated,
        }
    }
}

/// The identifier of the reference `key` of `body`, when given by identifier.
#[must_use]
pub fn id_in(body: &serde_json::Map<String, Value>, key: &str) -> Option<Uuid> {
    match serde_json::from_value(body.get(key)?.clone()).ok()? {
        Ref::Id(id) => Some(id),
        Ref::Local(_) => None,
    }
}

fn decode<T: serde::de::DeserializeOwned>(operation: &ModuleOperation) -> Result<T, Violation> {
    serde_json::from_value(Value::Object(operation.body.clone()))
        .map_err(|e| Violation::new(&VLAN_GROUP_ANCHOR, format!("{}: {e}", operation.tag())))
}

/// The assignment of the reference `value` to the property `property` of the group.
fn assign(group: Uuid, property: Uuid, value: Uuid) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(group),
        kind: ObjectKind::VlanGroup,
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.to_string())],
    }
}

/// The assignments of the anchors of `group` (V3): `core.owner` when known, `core.site`
/// for a site, its removal when the group leaves a site.
fn anchors(
    group: Uuid,
    anchor: Option<(&str, Uuid)>,
    owner: Option<Uuid>,
    completed: &Completed,
) -> Result<Vec<Operation>, Violation> {
    let undeclared =
        |key: &str| Violation::new(&VLAN_GROUP_ANCHOR, format!("{key} is not declared"));
    let mut core = Vec::new();
    if let Some(owner) = owner {
        let property = completed
            .owner_property
            .ok_or_else(|| undeclared("core.owner"))?;
        core.push(assign(group, property, owner));
    }
    match anchor {
        Some(("site", site)) => {
            let property = completed
                .site_property
                .ok_or_else(|| undeclared("core.site"))?;
            core.push(assign(group, property, site));
        }
        Some(_) if completed.anchor_kind.as_deref() == Some("site") => {
            let property = completed
                .site_property
                .ok_or_else(|| undeclared("core.site"))?;
            core.push(Operation::UnassignProperty {
                object: Ref::Id(group),
                kind: ObjectKind::VlanGroup,
                property: Ref::Id(property),
            });
        }
        _ => {}
    }
    Ok(core)
}

/// The owner of a group anchored on `anchor`: an entity owns the groups anchored on it;
/// otherwise the owner given (V3).
fn owner_of(anchor: Option<(&str, Uuid)>, given: Option<Uuid>) -> Result<Option<Uuid>, Violation> {
    match (anchor, given) {
        (Some(("entity", entity)), Some(owner)) if owner != entity => Err(Violation::new(
            &VLAN_GROUP_ANCHOR,
            "a group anchored on an entity is owned by that entity",
        )),
        (Some(("entity", entity)), _) => Ok(Some(entity)),
        (_, given) => Ok(given),
    }
}

fn bound(value: Option<i64>, default: i64) -> Result<i64, Violation> {
    let value = value.unwrap_or(default);
    Vid::new(value).map(|_| value)
}

fn plan_create_group(
    operation: &ModuleOperation,
    object: Uuid,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    let create: CreateGroup = decode(operation)?;
    let _ = create.local;
    let slug = Slug::parse(&create.slug)?;
    let range = VidRange::new(bound(create.vid_min, 1)?, bound(create.vid_max, 4094)?)?;
    let anchor = resolve(create.anchor.object())
        .ok_or_else(|| Violation::new(&VLAN_GROUP_ANCHOR, "unknown anchor"))?;
    let kind = create.anchor.kind();
    let given = match create.owner {
        Some(r) => {
            Some(resolve(r).ok_or_else(|| Violation::new(&VLAN_GROUP_ANCHOR, "unknown owner"))?)
        }
        None => None,
    };
    let owner = owner_of(Some((kind, anchor)), given)?;
    let domain = (kind == "domain").then_some(anchor);
    Ok(ModulePlan {
        kind: group_kind(),
        object,
        creates: true,
        domains: domain.into_iter().collect(),
        keys: Vec::new(),
        core: anchors(object, Some((kind, anchor)), owner, &create.completed)?,
        data: json!({
            "create": {
                "slug": slug.as_str(),
                "anchor_kind": kind,
                "anchor_id": anchor,
                "vid_min": range.min().get(),
                "vid_max": range.max().get(),
            }
        }),
    })
}

fn plan_update_group(
    operation: &ModuleOperation,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    let update: UpdateGroup = decode(operation)?;
    let group =
        resolve(update.id).ok_or_else(|| Violation::new(&VLAN_GROUP_ANCHOR, "unknown group"))?;
    let mut patch = serde_json::Map::new();
    if let Some(slug) = &update.slug {
        patch.insert("slug".into(), Slug::parse(slug)?.as_str().into());
    }
    if let (Some(min), Some(max)) = (update.vid_min, update.vid_max) {
        VidRange::new(min, max)?;
    }
    for (name, value) in [("vid_min", update.vid_min), ("vid_max", update.vid_max)] {
        if let Some(value) = value {
            patch.insert(name.into(), Vid::new(value)?.get().into());
        }
    }
    let anchor = match update.anchor {
        Some(input) => {
            let object = resolve(input.object())
                .ok_or_else(|| Violation::new(&VLAN_GROUP_ANCHOR, "unknown anchor"))?;
            patch.insert("anchor_kind".into(), input.kind().into());
            patch.insert("anchor_id".into(), object.to_string().into());
            Some((input.kind(), object))
        }
        None => None,
    };
    let given = match update.owner {
        Some(r) => {
            Some(resolve(r).ok_or_else(|| Violation::new(&VLAN_GROUP_ANCHOR, "unknown owner"))?)
        }
        None => None,
    };
    let owner = owner_of(anchor, given)?;
    let domain = anchor.and_then(|(kind, id)| (kind == "domain").then_some(id));
    Ok(ModulePlan {
        kind: group_kind(),
        object: group,
        creates: false,
        domains: domain.into_iter().collect(),
        keys: Vec::new(),
        core: anchors(group, anchor, owner, &update.completed)?,
        data: json!({ "update": patch }),
    })
}

fn plan_create_vlan(
    operation: &ModuleOperation,
    object: Uuid,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    let create: CreateVlan = decode(operation)?;
    let _ = create.local;
    let vid = Vid::new(create.vid)?;
    let name = VlanName::new(&create.name)?;
    let status = create.status.map_or(VlanStatus::Active, VlanStatus::from);
    let group = resolve(create.group)
        .ok_or_else(|| Violation::new(&VLAN_VID_OUT_OF_RANGE, "unknown group of VLAN"))?;
    Ok(ModulePlan {
        kind: vlan_kind(),
        object,
        creates: true,
        domains: create.completed.domains,
        keys: vec![group_key(group)],
        core: Vec::new(),
        data: json!({
            "create": {
                "group_id": group,
                "vid": vid.get(),
                "name": name.as_str(),
                "status": status.as_str(),
            }
        }),
    })
}

fn plan_update_vlan(
    operation: &ModuleOperation,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    let update: UpdateVlan = decode(operation)?;
    let vlan =
        resolve(update.id).ok_or_else(|| Violation::new(&VLAN_VID_OUT_OF_RANGE, "unknown VLAN"))?;
    let mut patch = serde_json::Map::new();
    // The group it leaves and the group it joins are both serialised.
    let mut keys: Vec<String> = update
        .completed
        .current_group
        .map(group_key)
        .into_iter()
        .collect();
    if let Some(group) = update.group {
        let group = resolve(group)
            .ok_or_else(|| Violation::new(&VLAN_VID_OUT_OF_RANGE, "unknown group of VLAN"))?;
        patch.insert("group_id".into(), group.to_string().into());
        keys.push(group_key(group));
    }
    if let Some(vid) = update.vid {
        patch.insert("vid".into(), Vid::new(vid)?.get().into());
    }
    if let Some(name) = &update.name {
        patch.insert("name".into(), VlanName::new(name)?.as_str().into());
    }
    if let Some(status) = update.status {
        patch.insert("status".into(), VlanStatus::from(status).as_str().into());
    }
    Ok(ModulePlan {
        kind: vlan_kind(),
        object: vlan,
        creates: false,
        domains: update.completed.domains,
        keys,
        core: Vec::new(),
        data: json!({ "update": patch }),
    })
}

/// Plans an operation of the VLANs (`create_vlan_group`, `update_vlan_group`,
/// `create_vlan`, `update_vlan`), completed by the storage beforehand.
///
/// # Errors
///
/// The violation of a malformed operation, of a VID, a name or a range out of their
/// bounds, or of an unknown anchor, owner, group or VLAN.
pub fn plan(
    operation: &ModuleOperation,
    object: Uuid,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    match operation.name.as_str() {
        "create_vlan_group" => plan_create_group(operation, object, resolve),
        "update_vlan_group" => plan_update_group(operation, resolve),
        "create_vlan" => plan_create_vlan(operation, object, resolve),
        "update_vlan" => plan_update_vlan(operation, resolve),
        other => Err(Violation::new(
            &VLAN_GROUP_ANCHOR,
            format!("{other}: unknown operation"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(name: &str, body: Value) -> ModuleOperation {
        ModuleOperation {
            module: crate::module::CORE.into(),
            name: name.into(),
            body: match body {
                Value::Object(map) => map,
                _ => serde_json::Map::new(),
            },
        }
    }

    #[test]
    fn a_group_anchored_on_an_entity_is_owned_by_it() {
        let entity = Uuid::now_v7();
        let owner = Uuid::now_v7();
        let completed = json!({ "owner_property": owner });
        let plan = plan(
            &op(
                "create_vlan_group",
                json!({ "slug": "g", "anchor": { "entity": { "id": entity } }, "completed": completed }),
            ),
            Uuid::now_v7(),
            &|r| match r {
                Ref::Id(id) => Some(id),
                Ref::Local(_) => None,
            },
        )
        .unwrap();
        assert_eq!(plan.kind.as_str(), "core.vlan_group");
        assert_eq!(plan.core.len(), 1);
        assert!(plan.domains.is_empty());
        let other = plan_create_group(
            &op(
                "create_vlan_group",
                json!({ "slug": "g", "anchor": { "entity": { "id": entity } },
                        "owner": { "id": Uuid::now_v7() }, "completed": completed }),
            ),
            Uuid::now_v7(),
            &|r| match r {
                Ref::Id(id) => Some(id),
                Ref::Local(_) => None,
            },
        );
        assert_eq!(other.unwrap_err().rule.id, "core.vlan.group_anchor");
    }

    #[test]
    fn a_vlan_takes_the_key_of_its_group() {
        let group = Uuid::now_v7();
        let plan = plan(
            &op(
                "create_vlan",
                json!({ "group": { "id": group }, "vid": 10, "name": "web" }),
            ),
            Uuid::now_v7(),
            &|r| match r {
                Ref::Id(id) => Some(id),
                Ref::Local(_) => None,
            },
        )
        .unwrap();
        assert_eq!(plan.keys, vec![group_key(group)]);
        assert_eq!(plan.kind, vlan_kind());
        let refused = plan_create_vlan(
            &op(
                "create_vlan",
                json!({ "group": { "id": group }, "vid": 4095, "name": "web" }),
            ),
            Uuid::now_v7(),
            &|_| None,
        );
        assert_eq!(refused.unwrap_err().rule.id, "core.vlan.vid_invalid");
    }
}
