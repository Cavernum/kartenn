//! The values of axes the server derives for a workload (spec 019, REQ-019-33, 34;
//! Q-019-33, 34, 37 to 41; design « Politique et flux », Labels; task 019-7d2). At each
//! `upsert` of a workload, `kube.sync` sets its axis `kube.namespace` (external
//! reference of the system `kube`, `<cluster id>/<name>`, labelled `<cluster slug>/<name>`)
//! and derives, from its tracked labels (Q-019-41), the values the mappings
//! (`kube.label_mapping`) give and the admitted values of its namespace
//! (`kube.admitted_values`) admit: assignments and removals of the core it adds to the
//! changeset (ADR 0026, clause 6), under their own rights, of the provenance
//! `module:kube#label:<keys>` (amendment 14). Two labels on one axis give the union of
//! their values to a set, nothing to a single value they disagree on (Q-019-40). The
//! derivation reads the provenance of the current assignments: it replaces or removes
//! only what the module set, never a value typed (`input`) nor a default value. A value
//! not admitted is ignored and warns (`kube.label.not_admitted`), labels in conflict warn
//! (`kube.label.conflict`), both rules of the final state; an assignment or a removal of
//! `kube.namespace` the author writes is refused (`kube.namespace.managed`).

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::rules::Violation;
use kartenn_service::ObjectKind;
use kartenn_service::module::CheckContext;
use kartenn_service::operations::{
    ModuleKind, Operation, RawInput, Ref, is_module_provenance, module_provenance,
};
use kartenn_service::port::StoreError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::module::{MODULE, NAMESPACE_AXIS, SYSTEM, WORKLOAD};
use crate::names::parse_labels;
use crate::pg::store;
use crate::rules::{LABEL_CONFLICT, LABEL_NOT_ADMITTED, NAMESPACE_MANAGED};

/// The motif of the provenance of `kube.namespace`, which no label gives.
const NAMESPACE_MOTIF: &str = "namespace";

/// A current assignment of a property of the workload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Current {
    /// The key of its property.
    key: String,
    /// Its provenance as stored.
    provenance: String,
    /// The keys of its living values of a closed list, one or a set, sorted; empty for a
    /// "no value" or another type.
    #[serde(default)]
    values: Vec<String>,
    /// The identifier and the label of its external reference.
    #[serde(default)]
    external: Option<String>,
    #[serde(default)]
    label: Option<String>,
}

/// An axis a label may give (a closed list carried by the workloads, neither sensitive
/// nor `kube.namespace`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Axis {
    /// Whether it takes a set of values.
    set: bool,
    /// The living values of its list: a value closed since is ignored.
    values: Vec<String>,
}

/// What [`prepare`] reads for the derivation of a workload an `upsert` writes, for the
/// plan, which reads no database.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Facts {
    /// The slug of the cluster: the label of the value of `kube.namespace`.
    #[serde(default)]
    slug: Option<String>,
    /// The labels of the living workload, which an `upsert` without `labels` keeps.
    #[serde(default)]
    stored: Option<BTreeMap<String, String>>,
    /// The tracked label keys: those of the module (`kube.tracked_label`) and those its
    /// cluster adds (019/X4); only they give a value (Q-019-41).
    #[serde(default)]
    tracked: BTreeSet<String>,
    /// The properties the derivation may write, by key: `kube.namespace` and the axes of
    /// the mappings.
    #[serde(default)]
    properties: BTreeMap<String, Uuid>,
    /// The living mappings of the tracked labels of the workload: (label key, label value,
    /// axis, value of the axis).
    #[serde(default)]
    mappings: Vec<(String, String, String, String)>,
    /// The axes of these mappings a label may give.
    #[serde(default)]
    axes: BTreeMap<String, Axis>,
    /// The values admitted in the namespace of the workload, by axis.
    #[serde(default)]
    admitted: BTreeMap<String, Vec<String>>,
    /// The current assignments of the workload of these properties, and those of the
    /// provenance of the module.
    #[serde(default)]
    current: Vec<Current>,
}

