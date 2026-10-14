//! Assignments and the incremental maintenance of origins along the addressing tree
//! (spec 003, REQ-003-4, 8, 11 to 13, 15; task 003-8; ADR 0016, P9).
//!
//! An origin row exists only where an object has an origin: an assignment of its own or
//! of an ancestor, explicit "no value" included. After the writes of a changeset, the
//! objects whose origin may have changed are recomputed by the storage adapter, which
//! writes only the segments that differ.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use kartenn_core::inheritance::Assigned;
use kartenn_core::period::Period;
use kartenn_core::properties::{Declaration, TypeSpec};
use kartenn_core::rules::{self, Violation};
use kartenn_core::values::{Raw, ValueContext, parse_value};
use uuid::Uuid;

use crate::operations::{ObjectKind, Operation, Ref};
use crate::pipeline::Planned;
use crate::port::{DiffCount, StoreError, WriteError, WriteTx};

/// Plans an assignment operation; nothing to check without the database.
pub(crate) fn plan(op: &Operation, resolve: impl Fn(Ref) -> Uuid) -> Planned {
    match op {
        Operation::AssignProperty {
            object,
            kind,
            property,
            values,
        } => Planned::Assign {
            object: resolve(*object),
            kind: *kind,
            property: resolve(*property),
            values: Some(
                values
                    .iter()
                    .map(crate::operations::RawInput::to_raw)
                    .collect(),
            ),
        },
        Operation::ClearProperty {
            object,
            kind,
            property,
        } => Planned::Assign {
            object: resolve(*object),
            kind: *kind,
            property: resolve(*property),
            values: None,
        },
        Operation::UnassignProperty {
            object,
            kind,
            property,
        } => Planned::Unassign {
            object: resolve(*object),
            kind: *kind,
            property: resolve(*property),
        },
        other => unreachable!("not an assignment operation: {other:?}"),
    }
}

/// Facts a value type needs, read beforehand from storage.
struct Facts {
    enum_values: HashMap<String, Uuid>,
    kinds: HashMap<Uuid, &'static str>,
}

impl ValueContext for Facts {
    fn enum_value(&self, key: &str) -> Option<Uuid> {
        self.enum_values.get(key).copied()
    }

    fn object_kind(&self, id: Uuid) -> Option<&'static str> {
        self.kinds.get(&id).copied()
    }
}

fn invalid_value(object: Uuid, message: String) -> WriteError {
    WriteError::Rule(Violation::new(&rules::PROP_INVALID_VALUE, message).with_subjects([object]))
}

/// The declaration of a property that `object` of `kind` may carry over `period`.
async fn carried<T: WriteTx>(
    tx: &mut T,
    object: Uuid,
    kind: ObjectKind,
    property: Uuid,
    period: Option<Period>,
) -> Result<Declaration, WriteError> {
    let Some(declaration) = tx.declaration(property, period).await? else {
        return Err(WriteError::NoVersion(property));
    };
    if !declaration.carriers.contains(kind.name()) {
        return Err(WriteError::Rule(
            Violation::new(
                &rules::PROP_NOT_CARRIER,
                format!("{} is not carried by a {}", declaration.key, kind.name()),
            )
            .with_subjects([object, property]),
        ));
    }
    if tx.object_kind_covering(object, period).await? != Some(kind.name()) {
        return Err(WriteError::NoVersion(object));
    }
    Ok(declaration)
}

/// Writes an assignment, or explicit "no value" when `values` is `None` (REQ-003-4, 8,
/// 12, 13).
pub(crate) async fn assign<T: WriteTx>(
    tx: &mut T,
    object: Uuid,
    kind: ObjectKind,
    property: Uuid,
    values: Option<&[Raw]>,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let declaration = carried(tx, object, kind, property, period).await?;
    let assigned = match values {
        None => Assigned::Cleared,
        Some(raws) => {
            let ty = declaration
                .spec
                .value_type()
                .map_err(|e| invalid_value(object, format!("{}: {e}", declaration.key)))?;
            let texts: Vec<String> = raws
                .iter()
                .filter_map(|r| match r {
                    Raw::Text(t) => Some(t.clone()),
                    _ => None,
                })
                .collect();
            let mut facts = Facts {
                enum_values: HashMap::new(),
                kinds: HashMap::new(),
            };
            match declaration.spec {
                TypeSpec::Enum => {
                    facts.enum_values = tx.enum_values_covering(property, &texts, period).await?;
                }
                TypeSpec::Ref { .. } => {
                    for id in texts.iter().filter_map(|t| t.parse::<Uuid>().ok()) {
                        if let Some(k) = tx.object_kind_covering(id, period).await? {
                            facts.kinds.insert(id, k);
                        }
                    }
                }
                _ => {}
            }
            let value = parse_value(ty.as_ref(), declaration.cardinality, raws, &facts)
                .map_err(|e| invalid_value(object, format!("{}: {e}", declaration.key)))?;
            Assigned::Value(value)
        }
    };
    tx.assign(object, kind.name(), property, &assigned, period, changeset)
        .await
}

/// Removes an assignment over the period; the object inherits again.
pub(crate) async fn unassign<T: WriteTx>(
    tx: &mut T,
    object: Uuid,
    kind: ObjectKind,
    property: Uuid,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    carried(tx, object, kind, property, period).await?;
    Ok(tx
        .unassign(object, Some(property), period, changeset)
        .await?)
}

