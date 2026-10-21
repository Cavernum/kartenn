//! The handler of the module in the write pipeline (feature `pg`; ADR 0026, clause 6): the
//! operations on the axes and on the values of the axes (REQ-010-1 to 3, 29, 30) and on
//! the objects of the catalogue (`objects`, REQ-010-4 to 9). Every write takes the key
//! `policy` (S17) and raises the revision of the policy once per changeset (REQ-010-26),
//! which the event `org.kartenn.policy.revised` announces.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use kartenn_core::period::Period;
use kartenn_core::properties::PropertyKey;
use kartenn_core::rules::{Rule, Violation};
use kartenn_service::ObjectKind;
use kartenn_service::module::{
    BoxFuture, CheckContext, DiffContext, DiffSection, Findings, ModuleHandler, ModuleObjectRow,
    ModulePlan, ModuleTx, Reference,
};
use kartenn_service::operations::{ModuleKind, ModuleOperation, Operation, Ref};
use kartenn_service::port::{StoreError, WriteError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::checks;
use crate::model::Label;
use crate::module::{AXIS, ENVIRONMENT, EXAMPLES, INFERENCE_RULE, KINDS, VALUE};
use crate::objects::{plan_object, unique_columns};
use crate::pg::{self, table};
use crate::rules::{AXIS_INVALID, KEY_DUPLICATE, OPERATION_INVALID, VALUE_UNKNOWN};

/// The key that serialises the writes on the policy (S17), after the domains.
pub(crate) const LOCK: &str = "policy";

/// The event of a new revision of the policy (N15).
const REVISED: &str = "policy.revised";

/// The longest description of an axis value, in characters.
const DESCRIPTION_MAX_CHARS: usize = 4000;

/// The handler of the module.
#[derive(Debug, Clone, Copy, Default)]
pub struct Handler;

/// The qualified kinds of the module, parsed once.
fn kind(name: &str) -> ModuleKind {
    static PARSED: OnceLock<BTreeMap<&'static str, ModuleKind>> = OnceLock::new();
    PARSED.get_or_init(|| {
        KINDS
            .iter()
            .map(|(k, _)| (*k, ModuleKind::parse(k).expect("a valid kind")))
            .collect()
    })[name]
}

/// The versioned table of the objects of `kind`: `policy.policy_<type>`.
fn table_of(kind: ModuleKind) -> String {
    let name = kind.as_str();
    format!(
        "policy.policy_{}",
        name.strip_prefix("policy.").unwrap_or(name)
    )
}

/// The rule that refuses a second object of `kind` with the same unique columns.
fn duplicate_rule(kind: &str) -> &'static Rule {
    match kind {
        AXIS => &AXIS_INVALID,
        VALUE => &VALUE_UNKNOWN,
        _ => &KEY_DUPLICATE,
    }
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
}

/// A refusal of `operation` by `policy.operation.invalid`.
pub(crate) fn invalid(operation: &ModuleOperation, why: impl std::fmt::Display) -> Violation {
    Violation::new(&OPERATION_INVALID, format!("{}: {why}", operation.tag()))
}

fn decode<T: serde::de::DeserializeOwned>(operation: &ModuleOperation) -> Result<T, Violation> {
    serde_json::from_value(Value::Object(operation.body.clone())).map_err(|e| invalid(operation, e))
}

/// The carrier of an axis, as received (REQ-010-29).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Carrier {
    Tree,
    Machine,
}

impl Carrier {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Machine => "machine",
        }
    }
}

/// `policy.axis.designate`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Designate {
    #[serde(default)]
    local: Option<u32>,
    property: String,
    carrier: Carrier,
    #[serde(default)]
    inference: bool,
}

/// `policy.axis.update`: a Merge Patch of the axis.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateAxis {
    id: Ref,
    #[serde(default)]
    carrier: Option<Carrier>,
    #[serde(default)]
    inference: Option<bool>,
}

/// The attributes of a value (REQ-010-2, 30), as received.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attributes {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    transverse: Option<bool>,
    #[serde(default)]
    rank: Option<i32>,
}

