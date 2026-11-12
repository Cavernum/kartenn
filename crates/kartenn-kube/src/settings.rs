//! The settings of the module in the write pipeline (feature `pg`; spec 019, REQ-019-34,
//! Q-019-35; design « Opérations »): three types of the module, global, written under
//! `kube.settings.edit` by their type permissions (spec 027, P4, P5).
//!
//! - `kube.set_label_mapping {action: upsert, label_key, label_value, axis, value}` creates
//!   the mapping of (label key, label value, axis) or changes its value;
//!   `{action: close, id}` closes it.
//! - `kube.set_admitted_values {namespace: {id}, axis, values}` replaces the set of the
//!   values of the axis admitted in the namespace; an empty set closes it.
//! - `kube.set_tracked_labels {keys}` replaces the label keys tracked in every cluster
//!   (019/X4): the new ones created, the others closed.
//!
//! An axis is a closed list carried by `kube.workload`, neither sensitive nor
//! `kube.namespace` (set by `kube.sync` alone, Q-019-39); a value is a living value of
//! its list. What the settings change on the workloads is derived by the server (tasks
//! 019-7d2, 7d3).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use kartenn_core::period::Period;
use kartenn_core::rules::Violation;
use kartenn_service::ObjectKind;
use kartenn_service::module::{ModuleObjectRow, ModulePlan, ModuleTx};
use kartenn_service::operations::{ModuleKind, ModuleOperation, Operation, Ref};
use kartenn_service::port::{STALE_VERSION, StoreError, WriteError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::derivation::{Recheck, Rederived};
use crate::module::{ADMITTED_VALUES, LABEL_MAPPING, NAMESPACE_AXIS, TRACKED_LABEL, WORKLOAD};
use crate::names::{LabelKey, LabelValue};
use crate::pg::{
    self, ADMITTED_VALUES_TABLE, LABEL_MAPPING_TABLE, NAMESPACE_TABLE, TRACKED_LABEL_TABLE,
};
use crate::rules::OPERATION_INVALID;

/// The operations of the settings.
const OPERATIONS: [&str; 3] = [
    "set_label_mapping",
    "set_admitted_values",
    "set_tracked_labels",
];

/// The lock key of the writes of the settings: two changesets of the settings do not
/// cross.
const LOCK: &str = "kube.settings";

/// The read of the namespace the admitted values cite.
const NAMESPACE_READ: &str = "kube.namespace.read";

/// Whether `name` is an operation of the settings.
pub(crate) fn is_setting(name: &str) -> bool {
    OPERATIONS.contains(&name)
}

fn kind(name: &'static str, cell: &'static OnceLock<ModuleKind>) -> ModuleKind {
    *cell.get_or_init(|| ModuleKind::parse(name).expect("a valid kind"))
}

fn mapping_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(LABEL_MAPPING, &KIND)
}

fn admitted_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(ADMITTED_VALUES, &KIND)
}

fn tracked_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    kind(TRACKED_LABEL, &KIND)
}

/// The table of `kind`, when a type of the settings.
pub(crate) fn table(kind: ModuleKind) -> Option<&'static str> {
    if kind == mapping_kind() {
        Some(LABEL_MAPPING_TABLE)
    } else if kind == admitted_kind() {
        Some(ADMITTED_VALUES_TABLE)
    } else if kind == tracked_kind() {
        Some(TRACKED_LABEL_TABLE)
    } else {
        None
    }
}

fn invalid(message: impl Into<String>) -> Violation {
    Violation::new(&OPERATION_INVALID, message.into())
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
}

/// A property as declared at the start of the period, to judge it as an axis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Axis {
    value_type: String,
    carriers: Vec<String>,
    sensitive: bool,
    /// Whether it takes a set of values (`cardinality = 'set'`): two labels give the union
    /// of their values (Q-019-40).
    #[serde(default)]
    pub(crate) set: bool,
    /// The living values of its closed list (its own, or those it borrows).
    pub(crate) values: Vec<String>,
}

