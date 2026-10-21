//! The handler of the module in the write pipeline (feature `pg`; ADR 0026, clause 6): the
//! operations on the groups of VLAN (REQ-008-12, 13). A group is anchored on an entity, a
//! site or a domain (V3): the handler adds to the changeset the assignments of
//! `core.owner` and, for a site, of `core.site`, ordinary operations of the core checked
//! under their own rights.

use std::sync::OnceLock;

use kartenn_core::objects::Slug;
use kartenn_core::period::Period;
use kartenn_core::rules::Violation;
use kartenn_service::ObjectKind;
use kartenn_service::module::{BoxFuture, ModuleHandler, ModuleObjectRow, ModulePlan, ModuleTx};
use kartenn_service::operations::{ModuleKind, ModuleOperation, Operation, RawInput, Ref};
use kartenn_service::port::{StoreError, WriteError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::group::VidRange;
use crate::module::GROUP;
use crate::pg;
use crate::rules::{GROUP_ANCHOR, VID_OUT_OF_RANGE};
use crate::vlan::Vid;

/// The handler of the module.
#[derive(Debug, Clone, Copy, Default)]
pub struct Handler;

fn group_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    *KIND.get_or_init(|| ModuleKind::parse(GROUP).expect("a valid kind"))
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
}

/// The anchor of a group (V3), as received.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum AnchorInput {
    Entity(Ref),
    Site(Ref),
    Domain(Ref),
}

impl AnchorInput {
    const fn kind(self) -> &'static str {
        match self {
            Self::Entity(_) => "entity",
            Self::Site(_) => "site",
            Self::Domain(_) => "domain",
        }
    }

    const fn object(self) -> Ref {
        match self {
            Self::Entity(r) | Self::Site(r) | Self::Domain(r) => r,
        }
    }
}

/// What [`Handler::prepare`] reads for the plan: the properties of the core it assigns,
/// and the anchor a modification replaces.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Completed {
    #[serde(default)]
    owner_property: Option<Uuid>,
    #[serde(default)]
    site_property: Option<Uuid>,
    #[serde(default)]
    anchor_kind: Option<String>,
}

/// `vlan.create_group`.
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
    /// Filled by the module before the plan.
    #[serde(default)]
    completed: Completed,
}

/// `vlan.update_group`: a Merge Patch of the group.
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
    /// Filled by the module before the plan.
    #[serde(default)]
    completed: Completed,
}

fn decode<T: serde::de::DeserializeOwned>(operation: &ModuleOperation) -> Result<T, Violation> {
    serde_json::from_value(Value::Object(operation.body.clone()))
        .map_err(|e| Violation::new(&GROUP_ANCHOR, format!("{}: {e}", operation.tag())))
}

/// The assignment of the reference `value` to the property `property` of the group.
fn assign(group: Uuid, property: Uuid, value: Uuid) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(group),
        kind: ObjectKind::Module(group_kind()),
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
    let undeclared = |key: &str| Violation::new(&GROUP_ANCHOR, format!("{key} is not declared"));
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
                kind: ObjectKind::Module(group_kind()),
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
            &GROUP_ANCHOR,
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