/// What [`Handler::prepare`] reads for the plan of an operation on a value.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueCompleted {
    /// The attributes already recorded for the value, which a description modifies.
    #[serde(default)]
    existing: Option<Uuid>,
    /// The property `policy.environment`, for the value it creates or closes.
    #[serde(default)]
    property: Option<Uuid>,
    /// The closed-list value an environment closes.
    #[serde(default)]
    value: Option<Uuid>,
}

/// `policy.value.describe`: the attributes of a value of an axis, created or modified.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Describe {
    #[serde(default)]
    local: Option<u32>,
    property: String,
    value: String,
    #[serde(flatten)]
    attributes: Attributes,
    #[serde(default)]
    completed: ValueCompleted,
}

/// `policy.environment.create`: a value of `policy.environment` and its attributes, in one
/// operation (REQ-010-2).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateEnvironment {
    #[serde(default)]
    local: Option<u32>,
    key: String,
    #[serde(flatten)]
    attributes: Attributes,
    #[serde(default)]
    completed: ValueCompleted,
}

/// `policy.environment.close`: the value of `policy.environment`, closed with its
/// attributes; refused while a node or a rule cites it (REQ-010-2).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CloseEnvironment {
    key: String,
    #[serde(default)]
    completed: ValueCompleted,
}

/// The columns of the attributes `attributes`, checked (REQ-010-2, 30).
fn attribute_columns(
    operation: &ModuleOperation,
    attributes: &Attributes,
) -> Result<Map<String, Value>, Violation> {
    let mut columns = Map::new();
    if let Some(label) = &attributes.label {
        columns.insert("label".into(), Label::new(label)?.as_str().into());
    }
    if let Some(description) = &attributes.description {
        if description.chars().count() > DESCRIPTION_MAX_CHARS
            || description.chars().any(|c| c.is_control() && c != '\n')
        {
            return Err(invalid(
                operation,
                "a description has at most 4 000 characters, without control characters",
            ));
        }
        columns.insert("description".into(), description.as_str().into());
    }
    if let Some(transverse) = attributes.transverse {
        columns.insert("transverse".into(), transverse.into());
    }
    if let Some(rank) = attributes.rank {
        columns.insert("rank".into(), rank.into());
    }
    Ok(columns)
}

/// The row of new attributes of `value` of `property`: every column given (REQ-010-30).
fn value_row(
    object: Uuid,
    property: &str,
    value: &str,
    mut columns: Map<String, Value>,
) -> Map<String, Value> {
    for column in ["label", "description", "rank"] {
        columns.entry(column).or_insert(Value::Null);
    }
    columns.entry("transverse").or_insert(false.into());
    columns.insert("id".into(), object.to_string().into());
    columns.insert("property_key".into(), property.into());
    columns.insert("value_key".into(), value.into());
    columns
}

/// The key of a property, checked.
fn property_key(operation: &ModuleOperation, key: &str) -> Result<PropertyKey, Violation> {
    PropertyKey::parse(key).map_err(|_| invalid(operation, format!("{key}: not a property key")))
}

/// A plan of the module: every write takes the key of the policy (S17).
pub(crate) fn plan(kind: &'static str, object: Uuid, creates: bool, data: Value) -> ModulePlan {
    ModulePlan {
        kind: self::kind(kind),
        object,
        creates,
        domains: Vec::new(),
        keys: vec![LOCK.to_owned()],
        core: Vec::new(),
        data,
    }
}

/// The designation of `property` as an axis carried by `carrier` (REQ-010-29), as an
/// operation of the module: what `policy.examples.install` adds for each example.
fn designation(property: &str, carrier: Carrier) -> Operation {
    let mut body = Map::new();
    body.insert("property".into(), property.into());
    body.insert("carrier".into(), carrier.as_str().into());
    Operation::Module(ModuleOperation {
        module: "policy".into(),
        name: "axis.designate".into(),
        body,
    })
}