/// What [`prepare`] reads for the plan, which reads no database.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Completed {
    /// The axis the operation names.
    #[serde(default)]
    axis: Option<Axis>,
    /// The living mapping of the label and axis (`upsert`) or of the identifier (`close`);
    /// the living admitted values of the namespace and axis.
    #[serde(default)]
    existing: Option<Uuid>,
    /// The namespace named, when it lives.
    #[serde(default)]
    namespace: Option<Uuid>,
    /// The tracked labels living, by key.
    #[serde(default)]
    tracked: Vec<(String, Uuid)>,
    /// The tracked label to create, in an operation a `set_tracked_labels` adds; in an
    /// operation of a client, [`prepare`] replaces it (refused then).
    #[serde(default)]
    create: Option<String>,
    /// The workloads the settings of the changeset concern, with what their derivation
    /// reads on the final state of the settings: in its last operation of the settings
    /// alone ([`complete_changeset`], Q-019-42).
    #[serde(default)]
    rederive: Vec<Rederived>,
    /// What the re-derivation read, read again once the locks are taken.
    #[serde(default)]
    recheck: Option<Recheck>,
}

/// Completes the operations of the module in a changeset once each is prepared (ADR 0026,
/// amendment 16; Q-019-36, 42): the last operation of the settings receives the
/// workloads all of them concern, derived once each on the final state of the settings.
pub(crate) async fn complete_changeset(
    conn: &mut PgConnection,
    operations: &mut [&mut ModuleOperation],
    at: Option<OffsetDateTime>,
) -> Result<(), StoreError> {
    let Some(last) = operations.iter().rposition(|o| is_setting(&o.name)) else {
        return Ok(());
    };
    let settings: Vec<&ModuleOperation> = operations
        .iter()
        .filter(|o| is_setting(&o.name))
        .map(|o| &**o)
        .collect();
    let (rederived, recheck) = crate::derivation::rederive(conn, &settings, at).await?;
    let operation = &mut *operations[last];
    let mut completed = completed_of(operation);
    completed.rederive = rederived;
    completed.recheck = recheck;
    operation.body.insert(
        "completed".into(),
        serde_json::to_value(&completed).expect("a JSON object"),
    );
    Ok(())
}

/// Reads again, once the locks are taken, what the re-derivation of `plan` read: when a
/// batch committed while this changeset waited for its locks has changed the labels of a
/// workload, or which workloads are concerned, the changeset is replayed in a new
/// transaction (`STALE_VERSION`, task 021-22p), which reads them anew.
pub(crate) async fn recheck(
    tx: &mut dyn ModuleTx,
    plan: &ModulePlan,
    period: Option<Period>,
) -> Result<(), WriteError> {
    let Some(recheck) = plan.data.get("recheck") else {
        return Ok(());
    };
    let recheck: Recheck =
        serde_json::from_value(recheck.clone()).map_err(|e| StoreError(e.to_string()))?;
    let conn = connection(tx)?;
    let (at, _) = pg::bounds(period);
    match crate::derivation::changed(conn, &recheck, at).await? {
        Some(workload) => Err(StoreError(format!(
            "{STALE_VERSION} {workload} changed while the settings waited for their locks"
        ))
        .into()),
        None => Ok(()),
    }
}

/// The settings written over a period of their own (a `Within`) among the author's
/// `operations`, refused: the settings are written in the present, their re-derivation
/// reading the period of the changeset (Q-019-42).
pub(crate) fn within(operations: &[Operation]) -> Vec<Violation> {
    operations
        .iter()
        .enumerate()
        .filter(|(_, o)| {
            matches!(o, Operation::Within { .. })
                && matches!(o.inner(), Operation::Module(m) if m.module == "kube" && is_setting(&m.name))
        })
        .map(|(i, _)| {
            invalid(format!(
                "the operation at position {i} writes the settings within a period of its own: the settings are written in the present"
            ))
        })
        .collect()
}