impl Handler {
    fn plan_create(
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let create: CreateGroup = decode(operation)?;
        let _ = create.local;
        let slug = Slug::parse(&create.slug)?;
        let range = VidRange::new(bound(create.vid_min, 1)?, bound(create.vid_max, 4094)?)?;
        let anchor = resolve(create.anchor.object())
            .ok_or_else(|| Violation::new(&GROUP_ANCHOR, "unknown anchor"))?;
        let kind = create.anchor.kind();
        let given = match create.owner {
            Some(r) => {
                Some(resolve(r).ok_or_else(|| Violation::new(&GROUP_ANCHOR, "unknown owner"))?)
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

    fn plan_update(
        operation: &ModuleOperation,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let update: UpdateGroup = decode(operation)?;
        let group =
            resolve(update.id).ok_or_else(|| Violation::new(&GROUP_ANCHOR, "unknown group"))?;
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
                    .ok_or_else(|| Violation::new(&GROUP_ANCHOR, "unknown anchor"))?;
                patch.insert("anchor_kind".into(), input.kind().into());
                patch.insert("anchor_id".into(), object.to_string().into());
                Some((input.kind(), object))
            }
            None => None,
        };
        let given = match update.owner {
            Some(r) => {
                Some(resolve(r).ok_or_else(|| Violation::new(&GROUP_ANCHOR, "unknown owner"))?)
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
                anchor_kind: None,
            };
            let body = &mut operation.body;
            if operation.name == "update_group"
                && let Some(Ok(Ref::Id(group))) =
                    body.get("id").cloned().map(serde_json::from_value::<Ref>)
            {
                completed.anchor_kind = pg::anchor(conn, group, at).await?.map(|(kind, _)| kind);
            }
            // A group anchored on a domain is owned, unless told otherwise, by the resolved
            // owner of the domain (V3).
            let domain = body
                .get("anchor")
                .and_then(|anchor| anchor.get("domain"))
                .cloned()
                .and_then(|r| serde_json::from_value::<Ref>(r).ok());
            if !body.contains_key("owner")
                && let Some(Ref::Id(domain)) = domain
                && let Some(owner) = pg::owner(conn, domain, at).await?
            {
                body.insert("owner".into(), json!({ "id": owner }));
            }
            body.insert(
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
            "create_group" => Self::plan_create(operation, object, resolve),
            "update_group" => Self::plan_update(operation, resolve),
            other => Err(Violation::new(
                &GROUP_ANCHOR,
                format!("vlan.{other}: unknown operation"),
            )),
        }
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
            if let Some(create) = plan.data.get("create") {
                let anchor_id: Uuid = create["anchor_id"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| StoreError("a planned anchor".into()))?;
                let kind = create["anchor_kind"].as_str().unwrap_or_default();
                pg::insert_group(
                    connection(tx)?,
                    id,
                    create["slug"].as_str().unwrap_or_default(),
                    (kind, anchor_id),
                    (
                        create["vid_min"].as_i64().unwrap_or(1),
                        create["vid_max"].as_i64().unwrap_or(4094),
                    ),
                    period,
                    changeset,
                )
                .await?;
            } else {
                pg::patch_group(connection(tx)?, id, &plan.data["update"], period, changeset)
                    .await?;
            }
            // The record of the core, with the domain of the anchor (ADR 0026, clause 4).
            let (at, _) = pg::bounds(period);
            let anchor = pg::anchor(connection(tx)?, id, at).await?;
            let domain = anchor.and_then(|(kind, id)| (kind == "domain").then_some(id));
            tx.register(
                ModuleObjectRow {
                    id,
                    kind: plan.kind,
                    parent: None,
                    domain,
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
        _kind: ModuleKind,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        tx.close_versioned(pg::GROUP_TABLE, id, period, changeset)
    }

    fn restore<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        _kind: ModuleKind,
        id: Uuid,
        before: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        tx.restore_versioned(pg::GROUP_TABLE, id, before, period, changeset)
    }

    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        touched: &'a [(ModuleKind, Uuid)],
    ) -> BoxFuture<'a, Result<Vec<Violation>, StoreError>> {
        Box::pin(async move {
            let groups: Vec<Uuid> = touched
                .iter()
                .filter(|(kind, _)| *kind == group_kind())
                .map(|(_, id)| *id)
                .collect();
            if groups.is_empty() {
                return Ok(Vec::new());
            }
            let conn = connection(tx)?;
            let mut found = Vec::new();
            // REQ-008-13: a range keeps the VLANs of its group, listed.
            let excluded = pg::excluded_vlans(conn, &groups).await?;
            for group in &groups {
                let vlans: Vec<String> = excluded
                    .iter()
                    .filter(|(g, _, _)| g == group)
                    .map(|(_, vid, name)| format!("{vid} ({name})"))
                    .collect();
                if !vlans.is_empty() {
                    found.push(
                        Violation::new(
                            &VID_OUT_OF_RANGE,
                            format!("the range of the group excludes VLAN {}", vlans.join(", ")),
                        )
                        .with_subjects([*group]),
                    );
                }
            }
            for (group, kind, anchor) in pg::dead_anchors(conn, &groups).await? {
                found.push(
                    Violation::new(
                        &GROUP_ANCHOR,
                        format!("the {kind} {anchor} does not live over the period of the group"),
                    )
                    .with_subjects([group]),
                );
            }
            Ok(found)
        })
    }
}