/// The labels of the body of an `upsert`, when it gives them and they are well formed.
fn given_labels(body: &Map<String, Value>) -> Option<BTreeMap<String, String>> {
    let raw: BTreeMap<String, String> = serde_json::from_value(body.get("labels")?.clone()).ok()?;
    let labels = parse_labels(raw.iter().map(|(k, v)| (k.as_str(), v.as_str()))).ok()?;
    Some(
        labels
            .into_iter()
            .map(|(k, v)| (k.as_str().to_owned(), v.as_str().to_owned()))
            .collect(),
    )
}

/// What the derivation reads of `cluster`: its slug and the tracked label keys, those of
/// the module and those it adds.
async fn of_cluster(
    conn: &mut PgConnection,
    cluster: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Facts, StoreError> {
    let mut facts = Facts::default();
    let row: Option<(String, Vec<String>)> = sqlx::query_as(
        "SELECT slug, tracked_label_keys FROM kube.cluster
          WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(cluster)
    .bind(at)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    if let Some((slug, keys)) = row {
        facts.slug = Some(slug);
        facts.tracked.extend(keys);
    }
    facts.tracked.extend(
        crate::settings::tracked(conn, at)
            .await?
            .into_iter()
            .map(|(key, _)| key),
    );
    Ok(facts)
}

/// Reads what the derivation of the workload of an `upsert` of `body` in `cluster` needs:
/// `namespace` is its living namespace, `workload` the living (or closed, opened again)
/// workload of its identity.
pub(crate) async fn prepare(
    conn: &mut PgConnection,
    body: &Map<String, Value>,
    cluster: Uuid,
    namespace: Option<Uuid>,
    workload: (Option<Uuid>, Option<Uuid>),
    at: Option<OffsetDateTime>,
) -> Result<Facts, StoreError> {
    let (existing, reopened) = workload;
    let mut facts = of_cluster(conn, cluster, at).await?;
    if let Some(workload) = existing {
        let stored: Option<sqlx::types::Json<BTreeMap<String, String>>> = sqlx::query_scalar(
            "SELECT labels FROM kube.workload
              WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
        )
        .bind(workload)
        .bind(at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        facts.stored = stored.map(|labels| labels.0);
    }
    let labels = given_labels(body)
        .or_else(|| facts.stored.clone())
        .unwrap_or_default();
    let (keys, values): (Vec<&str>, Vec<&str>) = labels
        .iter()
        .filter(|(k, _)| facts.tracked.contains(*k))
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .unzip();
    if !keys.is_empty() {
        facts.mappings = sqlx::query_as(
            "SELECT m.label_key, m.label_value, m.axis_key, m.axis_value
               FROM kube.label_mapping m
               JOIN unnest($1::text[], $2::text[]) AS l(k, v)
                 ON m.label_key = l.k AND m.label_value = l.v
              WHERE m.valid_period @> coalesce($3::timestamptz, now())
              ORDER BY 1, 3",
        )
        .bind(&keys)
        .bind(&values)
        .bind(at)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    }
    let wanted: BTreeSet<&str> = facts
        .mappings
        .iter()
        .map(|(_, _, axis, _)| axis.as_str())
        .collect();
    for axis in wanted {
        let declared = crate::settings::axis_of(conn, axis, at).await?;
        if let Ok(declared) = crate::settings::checked_axis(axis, declared.as_ref()) {
            facts.axes.insert(
                axis.to_owned(),
                Axis {
                    set: declared.set,
                    values: declared.values.clone(),
                },
            );
        }
    }
    if let Some(namespace) = namespace {
        let admitted: Vec<(String, Vec<String>)> = sqlx::query_as(
            "SELECT axis_key, axis_values FROM kube.admitted_values
              WHERE namespace_id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
        )
        .bind(namespace)
        .bind(at)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        facts.admitted = admitted.into_iter().collect();
    }
    let properties: Vec<String> = std::iter::once(NAMESPACE_AXIS.to_owned())
        .chain(facts.axes.keys().cloned())
        .collect();
    let declared: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT key, id FROM public.property_declaration
          WHERE key = ANY($1) AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(&properties)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    facts.properties = declared.into_iter().collect();
    if let Some(workload) = existing.or(reopened) {
        current(conn, workload, &properties, at, &mut facts).await?;
    }
    Ok(facts)
}

/// The current assignments of `workload` of `properties`, and those of the provenance of
/// the module, with their properties, into `facts`: a value of a closed list, one
/// (`v_uuid`) or a set (`v_uuids`), by the keys of its living values.
async fn current(
    conn: &mut PgConnection,
    workload: Uuid,
    properties: &[String],
    at: Option<OffsetDateTime>,
    facts: &mut Facts,
) -> Result<(), StoreError> {
    type Row = (
        String,
        Uuid,
        String,
        Vec<String>,
        Option<String>,
        Option<String>,
    );
    let current: Vec<Row> = sqlx::query_as(
        "SELECT d.key, a.property_id, a.provenance,
                ARRAY(SELECT v.key FROM public.property_enum_value v
                       WHERE (v.id = a.v_uuid OR v.id = ANY(a.v_uuids))
                         AND v.valid_period @> coalesce($3::timestamptz, now())
                       ORDER BY v.key),
                a.v_json ->> 'id', a.v_json ->> 'label'
           FROM public.property_assignment a
           JOIN public.property_declaration d
             ON d.id = a.property_id
            AND d.valid_period @> coalesce($3::timestamptz, now())
          WHERE a.object_id = $1
            AND a.valid_period @> coalesce($3::timestamptz, now())
            AND (d.key = ANY($2) OR a.provenance LIKE 'module:kube#%')
          ORDER BY d.key",
    )
    .bind(workload)
    .bind(properties)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    // The properties of the assignments the module set, which it may remove.
    facts.properties.extend(
        current
            .iter()
            .map(|(key, property, ..)| (key.clone(), *property)),
    );
    facts.current = current
        .into_iter()
        .map(|(key, _, provenance, mut values, external, label)| {
            // Sorted as the plan sorts the values it wants, whatever the collation.
            values.sort_unstable();
            values.dedup();
            Current {
                key,
                provenance,
                values,
                external,
                label,
            }
        })
        .collect();
    Ok(())
}

/// An operation of the core the derivation adds, with the provenance of its value.
pub(crate) type Derived = (Operation, Option<String>);

/// The values of the axes the tracked `labels` give on `facts`, admitted and living: by
/// axis, each value with the label keys that give it.
fn given<'a>(
    facts: &'a Facts,
    labels: Option<&BTreeMap<String, String>>,
) -> BTreeMap<&'a str, BTreeMap<&'a str, BTreeSet<&'a str>>> {
    let mut given: BTreeMap<&str, BTreeMap<&str, BTreeSet<&str>>> = BTreeMap::new();
    for (label_key, label_value, axis, value) in &facts.mappings {
        let carried = facts.tracked.contains(label_key)
            && labels.and_then(|l| l.get(label_key)) == Some(label_value);
        let living = facts
            .axes
            .get(axis)
            .is_some_and(|a| a.values.contains(value));
        let admitted = facts
            .admitted
            .get(axis)
            .is_some_and(|values| values.contains(value));
        if carried && living && admitted && axis != NAMESPACE_AXIS {
            given
                .entry(axis.as_str())
                .or_default()
                .entry(value.as_str())
                .or_default()
                .insert(label_key.as_str());
        }
    }
    given
}

/// The assignments and removals of the core the `upsert` of `workload`, of `labels`, in
/// the namespace `namespace` (by name) of `cluster`, adds to the changeset, on `facts`;
/// `labels` are those it writes, else those `facts` read on the workload.
pub(crate) fn derive(
    facts: &Facts,
    kind: ModuleKind,
    workload: Uuid,
    cluster: Uuid,
    namespace: &str,
    labels: Option<&BTreeMap<String, String>>,
) -> Vec<Derived> {
    let object = Ref::Id(workload);
    let kind = ObjectKind::Module(kind);
    let current = |key: &str| facts.current.iter().find(|c| c.key == key);
    let mut derived = Vec::new();
    // The namespace of the workload, set by the server alone (Q-019-37 to 39): whatever
    // provenance a value set before has, the server replaces it.
    if let (Some(property), Some(slug)) = (facts.properties.get(NAMESPACE_AXIS), &facts.slug) {
        let id = format!("{cluster}/{namespace}");
        let label = format!("{slug}/{namespace}");
        let provenance = module_provenance(MODULE, NAMESPACE_MOTIF);
        let unchanged = current(NAMESPACE_AXIS).is_some_and(|c| {
            c.provenance == provenance
                && c.external.as_deref() == Some(id.as_str())
                && c.label.as_deref() == Some(label.as_str())
        });
        if !unchanged {
            derived.push((
                Operation::AssignProperty {
                    object,
                    kind,
                    property: Ref::Id(*property),
                    values: vec![RawInput::External {
                        system: SYSTEM.to_owned(),
                        id,
                        label: Some(label),
                    }],
                },
                Some(provenance),
            ));
        }
    }
    // What each axis receives (Q-019-40): the union of the values of its labels for a set,
    // the value they agree on for a single value, nothing when they disagree.
    let labels = labels.or(facts.stored.as_ref());
    let mut wanted: BTreeMap<&str, (Vec<&str>, BTreeSet<&str>)> = BTreeMap::new();
    for (axis, values) in given(facts, labels) {
        let set = facts.axes.get(axis).is_some_and(|a| a.set);
        if set || values.len() == 1 {
            let keys = values.values().flatten().copied().collect();
            wanted.insert(axis, (values.into_keys().collect(), keys));
        }
    }
    let derived_by_module =
        |c: &Current| is_module_provenance(&c.provenance, MODULE) && c.key != NAMESPACE_AXIS;
    for (axis, (values, keys)) in &wanted {
        let Some(property) = facts.properties.get(*axis) else {
            continue;
        };
        let keys: Vec<&str> = keys.iter().copied().collect();
        let provenance = module_provenance(MODULE, &format!("label:{}", keys.join(",")));
        let replace = match current(axis) {
            None => true,
            // Typed or a default: it wins, neither replaced nor removed (Q-019-34).
            Some(c) if !derived_by_module(c) => false,
            Some(c) => c.provenance != provenance || c.values != *values,
        };
        if replace {
            derived.push((
                Operation::AssignProperty {
                    object,
                    kind,
                    property: Ref::Id(*property),
                    values: values
                        .iter()
                        .map(|v| RawInput::Text((*v).to_owned()))
                        .collect(),
                },
                Some(provenance),
            ));
        }
    }
    // What the module set that no label justifies any more: a label gone or untracked, a
    // mapping or admitted values closed or changed, labels now in conflict.
    for c in facts
        .current
        .iter()
        .filter(|c| derived_by_module(c) && !wanted.contains_key(c.key.as_str()))
    {
        if let Some(property) = facts.properties.get(&c.key) {
            derived.push((
                Operation::UnassignProperty {
                    object,
                    kind,
                    property: Ref::Id(*property),
                },
                None,
            ));
        }
    }
    derived
}

/// The object (`None` for a local name) and the property of `operation`, an assignment,
/// a "no value", a removal or the restoration of an assignment (an inverse changeset
/// rewrites it) written by the author, alone or within a period of its own; `None` for
/// another operation, or a property created in the changeset.
fn assigned(operation: &Operation) -> Option<(Option<Uuid>, Uuid)> {
    let id = |r: &Ref| match r {
        Ref::Id(id) => Some(*id),
        Ref::Local(_) => None,
    };
    match operation {
        Operation::Within { operation, .. } => assigned(operation),
        Operation::AssignProperty {
            object, property, ..
        }
        | Operation::ClearProperty {
            object, property, ..
        }
        | Operation::UnassignProperty {
            object, property, ..
        } => Some((id(object), id(property)?)),
        Operation::Restore {
            id: object,
            property: Some(property),
            ..
        } => Some((id(object), *property)),
        _ => None,
    }
}

/// `kube.namespace.managed` (Q-019-39): an assignment, a removal or a restoration of
/// `kube.namespace` the author writes, the operations `kube.sync` plans not being among
/// the author's.
async fn managed(
    conn: &mut PgConnection,
    context: &CheckContext<'_>,
    found: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let properties: Vec<(usize, Option<Uuid>, Uuid)> = context
        .operations
        .iter()
        .enumerate()
        .filter_map(|(i, o)| assigned(o).map(|(object, property)| (i, object, property)))
        .collect();
    if properties.is_empty() {
        return Ok(());
    }
    let managed: Vec<Uuid> =
        sqlx::query_scalar("SELECT DISTINCT id FROM public.property_declaration WHERE key = $1")
            .bind(NAMESPACE_AXIS)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
    for (i, object, _) in properties.iter().filter(|(.., p)| managed.contains(p)) {
        found.push(
            Violation::new(
                &NAMESPACE_MANAGED,
                format!(
                    "the operation at position {i} writes {NAMESPACE_AXIS}, which kube.sync alone sets on a workload from its namespace"
                ),
            )
            .with_subjects(*object),
        );
    }
    Ok(())
}

/// A tracked label of a workload with a living mapping to a living value of a valid axis:
/// workload, label key, label value, axis, value, whether the axis takes a set, whether
/// its namespace admits the value.
type Given = (Uuid, String, String, String, String, bool, bool);

/// The rules of the derivation on the final state: `kube.namespace.managed`; a workload
/// of `workloads` whose tracked label gives, by a mapping, a value its namespace does not
/// admit warns (`kube.label.not_admitted`), as one whose tracked labels give different
/// admitted values to a single-valued axis (`kube.label.conflict`, Q-019-40); both
/// evaluated anew, their anomalies close once the cause is gone.
pub(crate) async fn check(
    conn: &mut PgConnection,
    context: &CheckContext<'_>,
    workloads: &[Uuid],
    found: &mut Vec<Violation>,
) -> Result<Vec<(Uuid, &'static str)>, StoreError> {
    managed(conn, context, found).await?;
    if workloads.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<Given> = sqlx::query_as(
        "SELECT w.id, m.label_key, m.label_value, m.axis_key, m.axis_value,
                d.cardinality = 'set',
                EXISTS (SELECT 1 FROM kube.admitted_values a
                         WHERE a.namespace_id = w.namespace_id
                           AND a.axis_key = m.axis_key
                           AND m.axis_value = ANY(a.axis_values)
                           AND a.valid_period @> now())
           FROM kube.workload w
           JOIN kube.namespace n ON n.id = w.namespace_id AND n.valid_period @> now()
           JOIN kube.cluster c ON c.id = n.cluster_id AND c.valid_period @> now()
           JOIN kube.label_mapping m
             ON w.labels ->> m.label_key = m.label_value AND m.valid_period @> now()
           JOIN public.property_declaration d
             ON d.key = m.axis_key AND d.valid_period @> now()
          WHERE w.id = ANY($1) AND w.valid_period @> now()
            AND (m.label_key = ANY(c.tracked_label_keys)
                 OR EXISTS (SELECT 1 FROM kube.tracked_label t
                             WHERE t.label_key = m.label_key AND t.valid_period @> now()))
            AND m.axis_key <> $2
            AND d.value_type = 'enum' AND NOT d.sensitive AND $3 = ANY(d.carriers)
            AND EXISTS (SELECT 1 FROM public.property_enum_value v
                         WHERE v.property_id = public.kartenn_values_owner(d.id)
                           AND v.key = m.axis_value AND v.valid_period @> now())
          ORDER BY 1, 4, 5, 2",
    )
    .bind(workloads)
    .bind(NAMESPACE_AXIS)
    .bind(WORKLOAD)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut refused: BTreeMap<Uuid, Vec<String>> = BTreeMap::new();
    // By workload and single-valued axis, the admitted values and the labels giving them.
    let mut single: BTreeMap<(Uuid, &str), BTreeMap<&str, Vec<String>>> = BTreeMap::new();
    for (workload, label_key, label_value, axis, value, set, admitted) in &rows {
        let given = format!("{label_key}={label_value} gives {axis}: {value}");
        if !admitted {
            refused.entry(*workload).or_default().push(given);
        } else if !set {
            single
                .entry((*workload, axis.as_str()))
                .or_default()
                .entry(value.as_str())
                .or_default()
                .push(given);
        }
    }
    for (workload, labels) in refused {
        found.push(
            Violation::new(
                &LABEL_NOT_ADMITTED,
                format!(
                    "the namespace of the workload does not admit what its labels give, ignored: {}",
                    labels.join("; ")
                ),
            )
            .with_subjects([workload]),
        );
    }
    let mut conflicts: BTreeMap<Uuid, Vec<String>> = BTreeMap::new();
    for ((workload, axis), values) in single.into_iter().filter(|(_, v)| v.len() > 1) {
        let labels: Vec<String> = values.into_values().flatten().collect();
        conflicts
            .entry(workload)
            .or_default()
            .push(format!("{axis} ({})", labels.join(", ")));
    }
    for (workload, axes) in conflicts {
        found.push(
            Violation::new(
                &LABEL_CONFLICT,
                format!(
                    "the labels of the workload give different values to a single-valued axis, none set: {}",
                    axes.join("; ")
                ),
            )
            .with_subjects([workload]),
        );
    }
    Ok(workloads
        .iter()
        .flat_map(|w| [(*w, LABEL_NOT_ADMITTED.id), (*w, LABEL_CONFLICT.id)])
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPOSURE: &str = "custom.exposure";
    const ZONES: &str = "custom.zones";

    fn facts() -> Facts {
        let both = || vec!["externe".to_owned(), "interne".to_owned()];
        Facts {
            slug: Some("prod".into()),
            stored: None,
            tracked: BTreeSet::from(["app".into(), "tier".into()]),
            properties: BTreeMap::from([
                (NAMESPACE_AXIS.into(), Uuid::from_u128(1)),
                (EXPOSURE.into(), Uuid::from_u128(2)),
                ("custom.tier".into(), Uuid::from_u128(3)),
                (ZONES.into(), Uuid::from_u128(4)),
            ]),
            mappings: vec![
                ("app".into(), "db".into(), EXPOSURE.into(), "interne".into()),
                ("app".into(), "db".into(), ZONES.into(), "interne".into()),
                (
                    "tier".into(),
                    "db".into(),
                    EXPOSURE.into(),
                    "externe".into(),
                ),
                ("tier".into(), "db".into(), ZONES.into(), "externe".into()),
            ],
            axes: BTreeMap::from([
                (
                    EXPOSURE.into(),
                    Axis {
                        set: false,
                        values: both(),
                    },
                ),
                (
                    ZONES.into(),
                    Axis {
                        set: true,
                        values: both(),
                    },
                ),
            ]),
            admitted: BTreeMap::from([(EXPOSURE.into(), both()), (ZONES.into(), both())]),
            current: Vec::new(),
        }
    }

    fn kind() -> ModuleKind {
        ModuleKind::parse("kube.workload").unwrap()
    }

    fn derived(facts: &Facts, labels: &[(&str, &str)]) -> Vec<Derived> {
        let labels: BTreeMap<String, String> = labels
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        derive(
            facts,
            kind(),
            Uuid::from_u128(9),
            Uuid::nil(),
            "payments",
            Some(&labels),
        )
    }

    fn provenances(derived: &[Derived]) -> Vec<Option<&str>> {
        derived.iter().map(|(_, p)| p.as_deref()).collect()
    }

    fn texts(values: &[&str]) -> Vec<RawInput> {
        values.iter().map(|v| RawInput::Text((*v).into())).collect()
    }

    /// Two labels on one axis (Q-019-40): the union for a set, nothing for a single value
    /// they disagree on; the namespace is set once.
    #[test]
    fn union_or_nothing() {
        let derived = derived(&facts(), &[("app", "db"), ("tier", "db")]);
        assert_eq!(
            provenances(&derived),
            [
                Some("module:kube#namespace"),
                Some("module:kube#label:app,tier")
            ]
        );
        assert!(matches!(
            &derived[1].0,
            Operation::AssignProperty { property: Ref::Id(p), values, .. }
                if *p == Uuid::from_u128(4) && *values == texts(&["externe", "interne"])
        ));
    }

    /// A typed value wins; a value the module set and no label justifies is removed, as
    /// one its labels now disagree on; a value unchanged is not written again.
    #[test]
    fn typed_wins_derived_removed() {
        let mut facts = facts();
        let derived_value = |key: &str, provenance: &str, values: &[&str]| Current {
            key: key.into(),
            provenance: provenance.into(),
            values: values.iter().map(|&v| v.to_owned()).collect(),
            external: None,
            label: None,
        };
        facts.current = vec![
            Current {
                key: NAMESPACE_AXIS.into(),
                provenance: "module:kube#namespace".into(),
                values: Vec::new(),
                external: Some(format!("{}/payments", Uuid::nil())),
                label: Some("prod/payments".into()),
            },
            derived_value(EXPOSURE, "module:kube#label:app", &["interne"]),
            derived_value("custom.tier", "module:kube#label:tier", &["front"]),
            derived_value(ZONES, "module:kube#label:app,tier", &["externe", "interne"]),
        ];
        let derived = derived(&facts, &[("app", "db"), ("tier", "db")]);
        let removed: Vec<Uuid> = derived
            .iter()
            .map(|(o, _)| match o {
                Operation::UnassignProperty {
                    property: Ref::Id(p),
                    ..
                } => *p,
                other => panic!("not a removal: {other:?}"),
            })
            .collect();
        assert_eq!(removed, [Uuid::from_u128(2), Uuid::from_u128(3)]);
        // Typed: neither replaced nor removed.
        facts.current[1].provenance = "input".into();
        let derived = self::derived(&facts, &[("app", "db")]);
        assert_eq!(derived.len(), 2, "{derived:?}");
    }

    /// Not admitted, a value closed since or a label not tracked: ignored.
    #[test]
    fn ignored() {
        let only_namespace = [Some("module:kube#namespace")];
        let mut facts = facts();
        facts.admitted.clear();
        assert_eq!(
            provenances(&derived(&facts, &[("app", "db")])),
            only_namespace
        );
        let mut facts = self::facts();
        for axis in facts.axes.values_mut() {
            axis.values = vec!["externe".into()];
        }
        assert_eq!(
            provenances(&derived(&facts, &[("app", "db")])),
            only_namespace
        );
        let mut facts = self::facts();
        facts.tracked.remove("app");
        assert_eq!(
            provenances(&derived(&facts, &[("app", "db")])),
            only_namespace
        );
    }
}