/// The workloads the settings of `operations`, prepared, re-derive: their rules of the
/// derivation are evaluated anew (`kube.label.not_admitted`, `kube.label.conflict`).
pub(crate) fn rederived(operations: &[Operation]) -> Vec<Uuid> {
    operations
        .iter()
        .filter_map(|o| match o.inner() {
            Operation::Module(m) if m.module == "kube" && is_setting(&m.name) => Some(m),
            _ => None,
        })
        .flat_map(|m| completed_of(m).rederive)
        .map(|r| r.workload)
        .collect()
}

fn completed_of(operation: &ModuleOperation) -> Completed {
    operation
        .body
        .get("completed")
        .cloned()
        .and_then(|c| serde_json::from_value(c).ok())
        .unwrap_or_default()
}

/// Adds to `plan` the assignments and removals of the core the derivation of `rederived`
/// plans (the provenances by position, read back by the handler's `provenance`), under the
/// locks of their clusters: a batch of a cluster does not cross the re-derivation.
fn with_rederived(mut plan: ModulePlan, completed: &Completed) -> ModulePlan {
    if let Some(recheck) = &completed.recheck {
        plan.data["recheck"] = json!(recheck);
    }
    let rederived = &completed.rederive;
    if rederived.is_empty() {
        return plan;
    }
    let workload = ModuleKind::parse(WORKLOAD).expect("a valid kind");
    let mut provenances: Vec<Option<String>> = vec![None; plan.core.len()];
    for r in rederived {
        let derived = crate::derivation::derive(
            &r.facts,
            workload,
            r.workload,
            r.cluster,
            &r.namespace,
            None,
        );
        for (operation, provenance) in derived {
            plan.core.push(operation);
            provenances.push(provenance);
        }
        let key = crate::handler::cluster_key(r.cluster);
        if !plan.keys.contains(&key) {
            plan.keys.push(key);
        }
    }
    plan.data["provenances"] = json!(provenances);
    plan
}

/// The living object of `table` that matches `condition` (on the texts `$1` to `$n` of
/// `keys`) at `at`.
async fn living(
    conn: &mut PgConnection,
    table: &str,
    condition: &str,
    keys: &[&str],
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    let sql = format!(
        "SELECT id FROM {table}
          WHERE {condition} AND valid_period @> coalesce(${}::timestamptz, now())
          ORDER BY id LIMIT 1",
        keys.len() + 1
    );
    let mut query = sqlx::query_scalar(sqlx::AssertSqlSafe(sql));
    for key in keys {
        query = query.bind(*key);
    }
    query
        .bind(at)
        .fetch_optional(conn)
        .await
        .map_err(|e| pg::store(&e))
}

/// The property `key` declared at `at`, with the living values of its closed list.
pub(crate) async fn axis_of(
    conn: &mut PgConnection,
    key: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Axis>, StoreError> {
    type Row = (String, Vec<String>, bool, bool, Vec<String>);
    let row: Option<Row> = sqlx::query_as(
        "SELECT d.value_type, d.carriers, d.sensitive, d.cardinality = 'set',
                ARRAY(SELECT v.key FROM public.property_enum_value v
                       WHERE v.property_id = public.kartenn_values_owner(d.id)
                         AND v.valid_period @> coalesce($2::timestamptz, now())
                       ORDER BY v.key)
           FROM public.property_declaration d
          WHERE d.key = $1 AND d.valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(key)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| pg::store(&e))?;
    Ok(
        row.map(|(value_type, carriers, sensitive, set, values)| Axis {
            value_type,
            carriers,
            sensitive,
            set,
            values,
        }),
    )
}

/// The tracked labels living at `at`, by key.
pub(crate) async fn tracked(
    conn: &mut PgConnection,
    at: Option<OffsetDateTime>,
) -> Result<Vec<(String, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT label_key, id FROM kube.tracked_label
          WHERE valid_period @> coalesce($1::timestamptz, now())
          ORDER BY label_key",
    )
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| pg::store(&e))
}