impl Handler {
    fn plan_designate(operation: &ModuleOperation, object: Uuid) -> Result<ModulePlan, Violation> {
        let designate: Designate = decode(operation)?;
        let _ = designate.local;
        let key = property_key(operation, &designate.property)?;
        if key.to_string() == ENVIRONMENT {
            return Err(Violation::new(
                &AXIS_INVALID,
                "policy.environment is an axis of the policy already (S22)",
            ));
        }
        Ok(plan(
            AXIS,
            object,
            true,
            json!({ "create": {
                "id": object,
                "property_key": key.to_string(),
                "carrier": designate.carrier.as_str(),
                "inference": designate.inference,
            }}),
        ))
    }

    fn plan_update_axis(
        operation: &ModuleOperation,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let update: UpdateAxis = decode(operation)?;
        let axis = resolve(update.id).ok_or_else(|| invalid(operation, "unknown axis"))?;
        let mut patch = Map::new();
        if let Some(carrier) = update.carrier {
            patch.insert("carrier".into(), carrier.as_str().into());
        }
        if let Some(inference) = update.inference {
            patch.insert("inference".into(), inference.into());
        }
        Ok(plan(AXIS, axis, false, json!({ "update": patch })))
    }

    fn plan_describe(operation: &ModuleOperation, object: Uuid) -> Result<ModulePlan, Violation> {
        let describe: Describe = decode(operation)?;
        let _ = describe.local;
        let key = property_key(operation, &describe.property)?;
        let columns = attribute_columns(operation, &describe.attributes)?;
        // A value of `policy.environment` is an environment (S15).
        let environment = key.to_string() == ENVIRONMENT;
        if let Some(existing) = describe.completed.existing {
            return Ok(plan(
                VALUE,
                existing,
                false,
                json!({ "update": columns, "environment": environment }),
            ));
        }
        let row = value_row(object, &key.to_string(), &describe.value, columns);
        Ok(plan(
            VALUE,
            object,
            true,
            json!({ "create": row, "environment": environment }),
        ))
    }

    fn plan_create_environment(
        operation: &ModuleOperation,
        object: Uuid,
    ) -> Result<ModulePlan, Violation> {
        let create: CreateEnvironment = decode(operation)?;
        let _ = create.local;
        let property = create
            .completed
            .property
            .ok_or_else(|| invalid(operation, "policy.environment is not declared"))?;
        let columns = attribute_columns(operation, &create.attributes)?;
        let row = value_row(object, ENVIRONMENT, &create.key, columns);
        let mut planned = plan(
            VALUE,
            object,
            true,
            json!({ "create": row, "environment": true }),
        );
        // The value of the closed list, an ordinary operation of the core (REQ-003-5).
        planned.core.push(Operation::AddEnumValue {
            local: None,
            property: Ref::Id(property),
            key: create.key,
        });
        Ok(planned)
    }

    fn plan_close_environment(operation: &ModuleOperation) -> Result<ModulePlan, Violation> {
        let close: CloseEnvironment = decode(operation)?;
        let value = close.completed.value.ok_or_else(|| {
            invalid(
                operation,
                format!("{} is not a living environment", close.key),
            )
        })?;
        // The value is closed by the core, its attributes with it (reference with
        // cascade); a node or a rule that cites it refuses the closure (REQ-010-2).
        let object = close.completed.existing.unwrap_or(value);
        let mut planned = plan(
            VALUE,
            object,
            false,
            json!({ "noop": true, "environment": true }),
        );
        planned.core.push(Operation::Close {
            kind: ObjectKind::EnumValue,
            id: Ref::Id(value),
            expected: None,
            cascade: true,
        });
        Ok(planned)
    }

    /// The axes of the examples (REQ-010-29), designated: the first by this operation, the
    /// others by operations of the module it adds; `policy.exposure` is carried by the
    /// tree, the others by the machine with the tree as default (S27).
    fn plan_examples(operation: &ModuleOperation, object: Uuid) -> Result<ModulePlan, Violation> {
        if !operation.body.is_empty() {
            return Err(invalid(operation, "the operation takes no field"));
        }
        let carrier = |key: &str| {
            if key == "policy.exposure" {
                Carrier::Tree
            } else {
                Carrier::Machine
            }
        };
        let (first, _) = EXAMPLES[0];
        let mut planned = plan(
            AXIS,
            object,
            true,
            json!({ "create": {
                "id": object,
                "property_key": first,
                "carrier": carrier(first).as_str(),
                "inference": false,
            }}),
        );
        planned.core = EXAMPLES[1..]
            .iter()
            .map(|(key, _)| designation(key, carrier(key)))
            .collect();
        Ok(planned)
    }

