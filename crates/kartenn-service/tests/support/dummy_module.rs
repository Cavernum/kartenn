//! A module of test for the module contract (spec 008, tasks 008-3 and 4): one versioned
//! table, `dummy.create_thing`, that may reference a prefix, add a core operation and name
//! lock keys, and `dummy.update_thing`, that relabels a thing; a thing labelled
//! `forbidden` is refused by the module's own rule on the final state. Its rows end and
//! come back with the generic `close` and `restore`.

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
use sqlx::types::time::OffsetDateTime;
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

static OPERATIONS: &[OperationType] = &[
    OperationType {
        name: "create_thing",
        defines: Some("dummy.thing"),
    },
    OperationType {
        name: "update_thing",
        defines: None,
    },
];

static MIGRATIONS: &[ModuleMigration] = &[ModuleMigration {
    version: 1,
    name: "thing",
    sql: "CREATE TABLE thing (id uuid NOT NULL, label text NOT NULL, prefix_id uuid, \
          valid_period tstzrange NOT NULL, sys_start timestamptz NOT NULL DEFAULT now(), \
          changeset_id uuid NOT NULL, PRIMARY KEY (id, valid_period WITHOUT OVERLAPS), \
          UNIQUE (label, valid_period WITHOUT OVERLAPS)); \
          CALL kartenn_make_versioned('thing');",
}];

/// The versioned table of the things.
const TABLE: &str = "dummy.thing";

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

/// `dummy.update_thing`, as received.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateThing {
    id: Ref,
    label: String,
}

/// The bounds of a period to bind: `None` start is the instant of the transaction.
fn bounds(period: Option<Period>) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    let time = |i: kartenn_core::period::Instant| {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
    };
    period.map_or((None, None), |p| (Some(time(p.start())), p.end().map(time)))
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
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
        if operation.name == "update_thing" {
            let update: UpdateThing =
                serde_json::from_value(serde_json::Value::Object(operation.body.clone()))
                    .map_err(|e| invalid(e.to_string()))?;
            let target = resolve(update.id).ok_or_else(|| invalid("unknown thing".into()))?;
            return Ok(ModulePlan {
                kind: kind(),
                object: target,
                creates: false,
                domains: Vec::new(),
                keys: Vec::new(),
                core: Vec::new(),
                data: serde_json::json!({"label": update.label, "update": true}),
            });
        }
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
            let (a, b) = bounds(period);
            let label = plan.data["label"].as_str();
            if plan.data["update"].as_bool() == Some(true) {
                // Relabelled where it lives over the period (a test module: no split).
                let updated = sqlx::query(
                    "UPDATE dummy.thing SET label = $2, changeset_id = $5 WHERE id = $1
                       AND valid_period && tstzrange(coalesce($3::timestamptz, now()), $4)",
                )
                .bind(plan.object)
                .bind(label)
                .bind(a)
                .bind(b)
                .bind(changeset)
                .execute(&mut *connection(tx)?)
                .await
                .map_err(|e| StoreError(e.to_string()))?;
                if updated.rows_affected() == 0 {
                    return Err(WriteError::NoVersion(plan.object));
                }
            } else {
                let prefix: Option<Uuid> =
                    plan.data["prefix"].as_str().and_then(|p| p.parse().ok());
                sqlx::query(
                    "INSERT INTO dummy.thing (id, label, prefix_id, valid_period, changeset_id)
                     VALUES ($1, $2, $3, tstzrange(coalesce($4::timestamptz, now()), $5), $6)",
                )
                .bind(plan.object)
                .bind(label)
                .bind(prefix)
                .bind(a)
                .bind(b)
                .bind(changeset)
                .execute(&mut *connection(tx)?)
                .await
                .map_err(|e| StoreError(e.to_string()))?;
            }
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

    fn close<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        _kind: ModuleKind,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        tx.close_versioned(TABLE, id, period, changeset)
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
        tx.restore_versioned(TABLE, id, before, period, changeset)
    }

    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        touched: &'a [(ModuleKind, Uuid)],
    ) -> BoxFuture<'a, Result<Vec<Violation>, StoreError>> {
        Box::pin(async move {
            let ids: Vec<Uuid> = touched.iter().map(|(_, id)| *id).collect();
            let forbidden: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM dummy.thing WHERE id = ANY($1) AND label = 'forbidden'
                   AND valid_period @> now()",
            )
            .bind(&ids)
            .fetch_all(connection(tx)?)
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

/// `dummy.update_thing` from its JSON fields.
#[allow(dead_code)] // not every test binary relabels
pub(crate) fn update_thing(fields: serde_json::Value) -> Operation {
    let mut object = fields;
    object["op"] = "dummy.update_thing".into();
    serde_json::from_value(object).expect("an operation")
}

/// The type of the things.
#[allow(dead_code)] // not every test binary names it
pub(crate) fn thing_kind() -> kartenn_service::ObjectKind {
    kartenn_service::ObjectKind::Module(kind())
}