/// Completes an operation of the settings on the state of the transaction.
pub(crate) async fn prepare(
    conn: &mut PgConnection,
    operation: &mut ModuleOperation,
    at: Option<OffsetDateTime>,
) -> Result<(), StoreError> {
    let body = &operation.body;
    let text = |field: &str| body.get(field).and_then(Value::as_str).map(str::to_owned);
    let id = |field: &str| match body
        .get(field)
        .cloned()
        .and_then(|v| serde_json::from_value::<Ref>(v).ok())
    {
        Some(Ref::Id(id)) => Some(id.to_string()),
        _ => None,
    };
    let mut completed = Completed::default();
    if let Some(axis) = text("axis") {
        completed.axis = axis_of(conn, &axis, at).await?;
    }
    match operation.name.as_str() {
        "set_label_mapping" => {
            if let (Some(key), Some(value), Some(axis)) =
                (text("label_key"), text("label_value"), text("axis"))
            {
                completed.existing = living(
                    conn,
                    LABEL_MAPPING_TABLE,
                    "label_key = $1 AND label_value = $2 AND axis_key = $3",
                    &[&key, &value, &axis],
                    at,
                )
                .await?;
            } else if let (Some("close"), Some(id)) = (text("action").as_deref(), id("id")) {
                completed.existing =
                    living(conn, LABEL_MAPPING_TABLE, "id = $1::uuid", &[&id], at).await?;
            }
        }
        "set_admitted_values" => {
            if let Some(namespace) = id("namespace") {
                completed.namespace =
                    living(conn, NAMESPACE_TABLE, "id = $1::uuid", &[&namespace], at).await?;
                if let Some(axis) = text("axis") {
                    completed.existing = living(
                        conn,
                        ADMITTED_VALUES_TABLE,
                        "namespace_id = $1::uuid AND axis_key = $2",
                        &[&namespace, &axis],
                        at,
                    )
                    .await?;
                }
            }
        }
        _ => completed.tracked = tracked(conn, at).await?,
    }
    operation.body.insert(
        "completed".into(),
        serde_json::to_value(&completed).expect("a JSON object"),
    );
    Ok(())
}

/// The body of `operation`, refused if it carries a field out of `fields`, and what
/// [`prepare`] completed.
fn body(
    operation: &ModuleOperation,
    fields: &[&str],
) -> Result<(Map<String, Value>, Completed), Violation> {
    if let Some(unknown) = operation
        .body
        .keys()
        .find(|k| !fields.contains(&k.as_str()) && k.as_str() != "completed")
    {
        return Err(invalid(format!(
            "{}: unknown field `{unknown}`",
            operation.tag()
        )));
    }
    let completed = operation
        .body
        .get("completed")
        .cloned()
        .and_then(|c| serde_json::from_value(c).ok())
        .unwrap_or_default();
    Ok((operation.body.clone(), completed))
}

/// The text of `field`, required.
fn text<'a>(body: &'a Map<String, Value>, field: &str) -> Result<&'a str, Violation> {
    match body.get(field) {
        Some(Value::String(text)) => Ok(text),
        Some(_) => Err(invalid(format!("{field}: a string"))),
        None => Err(invalid(format!("{field}: required"))),
    }
}