    /// Completes an operation on a value on the state of the transaction.
    async fn prepare_value(
        conn: &mut PgConnection,
        operation: &mut ModuleOperation,
        at: Option<sqlx::types::time::OffsetDateTime>,
    ) -> Result<(), StoreError> {
        let body = &mut operation.body;
        let text = |body: &Map<String, Value>, key: &str| {
            body.get(key).and_then(Value::as_str).map(str::to_owned)
        };
        let mut completed = ValueCompleted::default();
        match operation.name.as_str() {
            "value.describe" => {
                if let (Some(property), Some(value)) = (text(body, "property"), text(body, "value"))
                {
                    completed.existing = pg::find(
                        conn,
                        table::VALUE,
                        &[("property_key", &property), ("value_key", &value)],
                        at,
                    )
                    .await?;
                }
            }
            "environment.create" => {
                completed.property = pg::property(conn, ENVIRONMENT, at).await?;
            }
            "environment.close" => {
                if let Some(key) = text(body, "key") {
                    completed.value = pg::enum_value(conn, ENVIRONMENT, &key, at).await?;
                    completed.existing = pg::find(
                        conn,
                        table::VALUE,
                        &[("property_key", ENVIRONMENT), ("value_key", &key)],
                        at,
                    )
                    .await?;
                }
            }
            _ => return Ok(()),
        }
        body.insert(
            "completed".into(),
            serde_json::to_value(&completed).expect("a JSON object"),
        );
        Ok(())
    }

    /// Raises the revision of the rules of inference once per changeset (REQ-010-50), or
    /// that of the policy for any other object.
    async fn revised_for(
        tx: &mut dyn ModuleTx,
        kind: ModuleKind,
        changeset: Uuid,
    ) -> Result<(), StoreError> {
        if kind.as_str() == INFERENCE_RULE {
            pg::bump(connection(tx)?, "inference", changeset).await?;
            return Ok(());
        }
        Self::revised(tx, changeset).await
    }

    /// Raises the revision of the policy once per changeset, and announces it.
    async fn revised(tx: &mut dyn ModuleTx, changeset: Uuid) -> Result<(), StoreError> {
        if let Some(revision) = pg::bump(connection(tx)?, "policy", changeset).await? {
            tx.emit(REVISED, None, json!({ "revision": revision }))?;
        }
        Ok(())
    }

