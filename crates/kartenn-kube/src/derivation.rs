//! The values of axes the server derives for a workload (spec 019, REQ-019-33, 34;
//! Q-019-33, 34, 37 to 39; design « Politique et flux », Labels; task 019-7d2). At each
//! `upsert` of a workload, `kube.sync` sets its axis `kube.namespace` (external
//! reference of the system `kube`, `<cluster id>/<name>`, labelled `<cluster slug>/<name>`)
//! and derives, from its labels, the values the mappings (`kube.label_mapping`) give and
//! the admitted values of its namespace (`kube.admitted_values`) admit: assignments and
//! removals of the core it adds to the changeset (ADR 0026, clause 6), under their own
//! rights, of the provenance `module:kube#label:<key>` (amendment 14). The derivation
//! reads the provenance of the current assignments: it replaces or removes only what the
//! module set, never a value typed (`input`) nor a default value. A value not admitted is
//! ignored and warns (`kube.label.not_admitted`, a rule of the final state); an
//! assignment or a removal of `kube.namespace` the author writes is refused
//! (`kube.namespace.managed`).

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

use crate::module::{NAMESPACE_AXIS, SYSTEM};
use crate::names::parse_labels;
use crate::pg::store;
use crate::rules::{LABEL_NOT_ADMITTED, NAMESPACE_MANAGED};

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
    /// The key of its value of a closed list, or the identifier of its external reference;
    /// `None` for a "no value" or a value closed since.
    value: Option<String>,
    /// The label of its external reference.
    #[serde(default)]
    label: Option<String>,
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
    /// The properties the derivation may write, by key: `kube.namespace` and the axes of
    /// the mappings.
    #[serde(default)]
    properties: BTreeMap<String, Uuid>,
    /// The living mappings of the labels of the workload: (label key, label value, axis,
    /// value of the axis).
    #[serde(default)]
    mappings: Vec<(String, String, String, String)>,
    /// The axes of these mappings a label may give (a closed list carried by the workloads,
    /// neither sensitive nor `kube.namespace`), with the living values of their list: a
    /// value closed since is ignored.
    #[serde(default)]
    axes: BTreeMap<String, Vec<String>>,
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
    let mut facts = Facts {
        slug: sqlx::query_scalar(
            "SELECT slug FROM kube.cluster
              WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
        )
        .bind(cluster)
        .bind(at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| store(&e))?,
        ..Facts::default()
    };
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
    let (keys, values): (Vec<&str>, Vec<&str>) =
        labels.iter().map(|(k, v)| (k.as_str(), v.as_str())).unzip();
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
            facts.axes.insert(axis.to_owned(), declared.values.clone());
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
/// the module, with their properties, into `facts`.
async fn current(
    conn: &mut PgConnection,
    workload: Uuid,
    properties: &[String],
    at: Option<OffsetDateTime>,
    facts: &mut Facts,
) -> Result<(), StoreError> {
    type Row = (String, Uuid, String, Option<String>, Option<String>);
    let current: Vec<Row> = sqlx::query_as(
        "SELECT d.key, a.property_id, a.provenance,
                coalesce((SELECT v.key FROM public.property_enum_value v
                           WHERE v.id = a.v_uuid
                             AND v.valid_period @> coalesce($3::timestamptz, now())),
                         a.v_json ->> 'id'),
                a.v_json ->> 'label'
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
        .map(|(key, _, provenance, value, label)| Current {
            key,
            provenance,
            value,
            label,
        })
        .collect();
    Ok(())
}

/// An operation of the core the derivation adds, with the provenance of its value.
pub(crate) type Derived = (Operation, Option<String>);

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
        let provenance = module_provenance(SYSTEM, NAMESPACE_MOTIF);
        let unchanged = current(NAMESPACE_AXIS).is_some_and(|c| {
            c.provenance == provenance
                && c.value.as_deref() == Some(id.as_str())
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
    // The value each axis receives from the labels: admitted in the namespace, of the
    // smallest label key that gives one.
    let labels = labels.or(facts.stored.as_ref());
    let mut wanted: BTreeMap<&str, (&str, &str)> = BTreeMap::new();
    for (label_key, label_value, axis, value) in &facts.mappings {
        let carried = labels.and_then(|l| l.get(label_key)) == Some(label_value);
        let living = facts
            .axes
            .get(axis)
            .is_some_and(|values| values.contains(value));
        let admitted = facts
            .admitted
            .get(axis)
            .is_some_and(|values| values.contains(value));
        if carried && living && admitted && axis != NAMESPACE_AXIS {
            wanted
                .entry(axis.as_str())
                .or_insert((label_key.as_str(), value.as_str()));
        }
    }
    let derived_by_module =
        |c: &Current| is_module_provenance(&c.provenance, SYSTEM) && c.key != NAMESPACE_AXIS;
    for (axis, (label_key, value)) in &wanted {
        let Some(property) = facts.properties.get(*axis) else {
            continue;
        };
        let provenance = module_provenance(SYSTEM, &format!("label:{label_key}"));
        let replace = match current(axis) {
            None => true,
            // Typed or a default: it wins, neither replaced nor removed (Q-019-34).
            Some(c) if !derived_by_module(c) => false,
            Some(c) => c.provenance != provenance || c.value.as_deref() != Some(*value),
        };
        if replace {
            derived.push((
                Operation::AssignProperty {
                    object,
                    kind,
                    property: Ref::Id(*property),
                    values: vec![RawInput::Text((*value).to_owned())],
                },
                Some(provenance),
            ));
        }
    }
    // What the module set that no label justifies any more: a label gone, a mapping or
    // admitted values closed or changed.
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

/// The object and the property of `operation`, an assignment, a "no value" or a removal
/// written by the author, alone or within a period of its own.
fn assigned(operation: &Operation) -> Option<(&Ref, &Ref)> {
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
        } => Some((object, property)),
        _ => None,
    }
}

/// The rules of the derivation on the final state: an assignment or a removal of
/// `kube.namespace` the author writes is refused (`kube.namespace.managed`, Q-019-39), the
/// operations `kube.sync` plans not being among the author's; a workload of `workloads`
/// that carries a label whose mapping gives a value its namespace does not admit warns
/// (`kube.label.not_admitted`), evaluated anew: its anomaly closes once the value is
/// admitted, the label or the mapping gone.
pub(crate) async fn check(
    conn: &mut PgConnection,
    context: &CheckContext<'_>,
    workloads: &[Uuid],
    found: &mut Vec<Violation>,
) -> Result<Vec<(Uuid, &'static str)>, StoreError> {
    let properties: Vec<(usize, Option<Uuid>, Uuid)> = context
        .operations
        .iter()
        .enumerate()
        .filter_map(|(i, o)| match assigned(o) {
            Some((object, Ref::Id(property))) => {
                let object = match object {
                    Ref::Id(id) => Some(*id),
                    Ref::Local(_) => None,
                };
                Some((i, object, *property))
            }
            _ => None,
        })
        .collect();
    if !properties.is_empty() {
        let managed: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT id FROM public.property_declaration WHERE key = $1",
        )
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
    }
    if workloads.is_empty() {
        return Ok(Vec::new());
    }
    let refused: Vec<(Uuid, String, String, String, String)> = sqlx::query_as(
        "SELECT w.id, m.label_key, m.label_value, m.axis_key, m.axis_value
           FROM kube.workload w
           JOIN kube.label_mapping m
             ON w.labels ->> m.label_key = m.label_value AND m.valid_period @> now()
           JOIN public.property_declaration d
             ON d.key = m.axis_key AND d.valid_period @> now()
          WHERE w.id = ANY($1) AND w.valid_period @> now()
            AND m.axis_key <> $2
            AND d.value_type = 'enum' AND NOT d.sensitive AND $3 = ANY(d.carriers)
            AND EXISTS (SELECT 1 FROM public.property_enum_value v
                         WHERE v.property_id = public.kartenn_values_owner(d.id)
                           AND v.key = m.axis_value AND v.valid_period @> now())
            AND NOT EXISTS (SELECT 1 FROM kube.admitted_values a
                             WHERE a.namespace_id = w.namespace_id
                               AND a.axis_key = m.axis_key
                               AND m.axis_value = ANY(a.axis_values)
                               AND a.valid_period @> now())
          ORDER BY 1, 2, 4",
    )
    .bind(workloads)
    .bind(NAMESPACE_AXIS)
    .bind(crate::module::WORKLOAD)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut by_workload: BTreeMap<Uuid, Vec<String>> = BTreeMap::new();
    for (workload, label_key, label_value, axis, value) in refused {
        by_workload
            .entry(workload)
            .or_default()
            .push(format!("{label_key}={label_value} gives {axis}: {value}"));
    }
    for (workload, labels) in by_workload {
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
    Ok(workloads
        .iter()
        .map(|w| (*w, LABEL_NOT_ADMITTED.id))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            slug: Some("prod".into()),
            stored: None,
            properties: BTreeMap::from([
                (NAMESPACE_AXIS.into(), Uuid::from_u128(1)),
                ("custom.exposure".into(), Uuid::from_u128(2)),
                ("custom.tier".into(), Uuid::from_u128(3)),
            ]),
            mappings: vec![
                (
                    "app".into(),
                    "db".into(),
                    "custom.exposure".into(),
                    "interne".into(),
                ),
                (
                    "tier".into(),
                    "db".into(),
                    "custom.exposure".into(),
                    "externe".into(),
                ),
            ],
            axes: BTreeMap::from([(
                "custom.exposure".into(),
                vec!["externe".into(), "interne".into()],
            )]),
            admitted: BTreeMap::from([(
                "custom.exposure".into(),
                vec!["externe".into(), "interne".into()],
            )]),
            current: Vec::new(),
        }
    }

    fn kind() -> ModuleKind {
        ModuleKind::parse("kube.workload").unwrap()
    }

    fn provenances(derived: &[Derived]) -> Vec<Option<&str>> {
        derived.iter().map(|(_, p)| p.as_deref()).collect()
    }

    /// Two labels give the same axis: the smallest key wins; the namespace is set once.
    #[test]
    fn smallest_label_key_wins() {
        let labels = BTreeMap::from([
            ("app".to_owned(), "db".to_owned()),
            ("tier".to_owned(), "db".to_owned()),
        ]);
        let derived = derive(
            &facts(),
            kind(),
            Uuid::from_u128(9),
            Uuid::nil(),
            "payments",
            Some(&labels),
        );
        assert_eq!(
            provenances(&derived),
            [Some("module:kube#namespace"), Some("module:kube#label:app")]
        );
        assert!(matches!(
            &derived[1].0,
            Operation::AssignProperty { values, .. } if values == &[RawInput::Text("interne".into())]
        ));
    }

    /// A typed value wins; a value the module set and no label justifies is removed; a
    /// value unchanged is not written again.
    #[test]
    fn typed_wins_derived_removed() {
        let mut facts = facts();
        let namespace = Current {
            key: NAMESPACE_AXIS.into(),
            provenance: "module:kube#namespace".into(),
            value: Some(format!("{}/payments", Uuid::nil())),
            label: Some("prod/payments".into()),
        };
        facts.current = vec![
            namespace,
            Current {
                key: "custom.exposure".into(),
                provenance: "input".into(),
                value: Some("externe".into()),
                label: None,
            },
            Current {
                key: "custom.tier".into(),
                provenance: "module:kube#label:tier".into(),
                value: Some("front".into()),
                label: None,
            },
        ];
        let labels = BTreeMap::from([("app".to_owned(), "db".to_owned())]);
        let derived = derive(
            &facts,
            kind(),
            Uuid::from_u128(9),
            Uuid::nil(),
            "payments",
            Some(&labels),
        );
        assert_eq!(derived.len(), 1, "{derived:?}");
        assert!(matches!(
            &derived[0].0,
            Operation::UnassignProperty { property: Ref::Id(p), .. } if *p == Uuid::from_u128(3)
        ));
    }

    /// Not admitted, or a value closed since: ignored.
    #[test]
    fn not_admitted_ignored() {
        let mut facts = facts();
        facts.admitted.clear();
        let labels = BTreeMap::from([("app".to_owned(), "db".to_owned())]);
        let derived = derive(
            &facts,
            kind(),
            Uuid::from_u128(9),
            Uuid::nil(),
            "payments",
            Some(&labels),
        );
        assert_eq!(provenances(&derived), [Some("module:kube#namespace")]);
        let mut facts = self::facts();
        facts
            .axes
            .insert("custom.exposure".into(), vec!["externe".into()]);
        let derived = derive(
            &facts,
            kind(),
            Uuid::from_u128(9),
            Uuid::nil(),
            "payments",
            Some(&labels),
        );
        assert_eq!(provenances(&derived), [Some("module:kube#namespace")]);
    }
}