/// The axis `key`, as declared: a closed list carried by the workloads, neither sensitive
/// nor `kube.namespace`; the derivation of a workload ignores a mapping to another
/// (task 019-7d2).
pub(crate) fn checked_axis<'a>(key: &str, axis: Option<&'a Axis>) -> Result<&'a Axis, Violation> {
    if key == NAMESPACE_AXIS {
        return Err(invalid(format!(
            "axis: {NAMESPACE_AXIS} is set by kube.sync alone, no setting gives it"
        )));
    }
    let axis = axis.ok_or_else(|| invalid(format!("axis: no property {key} is declared")))?;
    if axis.sensitive {
        return Err(invalid(format!(
            "axis: {key} is sensitive, a label does not give it"
        )));
    }
    if axis.value_type != "enum" {
        return Err(invalid(format!("axis: {key} is not a closed list")));
    }
    if !axis.carriers.iter().any(|c| c == WORKLOAD) {
        return Err(invalid(format!("axis: {key} is not carried by {WORKLOAD}")));
    }
    Ok(axis)
}

/// `value`, a living value of the list of `axis`.
fn checked_value(axis: &Axis, key: &str, value: &str, field: &str) -> Result<(), Violation> {
    if axis.values.iter().any(|v| v == value) {
        Ok(())
    } else {
        Err(invalid(format!("{field}: {value} is not a value of {key}")))
    }
}

/// A plan of the settings, under their lock.
fn plan_of(kind: ModuleKind, object: Uuid, creates: bool, data: Value) -> ModulePlan {
    ModulePlan {
        kind,
        object,
        creates,
        domains: Vec::new(),
        keys: vec![LOCK.to_owned()],
        core: Vec::new(),
        data,
    }
}

/// The closure of the setting `id` of `kind` by the core.
fn close(kind: ModuleKind, id: Uuid) -> Operation {
    Operation::Close {
        kind: ObjectKind::Module(kind),
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    }
}

/// A plan that writes nothing itself and closes `ids` (the first its object) by the core.
fn closing(kind: ModuleKind, ids: &[Uuid]) -> Option<ModulePlan> {
    let (&first, _) = ids.split_first()?;
    let mut plan = plan_of(kind, first, false, json!({ "noop": true }));
    plan.core = ids.iter().map(|&id| close(kind, id)).collect();
    Some(plan)
}

/// Plans an operation of the settings.
pub(crate) fn plan(
    operation: &ModuleOperation,
    object: Uuid,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    let plan = match operation.name.as_str() {
        "set_label_mapping" => plan_mapping(operation, object, resolve),
        "set_admitted_values" => plan_admitted(operation, object, resolve),
        _ => plan_tracked(operation, object),
    }?;
    Ok(with_rederived(plan, &completed_of(operation)))
}

