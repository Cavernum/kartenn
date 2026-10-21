//! A module of test for the anchors of module objects (spec 008, task 008-5): groups,
//! anchored by `core.owner` and `core.site` and optionally in a domain, and items below a
//! group, which inherit their anchors along the tree of the module. No table of its own:
//! the objects live in `module_object`.

use std::sync::Arc;

use kartenn_core::authz::Permission;
use kartenn_core::period::Period;
use kartenn_core::rules::{Rule, Severity, Violation};
use kartenn_service::module::{
    BoxFuture, Module, ModuleHandler, ModuleObjectRow, ModulePlan, ModuleRegistry, ModuleTx,
    ObjectType, OperationType,
};
use kartenn_service::operations::{ModuleKind, ModuleOperation, Operation, Ref};
use kartenn_service::port::WriteError;
use kartenn_store_pg::write::PgStore;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

/// An operation of the module is well formed.
pub(crate) static INVALID: Rule = Rule {
    id: "kit.object.invalid",
    severity: Severity::Error,
    summary: "An object of the kit names existing parents and domains.",
    requirements: &["REQ-008-6"],
    source: "008/design",
};

static TYPES: &[ObjectType] = &[
    ObjectType {
        kind: "kit.group",
        collection: "kit-groups",
        parent: None,
        owner_required: true,
        carries_site: true,
        domain_anchored: true,
    },
    ObjectType {
        kind: "kit.item",
        collection: "kit-items",
        parent: Some("kit.group"),
        owner_required: false,
        carries_site: false,
        domain_anchored: false,
    },
];

static OPERATIONS: &[OperationType] = &[
    OperationType {
        name: "create_group",
        defines: Some("kit.group"),
    },
    OperationType {
        name: "create_item",
        defines: Some("kit.item"),
    },
];

static GROUP_READ: Permission = Permission {
    name: "kit.group.read",
    summary: "Lire les groupes du kit.",
    global_only: false,
};
static GROUP_WRITE: Permission = Permission {
    name: "kit.group.write",
    summary: "Créer ou modifier un groupe du kit.",
    global_only: false,
};
static ITEM_READ: Permission = Permission {
    name: "kit.item.read",
    summary: "Lire les éléments du kit.",
    global_only: false,
};
static PERMISSIONS: &[&Permission] = &[&GROUP_READ, &GROUP_WRITE, &ITEM_READ];
static RULES: &[&Rule] = &[&INVALID];

/// The module.
#[derive(Debug)]
pub(crate) struct Kit;

impl Module for Kit {
    fn name(&self) -> &'static str {
        "kit"
    }
    fn version(&self) -> &'static str {
        "0.0.1"
    }
    fn object_types(&self) -> &'static [ObjectType] {
        TYPES
    }
    fn permissions(&self) -> &'static [&'static Permission] {
        PERMISSIONS
    }
    fn rules(&self) -> &'static [&'static Rule] {
        RULES
    }
    fn operations(&self) -> &'static [OperationType] {
        OPERATIONS
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        Some(Arc::new(Handler))
    }
}

/// `kit.create_group` and `kit.create_item`, as received.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    #[serde(default)]
    local: Option<u32>,
    #[serde(default)]
    parent: Option<Ref>,
    #[serde(default)]
    domain: Option<Ref>,
}

#[derive(Debug)]
struct Handler;

impl ModuleHandler for Handler {
    fn plan(
        &self,
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let invalid = |message: String| Violation::new(&INVALID, message);
        let create: Create =
            serde_json::from_value(serde_json::Value::Object(operation.body.clone()))
                .map_err(|e| invalid(e.to_string()))?;
        let _ = create.local;
        let kind = match operation.name.as_str() {
            "create_group" => "kit.group",
            _ => "kit.item",
        };
        let lookup = |r: Option<Ref>, what: &str| match r {
            Some(r) => resolve(r)
                .map(Some)
                .ok_or_else(|| invalid(format!("unknown {what}"))),
            None => Ok(None),
        };
        let parent = lookup(create.parent, "parent")?;
        let domain = lookup(create.domain, "domain")?;
        Ok(ModulePlan {
            kind: ModuleKind::parse(kind).expect("a valid kind"),
            object,
            creates: true,
            domains: domain.into_iter().collect(),
            keys: Vec::new(),
            core: Vec::new(),
            data: serde_json::json!({"parent": parent, "domain": domain}),
        })
    }

    fn apply<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        plan: &'a ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        let id = |field: &str| plan.data[field].as_str().and_then(|s| s.parse().ok());
        let row = ModuleObjectRow {
            id: plan.object,
            kind: plan.kind,
            parent: id("parent"),
            domain: id("domain"),
        };
        Box::pin(async move { tx.register(row, period, changeset).await })
    }
}

/// The registry of a build with the module.
pub(crate) fn registry() -> Arc<ModuleRegistry> {
    Arc::new(ModuleRegistry::new(vec![Arc::new(Kit)]).expect("a valid registry"))
}

/// A store whose changesets accept the operations of the module.
pub(crate) fn store(pool: &PgPool) -> PgStore {
    PgStore::new(pool.clone()).with_modules(registry())
}

/// `kit.<name>` from its JSON fields.
#[allow(dead_code)] // the tests of the catalogues create no object
pub(crate) fn op(name: &str, fields: serde_json::Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("kit.{name}").into();
    serde_json::from_value(object).expect("an operation")
}
