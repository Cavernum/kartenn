//! A module of test for the module contract (spec 008, tasks 008-3 and 4): one versioned
//! table, `dummy.create_thing`, that may reference a prefix, add a core operation and name
//! lock keys, and `dummy.update_thing`, that relabels a thing; a thing labelled
//! `forbidden` is refused by the module's own rule on the final state. Its rows end and
//! come back with the generic `close` and `restore`. A thing references its prefix (closed
//! with it in a cascade) and a domain (cleared in a cascade) (task 008-7).

use std::sync::Arc;

use kartenn_core::period::Period;
use kartenn_core::rules::{Rule, Severity, Violation};
use kartenn_service::module::{
    BoxFuture, CheckContext, Findings, Module, ModuleHandler, ModuleMigration, ModuleObjectRow,
    ModulePlan, ModuleRegistry, ModuleTx, ObjectType, OnClose, OperationType, Reference,
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
    sql: "CREATE TABLE thing (id uuid NOT NULL, label text NOT NULL, prefix_id uuid, domain_ref uuid, \
          valid_period tstzrange NOT NULL, sys_start timestamptz NOT NULL DEFAULT now(), \
          changeset_id uuid NOT NULL, PRIMARY KEY (id, valid_period WITHOUT OVERLAPS), \
          UNIQUE (label, valid_period WITHOUT OVERLAPS)); \
          CALL kartenn_make_versioned('thing');",
}];

/// The versioned table of the things.
const TABLE: &str = "dummy.thing";

/// A thing closes with its prefix; its domain is cleared when the domain closes.
static REFERENCES: &[Reference] = &[
    Reference {
        from: "dummy.thing",
        to: "prefix",
        on_close: OnClose::Cascade,
    },
    Reference {
        from: "dummy.thing",
        to: "domain",
        on_close: OnClose::Nullify,
    },
];

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
    fn references(&self) -> &'static [Reference] {
        REFERENCES
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        Some(Arc::new(Handler))
    }
    fn jobs(&self) -> &'static [kartenn_service::module::JobSpec] {
        const JOBS: &[kartenn_service::module::JobSpec] = &[kartenn_service::module::JobSpec {
            name: "tick",
            period: std::time::Duration::from_secs(3_600),
        }];
        JOBS
    }
    fn events(&self) -> &'static [&'static str] {
        &["created"]
    }
    fn measures(&self) -> &'static [&'static str] {
        &["things"]
    }
    fn default_policies(&self) -> &'static [kartenn_service::module::DefaultPolicy] {
        &[kartenn_service::module::DefaultPolicy {
            name: "dummy-many-things",
            triggers: &[("things", 10)],
        }]
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
    /// A domain it references.
    #[serde(default)]
    domain_ref: Option<Ref>,
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
        let domain_ref = match thing.domain_ref {
            Some(r) => Some(resolve(r).ok_or_else(|| invalid("unknown domain".into()))?),
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
            data: serde_json::json!({"label": thing.label, "prefix": prefix, "domain_ref": domain_ref}),
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
            if plan.data["clear_domain"].as_bool() == Some(true) {
                sqlx::query(
                    "UPDATE dummy.thing SET domain_ref = NULL, changeset_id = $4 WHERE id = $1
                       AND valid_period && tstzrange(coalesce($2::timestamptz, now()), $3)",
                )
                .bind(plan.object)
                .bind(a)
                .bind(b)
                .bind(changeset)
                .execute(&mut *connection(tx)?)
                .await
                .map_err(|e| StoreError(e.to_string()))?;
            } else if plan.data["update"].as_bool() == Some(true) {
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
                let uuid = |field: &str| -> Option<Uuid> {
                    plan.data[field].as_str().and_then(|p| p.parse().ok())
                };
                sqlx::query(
                    "INSERT INTO dummy.thing (id, label, prefix_id, domain_ref, valid_period, changeset_id)
                     VALUES ($1, $2, $3, $7, tstzrange(coalesce($4::timestamptz, now()), $5), $6)",
                )
                .bind(plan.object)
                .bind(label)
                .bind(uuid("prefix"))
                .bind(a)
                .bind(b)
                .bind(changeset)
                .bind(uuid("domain_ref"))
                .execute(&mut *connection(tx)?)
                .await
                .map_err(|e| StoreError(e.to_string()))?;
            }
            // A thing labelled `emits` emits `dummy.created`; `emits-undeclared`, an event
            // the module does not declare (REQ-016-15).
            match label {
                Some("emits") => tx.emit(
                    "dummy.created",
                    Some(plan.object.to_string()),
                    serde_json::json!({"label": label}),
                )?,
                Some("emits-undeclared") => {
                    tx.emit("dummy.undeclared", None, serde_json::json!({}))?;
                }
                _ => {}
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

    fn measure<'a>(
        &'a self,
        _tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Vec<(&'static str, u64)>, StoreError>> {
        // The things the changeset writes.
        let things = context.touched.len() as u64;
        Box::pin(std::future::ready(Ok(vec![("things", things)])))
    }

    fn run_job<'a>(
        &'a self,
        name: &'a str,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move { tx.emit("dummy.created", None, serde_json::json!({"job": name})) })
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

    fn referrers<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        reference: &'a Reference,
        target: Uuid,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<Vec<Uuid>, StoreError>> {
        Box::pin(async move {
            let sql = match reference.to {
                "prefix" => {
                    "SELECT DISTINCT id FROM dummy.thing WHERE prefix_id = $1
                       AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)"
                }
                _ => {
                    "SELECT DISTINCT id FROM dummy.thing WHERE domain_ref = $1
                       AND valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)"
                }
            };
            sqlx::query_scalar(sql)
                .bind(target)
                .bind(bounds(period).0)
                .fetch_all(connection(tx)?)
                .await
                .map_err(|e| StoreError(e.to_string()))
        })
    }

    fn nullify(&self, reference: &Reference, source: Uuid, _target: Uuid) -> Option<ModulePlan> {
        (reference.to == "domain").then(|| ModulePlan {
            kind: kind(),
            object: source,
            creates: false,
            domains: Vec::new(),
            keys: Vec::new(),
            core: Vec::new(),
            data: serde_json::json!({"clear_domain": true}),
        })
    }

    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        Box::pin(async move {
            let ids: Vec<Uuid> = context.touched.iter().map(|(_, id)| *id).collect();
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
                .collect::<Vec<_>>()
                .into())
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