/// The identifier the reference `field` of `body` names.
fn reference(
    body: &Map<String, Value>,
    field: &str,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<Uuid, Violation> {
    let value = body
        .get(field)
        .ok_or_else(|| invalid(format!("{field}: required")))?;
    let r: Ref =
        serde_json::from_value(value.clone()).map_err(|e| invalid(format!("{field}: {e}")))?;
    resolve(r).ok_or_else(|| invalid(format!("{field}: unknown object")))
}

fn plan_mapping(
    operation: &ModuleOperation,
    object: Uuid,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    match operation.body.get("action").and_then(Value::as_str) {
        Some("upsert") => {
            let (body, completed) = body(
                operation,
                &["action", "label_key", "label_value", "axis", "value"],
            )?;
            let label_key = LabelKey::parse(text(&body, "label_key")?)?;
            let label_value = LabelValue::parse(text(&body, "label_value")?)?;
            let key = text(&body, "axis")?;
            let axis = checked_axis(key, completed.axis.as_ref())?;
            let value = text(&body, "value")?;
            checked_value(axis, key, value, "value")?;
            Ok(match completed.existing {
                Some(existing) => plan_of(
                    mapping_kind(),
                    existing,
                    false,
                    json!({ "update": { "axis_value": value } }),
                ),
                None => plan_of(
                    mapping_kind(),
                    object,
                    true,
                    json!({ "create": {
                        "id": object,
                        "label_key": label_key.as_str(),
                        "label_value": label_value.as_str(),
                        "axis_key": key,
                        "axis_value": value,
                    }}),
                ),
            })
        }
        Some("close") => {
            let (body, completed) = body(operation, &["action", "id"])?;
            let id = reference(&body, "id", resolve)?;
            if completed.existing != Some(id) {
                return Err(invalid(format!("id: {id} is not a living label mapping")));
            }
            Ok(closing(mapping_kind(), &[id]).expect("one mapping"))
        }
        _ => Err(invalid("action: upsert or close")),
    }
}

fn plan_admitted(
    operation: &ModuleOperation,
    object: Uuid,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    let (body, completed) = body(operation, &["namespace", "axis", "values"])?;
    let namespace = reference(&body, "namespace", resolve)?;
    if completed.namespace != Some(namespace) {
        return Err(invalid(format!(
            "namespace: {namespace} is not a living namespace"
        )));
    }
    let key = text(&body, "axis")?;
    let axis = checked_axis(key, completed.axis.as_ref())?;
    let values: Vec<String> = match body.get("values") {
        Some(values) => serde_json::from_value(values.clone())
            .map_err(|_| invalid("values: an array of strings"))?,
        None => return Err(invalid("values: required")),
    };
    // A set: the same values make the same admitted values.
    let values: BTreeSet<String> = values.into_iter().collect();
    for value in &values {
        checked_value(axis, key, value, "values")?;
    }
    let mut plan = match (completed.existing, values.is_empty()) {
        (Some(existing), true) => closing(admitted_kind(), &[existing]).expect("one object"),
        (Some(existing), false) => plan_of(
            admitted_kind(),
            existing,
            false,
            json!({ "update": { "axis_values": values } }),
        ),
        // Nothing admitted, nothing to write.
        (None, true) => plan_of(admitted_kind(), object, true, json!({ "noop": true })),
        (None, false) => plan_of(
            admitted_kind(),
            object,
            true,
            json!({ "create": {
                "id": object,
                "namespace_id": namespace,
                "axis_key": key,
                "axis_values": values,
            }}),
        ),
    };
    // The namespace it cites is read by its author, as a cluster its domains (009/M16).
    plan.data["reads"] = json!([[NAMESPACE_READ, namespace]]);
    Ok(plan)
}

/// The operation, added by `set_tracked_labels`, that creates the tracked label `key`.
fn tracked_creation(key: &str) -> Operation {
    let Value::Object(body) = json!({ "completed": { "create": key } }) else {
        unreachable!("an object literal")
    };
    Operation::Module(ModuleOperation {
        module: "kube".into(),
        name: "set_tracked_labels".into(),
        body,
    })
}

/// The row of the tracked label `key`, created as `object`.
fn tracked_row(object: Uuid, key: &str) -> Value {
    json!({ "create": { "id": object, "label_key": key } })
}

/// The plan of `set_tracked_labels`: creates the first new key, adds an operation for each
/// other and closes the keys no longer listed; without a new key, closes them; without a
/// change, writes nothing.
fn plan_tracked(operation: &ModuleOperation, object: Uuid) -> Result<ModulePlan, Violation> {
    let (body, completed) = body(operation, &["keys"])?;
    if let Some(key) = completed.create {
        return Ok(plan_of(
            tracked_kind(),
            object,
            true,
            tracked_row(object, &key),
        ));
    }
    let keys: Vec<String> = match body.get("keys") {
        Some(keys) => serde_json::from_value(keys.clone())
            .map_err(|_| invalid("keys: an array of strings"))?,
        None => return Err(invalid("keys: required")),
    };
    let keys: BTreeSet<String> = keys
        .iter()
        .map(|k| LabelKey::parse(k).map(|k| k.as_str().to_owned()))
        .collect::<Result<_, _>>()?;
    let new: Vec<&String> = keys
        .iter()
        .filter(|k| !completed.tracked.iter().any(|(t, _)| t == *k))
        .collect();
    let gone: Vec<Uuid> = completed
        .tracked
        .iter()
        .filter(|(t, _)| !keys.contains(t))
        .map(|(_, id)| *id)
        .collect();
    let Some((first, others)) = new.split_first() else {
        return Ok(closing(tracked_kind(), &gone)
            .unwrap_or_else(|| plan_of(tracked_kind(), object, true, json!({ "noop": true }))));
    };
    let mut plan = plan_of(tracked_kind(), object, true, tracked_row(object, first));
    plan.core = others
        .iter()
        .map(|k| tracked_creation(k))
        .chain(gone.iter().map(|&id| close(tracked_kind(), id)))
        .collect();
    Ok(plan)
}

/// The identity of a setting of `table` in `row`, its condition and keys: one at every
/// instant.
fn identity<'a>(table: &str, row: &'a Value) -> (&'static str, Vec<&'a str>) {
    let field = |name: &str| row.get(name).and_then(Value::as_str).unwrap_or_default();
    match table {
        LABEL_MAPPING_TABLE => (
            "label_key = $1 AND label_value = $2 AND axis_key = $3",
            vec![field("label_key"), field("label_value"), field("axis_key")],
        ),
        ADMITTED_VALUES_TABLE => (
            "namespace_id = $1::uuid AND axis_key = $2",
            vec![field("namespace_id"), field("axis_key")],
        ),
        _ => ("label_key = $1", vec![field("label_key")]),
    }
}

