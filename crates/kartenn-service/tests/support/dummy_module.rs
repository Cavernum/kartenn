//! A module of test for the module contract (spec 008, task 008-3): one table, one
//! operation, `dummy.create_thing`, that may reference a prefix, add a core operation and
//! name lock keys; a thing labelled `forbidden` is refused by the module's own rule on the
//! final state.

use std::sync::Arc;

use kartenn_core::period::Period;
use kartenn_core::rules::{Rule, Severity, Violation};
use kartenn_service::module::{
    BoxFuture, Module, ModuleHandler, ModuleMigration, ModuleObjectRow, ModulePlan, ModuleRegistry,
    ModuleTx, ObjectType, OperationType,
};
use kartenn_service::operations::{ModuleKind, ModuleOperation, Operation, Ref};
use kartenn_service::port::{StoreError, WriteError};
use kartenn_store_pg::write::PgStore;
use serde::Deserialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// A thing has a label, and its references exist.
pub(crate) static INVALID: Rule = Rule {
    id: "dummy.thing.invalid",
    severity: Severity::Error,
    summary: "A thing has a label and designates existing objects.",
    requirements: &["REQ-008-4"],
    source: "008/design",
};

/// No thing is labelled `forbidden` (a rule on the final state).
pub(crate) static FORBIDDEN: Rule = Rule {
    id: "dummy.thing.forbidden",
    severity: Severity::Error,
    summary: "No thing is labelled forbidden.",
    requirements: &["REQ-008-4"],
    source: "008/design",
};

static TYPES: &[ObjectType] = &[ObjectType {
    kind: "dummy.thing",
    collection: "things",
    parent: None,
    owner_required: false,
    carries_site: false,
    domain_anchored: false,
}];

static OPERATIONS: &[OperationType] = &[OperationType {
    name: "create_thing",
    defines: Some("dummy.thing"),
}];

static MIGRATIONS: &[ModuleMigration] = &[ModuleMigration {
    version: 1,
    name: "thing",
    sql: "CREATE TABLE thing (id uuid PRIMARY KEY, label text NOT NULL UNIQUE, \
          prefix_id uuid, changeset_id uuid NOT NULL); \
          GRANT SELECT, INSERT, UPDATE, DELETE ON thing TO kartenn_app;",
}];

/// The module.
#[derive(Debug)]
pub(crate) struct Dummy;

impl Module for Dummy {
    fn name(&self) -> &'static str {
        "dummy"
    }
    fn version(&self) -> &'static str {
        "0.0.1"
    }
    fn object_types(&self) -> &'static [ObjectType] {
        TYPES
    }
    fn migrations(&self) -> &'static [ModuleMigration] {
        MIGRATIONS
    }
    fn operations(&self) -> &'static [OperationType] {
        OPERATIONS
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        Some(Arc::new(Handler))
    }
}

/// `dummy.create_thing`, as received.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateThing {
    #[serde(default)]
    local: Option<u32>,
    label: String,
    #[serde(default)]
    prefix: Option<Ref>,
    /// A domain the thing creates along, by a core operation.
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    keys: Vec<String>,
}

#[derive(Debug)]
struct Handler;

fn kind() -> ModuleKind {
    ModuleKind::parse("dummy.thing").expect("a valid kind")
}

impl ModuleHandler for Handler {
    fn plan(
        &self,
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let invalid = |message: String| Violation::new(&INVALID, message);
        let thing: CreateThing =
            serde_json::from_value(serde_json::Value::Object(operation.body.clone()))
                .map_err(|e| invalid(e.to_string()))?;
        let _ = thing.local;
        if thing.label.is_empty() {
            return Err(invalid("a thing needs a label".into()));
        }
        let prefix = match thing.prefix {
            Some(r) => Some(resolve(r).ok_or_else(|| invalid("unknown prefix".into()))?),
            None => None,
        };
        let core = thing
            .domain
            .into_iter()
            .map(|slug| Operation::CreateDomain { local: None, slug })
            .collect();
        Ok(ModulePlan {
            kind: kind(),
            object,
            creates: true,
            domains: Vec::new(),
            keys: thing.keys,
            core,
            data: serde_json::json!({"label": thing.label, "prefix": prefix}),
        })
    }

    fn apply<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        plan: &'a ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(async move {
            let connection: &mut PgConnection = tx
                .connection()
                .downcast_mut()
                .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))?;
            let prefix: Option<Uuid> = plan.data["prefix"].as_str().and_then(|p| p.parse().ok());
            sqlx::query(
                "INSERT INTO dummy.thing (id, label, prefix_id, changeset_id) VALUES ($1, $2, $3, $4)",
            )
            .bind(plan.object)
            .bind(plan.data["label"].as_str())
            .bind(prefix)
            .bind(changeset)
            .execute(connection)
            .await
            .map_err(|e| StoreError(e.to_string()))?;
            tx.register(
                ModuleObjectRow {
                    id: plan.object,
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

    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        touched: &'a [(ModuleKind, Uuid)],
    ) -> BoxFuture<'a, Result<Vec<Violation>, StoreError>> {
        Box::pin(async move {
            let connection: &mut PgConnection = tx
                .connection()
                .downcast_mut()
                .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))?;
            let ids: Vec<Uuid> = touched.iter().map(|(_, id)| *id).collect();
            let forbidden: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM dummy.thing WHERE id = ANY($1) AND label = 'forbidden'",
            )
            .bind(&ids)
            .fetch_all(connection)
            .await
            .map_err(|e| StoreError(e.to_string()))?;
            Ok(forbidden
                .into_iter()
                .map(|id| {
                    Violation::new(&FORBIDDEN, "a thing is labelled forbidden").with_subjects([id])
                })
                .collect())
        })
    }
}

/// The registry of a build with the module.
pub(crate) fn registry() -> Arc<ModuleRegistry> {
    Arc::new(ModuleRegistry::new(vec![Arc::new(Dummy)]).expect("a valid registry"))
}

/// A store whose changesets accept the operations of the module, its schema migrated.
pub(crate) async fn store(pool: &PgPool) -> PgStore {
    let modules = registry();
    kartenn_store_pg::modules::migrate(pool, &modules)
        .await
        .expect("the migrations of the module");
    PgStore::new(pool.clone()).with_modules(modules)
}

/// `dummy.create_thing` from its JSON fields.
pub(crate) fn create_thing(fields: serde_json::Value) -> Operation {
    let mut object = fields;
    object["op"] = "dummy.create_thing".into();
    serde_json::from_value(object).expect("an operation")
}