    /// Writes a planned operation, then its record in `module_object`, and raises the
    /// revision of the policy.
    async fn write(
        tx: &mut dyn ModuleTx,
        planned: &ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let id = planned.object;
        let name = planned.kind.as_str();
        let table = table_of(planned.kind);
        let (columns, written) = if let Some(row) = planned.data.get("create") {
            (row, true)
        } else if let Some(patch) = planned.data.get("update") {
            (patch, false)
        } else {
            // `environment.close`: the core closes the value and its attributes.
            return Ok(());
        };
        // The unique columns it writes, checked before the write: a violated exclusion
        // would abort the whole transaction.
        let unique: Vec<(&str, &str)> = unique_columns(name)
            .iter()
            .filter_map(|c| columns.get(*c).and_then(Value::as_str).map(|v| (*c, v)))
            .collect();
        let conn = connection(tx)?;
        if !unique.is_empty() {
            pg::unique(conn, &table, id, &unique, period, duplicate_rule(name)).await?;
        }
        if written {
            pg::insert(conn, &table, columns, period, changeset).await?;
        } else {
            pg::patch(conn, &table, id, columns, period, changeset).await?;
        }
        Self::revised_for(tx, planned.kind, changeset).await?;
        // A rule of inference is anchored in the domain it cites (REQ-010-46).
        let domain = if name == INFERENCE_RULE {
            crate::infer::domain_of(connection(tx)?, id).await?
        } else {
            None
        };
        tx.register(
            ModuleObjectRow {
                id,
                kind: planned.kind,
                parent: None,
                domain,
            },
            period,
            changeset,
        )
        .await
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
            if operation.name == "matrix.set" {
                return crate::matrix::prepare_set(connection(tx)?, operation, at).await;
            }
            Self::prepare_value(connection(tx)?, operation, at).await
        })
    }

    fn plan(
        &self,
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        match operation.name.as_str() {
            "axis.designate" => Self::plan_designate(operation, object),
            "axis.update" => Self::plan_update_axis(operation, resolve),
            "value.describe" => Self::plan_describe(operation, object),
            "environment.create" => Self::plan_create_environment(operation, object),
            "environment.close" => Self::plan_close_environment(operation),
            "examples.install" => Self::plan_examples(operation, object),
            "matrix.set" => crate::matrix::plan_set(operation, object, resolve),
            "inference_rule.create" | "inference_rule.update" => {
                crate::infer::plan_rule(operation, object, resolve)
            }
            _ => plan_object(operation, object, resolve)
                .unwrap_or_else(|| Err(invalid(operation, "unknown operation"))),
        }
    }

    /// An environment, a value of `policy.environment`, is written under
    /// `policy.environment.admin` besides the write of the values (S15).
    fn reads(&self, plan: &ModulePlan) -> Vec<(String, Uuid)> {
        if plan.data.get("environment") == Some(&serde_json::Value::Bool(true)) {
            vec![(crate::module::ENVIRONMENT_ADMIN.to_owned(), plan.object)]
        } else {
            Vec::new()
        }
    }

    fn apply<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        plan: &'a ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(Self::write(tx, plan, period, changeset))
    }

    fn close<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        kind: ModuleKind,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(async move {
            tx.close_versioned(&table_of(kind), id, period, changeset)
                .await?;
            Self::revised_for(tx, kind, changeset).await?;
            Ok(())
        })
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
            tx.restore_versioned(&table_of(kind), id, before, period, changeset)
                .await?;
            Self::revised(tx, changeset).await?;
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

    fn diff<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a DiffContext<'a>,
    ) -> BoxFuture<'a, Result<Option<DiffSection>, StoreError>> {
        Box::pin(async move { crate::contribution::section(connection(tx)?, context).await })
    }

    fn measure<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Vec<(&'static str, u64)>, StoreError>> {
        let _ = tx;
        Box::pin(std::future::ready(Ok(crate::contribution::measures(
            context,
        ))))
    }

    /// `policy.inference_check`: the deviations of the inference by origin node, re-evaluated
    /// by batches once its rules change (REQ-010-48; ADR 0026, amendment 4).
    fn run_job_findings<'a>(
        &'a self,
        name: &'a str,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        Box::pin(async move {
            if name == crate::module::INFERENCE_CHECK {
                crate::mismatch::run(tx).await
            } else {
                Ok(Findings::default())
            }
        })
    }

    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        Box::pin(async move {
            let conn = connection(tx)?;
            // The machines touched, whatever the module that wrote them (REQ-010-17).
            let mut findings = crate::coherence::check(conn, context).await?;
            if context.touched.is_empty() {
                return Ok(findings);
            }
            let mut found = Vec::new();
            for (axis, why) in pg::invalid_axes(conn, context.changeset).await? {
                found.push(Violation::new(&AXIS_INVALID, why).with_subjects([axis]));
            }
            for (value, which) in pg::unknown_values(conn, context.changeset).await? {
                found.push(
                    Violation::new(
                        &VALUE_UNKNOWN,
                        format!("{which} is not a living value of an axis of the policy"),
                    )
                    .with_subjects([value]),
                );
            }
            found.extend(checks::catalogue(conn, context.changeset).await?);
            found.extend(crate::infer::check(conn, context.changeset).await?);
            findings.violations.extend(found);
            Ok(findings)
        })
    }
}