/// Writes the setting of `plan`: its row created or changed, the object registered.
pub(crate) async fn apply(
    tx: &mut dyn ModuleTx,
    plan: &ModulePlan,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let id = plan.object;
    let table = table(plan.kind).expect("a type of the settings");
    let conn = connection(tx)?;
    if let Some(row) = plan.data.get("create") {
        // Given twice in a changeset: refused before the exclusion constraint, whose
        // failure would abort the transaction.
        let (condition, keys) = identity(table, row);
        if pg::taken(conn, table, condition, &keys, id, period).await? {
            return Err(pg::refused(
                &OPERATION_INVALID,
                format!(
                    "{}: {} is given twice over the period",
                    plan.kind,
                    keys.join(", ")
                ),
                &[id],
            ));
        }
        pg::insert(conn, table, row, period, changeset).await?;
    } else {
        pg::portion(conn, table, id, &plan.data["update"], period, changeset).await?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(value_type: &str, carriers: &[&str], sensitive: bool) -> Axis {
        Axis {
            value_type: value_type.into(),
            carriers: carriers.iter().map(|&c| c.to_owned()).collect(),
            sensitive,
            set: false,
            values: vec!["interne".into()],
        }
    }

    /// Q-019-35: an axis of a mapping or of admitted values is a closed list carried by
    /// the workloads, neither sensitive nor `kube.namespace`.
    #[test]
    fn axis_checked() {
        let good = axis("enum", &[WORKLOAD], false);
        assert!(checked_axis("custom.exposure", Some(&good)).is_ok());
        for (key, axis, says) in [
            ("kube.namespace", Some(&good), "set by kube.sync alone"),
            ("custom.none", None, "no property custom.none"),
            (
                "custom.secret",
                Some(&axis("enum", &[WORKLOAD], true)),
                "sensitive",
            ),
            (
                "custom.note",
                Some(&axis("text", &[WORKLOAD], false)),
                "not a closed list",
            ),
            (
                "custom.zone",
                Some(&axis("enum", &["domain"], false)),
                "not carried by",
            ),
        ] {
            let refused = checked_axis(key, axis).expect_err(key);
            assert_eq!(refused.rule.id, "kube.operation.invalid");
            assert!(refused.message.contains(says), "{key}: {}", refused.message);
        }
        assert!(checked_value(&good, "custom.exposure", "interne", "value").is_ok());
        assert!(checked_value(&good, "custom.exposure", "dmz", "value").is_err());
    }
}