/// Brings the materialised origins in line with the writes of the changeset (the four
/// maintenance rules of the 003 design): new objects, assignments written or removed,
/// closed objects and properties, declarations whose carriers or tree changed. Returns
/// the number of objects whose origin changed, per property.
pub(crate) async fn maintain<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    period: Option<Period>,
    changeset: Uuid,
) -> Result<BTreeMap<Uuid, Vec<Uuid>>, StoreError> {
    let heritable: HashMap<Uuid, Vec<String>> =
        tx.addressing_heritable(period).await?.into_iter().collect();
    let mut targets: HashMap<Uuid, BTreeSet<Uuid>> = HashMap::new();
    let mut everything: BTreeSet<Uuid> = BTreeSet::new();
    let add_all = |targets: &mut HashMap<Uuid, BTreeSet<Uuid>>, objects: &[Uuid]| {
        for p in heritable.keys() {
            targets.entry(*p).or_default().extend(objects);
        }
    };
    for p in planned {
        match p {
            Planned::Domain { id, .. } => add_all(&mut targets, &[*id]),
            Planned::Prefix { object, .. } => add_all(&mut targets, &[object.id]),
            Planned::Range { object, .. } => add_all(&mut targets, &[object.id]),
            Planned::Address { object, .. } => add_all(&mut targets, &[object.id]),
            // Rules 2 and 3: an assignment written or removed changes the origin of the
            // object and of the objects below it that do not carry their own.
            Planned::Assign {
                object, property, ..
            }
            | Planned::Unassign {
                object, property, ..
            } if heritable.contains_key(property) => {
                let below = tx.subtree(*object, period).await?;
                targets.entry(*property).or_default().extend(below);
            }
            // Rule 3: a closed object loses its rows; its inheritors look further up.
            Planned::Close { kind, id, .. } if kind.is_addressing() => {
                add_all(&mut targets, &[*id]);
                for (property, object) in tx.inheritors(*id, period).await? {
                    targets.entry(property).or_default().insert(object);
                }
            }
            Planned::Close {
                kind: ObjectKind::Property,
                id,
                ..
            } => tx.drop_origins(*id, period, changeset).await?,
            Planned::UpdateDeclaration { id, .. } => {
                if heritable.contains_key(id) {
                    everything.insert(*id);
                } else {
                    tx.drop_origins(*id, period, changeset).await?;
                }
            }
            _ => {}
        }
    }
    for property in everything {
        let all = tx.objects_of_kinds(&heritable[&property], period).await?;
        targets.entry(property).or_default().extend(all);
    }
    let mut refreshed = BTreeMap::new();
    for (property, objects) in targets {
        let objects: Vec<Uuid> = objects.into_iter().collect();
        tx.refresh_origins(property, &heritable[&property], &objects, period, changeset)
            .await?;
        refreshed.insert(property, objects);
    }
    Ok(refreshed)
}

/// Records the differential of resolved values (REQ-003-19): the heritable properties on
/// the objects whose origin was refreshed, the others on the objects assigned directly.
/// New objects are left out (only existing objects change value).
pub(crate) async fn diff<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    refreshed: BTreeMap<Uuid, Vec<Uuid>>,
    period: Option<Period>,
) -> Result<Vec<DiffCount>, StoreError> {
    let created = created(planned);
    let mut direct: BTreeMap<Uuid, BTreeSet<Uuid>> = BTreeMap::new();
    for p in planned {
        if let Planned::Assign {
            object, property, ..
        }
        | Planned::Unassign {
            object, property, ..
        } = p
            && !refreshed.contains_key(property)
        {
            direct.entry(*property).or_default().insert(*object);
        }
    }
    for (property, objects) in &refreshed {
        tx.record_diff(*property, objects, true, &created, period)
            .await?;
    }
    for (property, objects) in direct {
        let objects: Vec<Uuid> = objects.into_iter().collect();
        tx.record_diff(property, &objects, false, &created, period)
            .await?;
    }
    tx.diff_summary().await
}

/// Objects the changeset creates.
fn created(planned: &[Planned]) -> Vec<Uuid> {
    planned
        .iter()
        .filter_map(|p| match p {
            Planned::Domain { id, .. } => Some(*id),
            Planned::Prefix { object, .. } => Some(object.id),
            Planned::Range { object, .. } => Some(object.id),
            Planned::Address { object, .. } => Some(object.id),
            _ => None,
        })
        .collect()
}

/// Obligations after resolution (REQ-003-16, P4): every object the changeset creates,
/// whose origin it refreshed or that it assigns directly, if it is a carrier in the scope
/// of an obligation, has a value over the whole window; explicit "no value" does not
/// count. The objects left without one are listed.
pub(crate) async fn check_obligations<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    refreshed: &BTreeMap<Uuid, Vec<Uuid>>,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let obligated = tx.obligated(period).await?;
    if obligated.is_empty() {
        return Ok(());
    }
    let mut touched: BTreeSet<Uuid> = created(planned).into_iter().collect();
    touched.extend(refreshed.values().flatten());
    for p in planned {
        if let Planned::Assign { object, .. } | Planned::Unassign { object, .. } = p {
            touched.insert(*object);
        }
    }
    let touched: Vec<Uuid> = touched.into_iter().collect();
    for (property, declaration) in obligated {
        let carriers: Vec<String> = declaration.carriers.iter().cloned().collect();
        let mut missing = BTreeSet::new();
        for scope in &declaration.obligations {
            missing.extend(
                tx.objects_without_value(
                    property,
                    &carriers,
                    declaration.inherit.is_some(),
                    scope,
                    Some(&touched),
                    period,
                )
                .await?,
            );
        }
        if !missing.is_empty() {
            violations.push(crate::declarations::required(
                &declaration.key,
                missing.into_iter().collect(),
            ));
        }
    }
    Ok(())
}
