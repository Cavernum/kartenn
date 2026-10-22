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
use kartenn_core::properties::{Declaration, InheritTree, ObligationScope, TypeSpec};
use kartenn_core::rules::{self, Violation};
use kartenn_core::values::{Raw, Value, ValueContext, parse_value};
use uuid::Uuid;

use crate::operations::{ObjectKind, Operation, Ref};
use crate::pipeline::Planned;
use crate::port::{Change, DiffCount, StoreError, WriteError, WriteTx};

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
    external_systems: HashMap<String, Uuid>,
}

impl ValueContext for Facts {
    fn enum_value(&self, key: &str) -> Option<Uuid> {
        self.enum_values.get(key).copied()
    }

    fn object_kind(&self, id: Uuid) -> Option<&'static str> {
        self.kinds.get(&id).copied()
    }

    fn external_system(&self, key: &str) -> Option<Uuid> {
        self.external_systems.get(key).copied()
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

/// Parses and validates raw values of a property for `object` over `period`, as an
/// assignment would (REQ-003-8): closed-list keys valid over the period, references to
/// objects of the target kind covering it.
pub(crate) async fn parse<T: WriteTx>(
    tx: &mut T,
    declaration: &Declaration,
    property: Uuid,
    object: Uuid,
    raws: &[Raw],
    period: Option<Period>,
) -> Result<Value, WriteError> {
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
        external_systems: HashMap::new(),
    };
    match declaration.spec {
        TypeSpec::Enum { .. } => {
            facts.enum_values = tx.enum_values_covering(property, &texts, period).await?;
        }
        // The systems valid over the whole period (REQ-016-3): a closed one is refused.
        TypeSpec::ExternalRef => {
            let systems: Vec<String> = raws
                .iter()
                .filter_map(|r| match r {
                    Raw::External { system, .. } => Some(system.clone()),
                    _ => None,
                })
                .collect();
            if let Some((list, _)) = tx
                .declaration_by_key(crate::declarations::EXTERNAL_SYSTEM, period)
                .await?
            {
                facts.external_systems = tx.enum_values_covering(list, &systems, period).await?;
            }
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
    parse_value(ty.as_ref(), declaration.cardinality, raws, &facts)
        .map_err(|e| invalid_value(object, format!("{}: {e}", declaration.key)))
}

/// Writes an assignment, or explicit "no value" when `values` is `None` (REQ-003-4, 8,
/// 12, 13), with its provenance (`input`, or `source:<id>@<version>`, REQ-003-22).
#[allow(clippy::too_many_arguments)] // one per column of the assignment and its context
pub(crate) async fn assign<T: WriteTx>(
    tx: &mut T,
    object: Uuid,
    kind: ObjectKind,
    property: Uuid,
    values: Option<&[Raw]>,
    provenance: &str,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let declaration = carried(tx, object, kind, property, period).await?;
    let assigned = match values {
        None => Assigned::Cleared,
        Some(raws) => {
            Assigned::Value(parse(tx, &declaration, property, object, raws, period).await?)
        }
    };
    tx.assign(
        object,
        kind.name(),
        property,
        &assigned,
        provenance,
        period,
        changeset,
    )
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

/// The tree an object of `kind` belongs to, if any: a tree of the core, or the tree of its
/// module.
fn tree_of(kind: ObjectKind) -> Option<InheritTree> {
    match kind {
        ObjectKind::Domain | ObjectKind::Prefix | ObjectKind::Range | ObjectKind::Address => {
            Some(InheritTree::Addressing)
        }
        ObjectKind::Entity => Some(InheritTree::Entity),
        ObjectKind::Site => Some(InheritTree::Site),
        ObjectKind::Module(kind) => Some(InheritTree::Module(kind.module().to_owned())),
        _ => None,
    }
}

/// Whether the objects of `kind` inherit a property heritable along `declared`: the
/// objects of its tree, and the objects of a module along the tree of their module,
/// whatever the tree declared (spec 008, REQ-008-6).
fn inherits_along(kind: ObjectKind, declared: &InheritTree) -> bool {
    matches!(kind, ObjectKind::Module(_)) || tree_of(kind).as_ref() == Some(declared)
}

/// What the maintenance of the origins did, per heritable property.
#[derive(Debug, Default)]
pub(crate) struct Maintained {
    /// Objects whose origin was recomputed.
    pub(crate) recomputed: BTreeMap<Uuid, Vec<Uuid>>,
    /// Origins whose value changed while carrying an assignment throughout: no origin
    /// moved because of them.
    pub(crate) revalued: BTreeMap<Uuid, Vec<Uuid>>,
    /// Objects whose resolved value may have changed: recomputed, or heirs of a revalued
    /// origin.
    pub(crate) affected: BTreeMap<Uuid, Vec<Uuid>>,
}

/// Brings the materialised origins in line with the writes of the changeset (the four
/// maintenance rules of the 003 design): new objects, assignments written or removed,
/// closed objects and properties, declarations whose carriers or tree changed. The origins
/// in `fresh` (per property, refreshed by the default values and still holding) count as
/// recomputed without being computed again.
#[allow(clippy::too_many_lines)] // one walk of the plan, then its reads in batches
pub(crate) async fn maintain<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    fresh: &BTreeMap<Uuid, BTreeSet<Uuid>>,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<Maintained, StoreError> {
    let heritable: HashMap<Uuid, InheritTree> = tx.heritable(period).await?.into_iter().collect();
    let mut targets: HashMap<Uuid, BTreeSet<Uuid>> = HashMap::new();
    // Origins whose value changed, and their heirs: values change, origins stay.
    let mut revalued: HashMap<Uuid, BTreeSet<Uuid>> = HashMap::new();
    let mut affected: HashMap<Uuid, BTreeSet<Uuid>> = HashMap::new();
    let mut everything: BTreeSet<Uuid> = BTreeSet::new();
    // Every property heritable along the tree of `kind` targets `objects`.
    let add_all =
        |targets: &mut HashMap<Uuid, BTreeSet<Uuid>>, kind: ObjectKind, objects: &[Uuid]| {
            for (p, t) in &heritable {
                if inherits_along(kind, t) {
                    targets.entry(*p).or_default().extend(objects);
                }
            }
        };
    // The reads of the subtrees and of the inheritors wait for the end of the walk, to be
    // made once for every object of the changeset rather than once per object (the import
    // of a batch of machines read them four times per machine).
    let mut moved: HashMap<ObjectKind, Vec<Uuid>> = HashMap::new();
    let mut origins: Vec<Uuid> = Vec::new();
    let mut assigned: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for p in planned {
        if let Some((kind, id)) = new_object(p) {
            add_all(&mut targets, kind, &[id]);
            continue;
        }
        match p {
            // A changed object of a module may have moved in the tree of its module: it and
            // the objects below it look for their origins again (REQ-008-6).
            Planned::Module { plan, .. } => {
                moved
                    .entry(ObjectKind::Module(plan.kind))
                    .or_default()
                    .push(plan.object);
                origins.push(plan.object);
            }
            // Rules 2 and 3: an assignment written or removed changes the origin of the
            // object and of the objects below it that do not carry their own; a new value
            // where one was already carried moves no origin, only its heirs' values.
            Planned::Assign {
                object, property, ..
            }
            | Planned::Unassign {
                object, property, ..
            } if heritable.contains_key(property) => {
                if tx
                    .assignment_presence_changed(*object, *property, period)
                    .await?
                {
                    assigned.entry(*property).or_default().push(*object);
                } else {
                    let heirs = tx.heirs(*property, *object, period).await?;
                    affected.entry(*property).or_default().extend(heirs);
                    revalued.entry(*property).or_default().insert(*object);
                }
            }
            // Rule 3: a closed object loses its rows; its inheritors look further up.
            Planned::Close { kind, id, .. } if tree_of(*kind).is_some() => {
                add_all(&mut targets, *kind, &[*id]);
                origins.push(*id);
            }
            Planned::Restore {
                kind, id, property, ..
            } => {
                restored(
                    tx,
                    (*kind, *id, *property),
                    &heritable,
                    &mut targets,
                    period,
                )
                .await?;
            }
            // A resized prefix or a moved entity or site moves the tree: the objects now
            // below it, and those that inherited from it and may have left (004-8, 003-18).
            Planned::Modify {
                kind: kind @ ObjectKind::Prefix,
                id,
                change: Change::Prefix { value: Some(_), .. },
                ..
            }
            | Planned::Modify {
                kind: kind @ (ObjectKind::Entity | ObjectKind::Site),
                id,
                change: Change::Parent(_),
                ..
            } => {
                moved.entry(*kind).or_default().push(*id);
                origins.push(*id);
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
    for (kind, objects) in moved {
        let below = tx.subtree(&objects, period).await?;
        add_all(&mut targets, kind, &below);
    }
    for (property, objects) in assigned {
        let below = tx.subtree(&objects, period).await?;
        targets.entry(property).or_default().extend(below);
    }
    inheritors(tx, &origins, &mut targets, period).await?;
    let tree = (heritable, targets, everything);
    refresh(tx, tree, fresh, revalued, affected, (period, changeset)).await
}

/// The object a planned operation creates in a tree, with its kind.
fn new_object(p: &Planned) -> Option<(ObjectKind, Uuid)> {
    match p {
        Planned::Domain { id, .. } => Some((ObjectKind::Domain, *id)),
        Planned::Prefix { object, .. } => Some((ObjectKind::Prefix, object.id)),
        Planned::Range { object, .. } => Some((ObjectKind::Range, object.id)),
        Planned::Address { object, .. } => Some((ObjectKind::Address, object.id)),
        Planned::Anchor { kind, id, .. } => Some((*kind, *id)),
        Planned::Module { plan, .. } if plan.creates => {
            Some((ObjectKind::Module(plan.kind), plan.object))
        }
        _ => None,
    }
}

/// The objects that inherit from one of `origins` look for their origin again.
async fn inheritors<T: WriteTx>(
    tx: &mut T,
    origins: &[Uuid],
    targets: &mut HashMap<Uuid, BTreeSet<Uuid>>,
    period: Option<Period>,
) -> Result<(), StoreError> {
    if origins.is_empty() {
        return Ok(());
    }
    for (property, object) in tx.inheritors(origins, period).await? {
        targets.entry(property).or_default().insert(object);
    }
    Ok(())
}

/// Objects per property.
type PerProperty = HashMap<Uuid, BTreeSet<Uuid>>;

/// Recomputes the origins of `targets`, and of every object of the tree for the
/// properties in `everything` (003/design: a row per carrier or heir, whatever the
/// carriers); what was done, with the origins revalued and the objects affected. The
/// objects of `fresh` are counted as recomputed, their origins already up to date.
async fn refresh<T: WriteTx>(
    tx: &mut T,
    (heritable, mut targets, everything): (HashMap<Uuid, InheritTree>, PerProperty, BTreeSet<Uuid>),
    fresh: &BTreeMap<Uuid, BTreeSet<Uuid>>,
    revalued: PerProperty,
    mut affected: PerProperty,
    (period, changeset): (Option<Period>, Uuid),
) -> Result<Maintained, StoreError> {
    let mut out = Maintained::default();
    for property in everything {
        let mut kinds: Vec<String> = heritable[&property]
            .kinds()
            .iter()
            .map(|k| (*k).to_owned())
            .collect();
        // The objects of modules that carry it inherit along the trees of their modules.
        if let Some(declaration) = tx.declaration(property, period).await? {
            kinds.extend(
                declaration
                    .carriers
                    .into_iter()
                    .filter(|c| kartenn_core::properties::is_module_kind(c)),
            );
        }
        let all = tx.objects_of_kinds(&kinds, period).await?;
        targets.entry(property).or_default().extend(all);
    }
    for (property, objects) in targets {
        let objects: Vec<Uuid> = objects.into_iter().collect();
        let stale: Vec<Uuid> = match fresh.get(&property) {
            Some(done) => objects
                .iter()
                .filter(|o| !done.contains(o))
                .copied()
                .collect(),
            None => objects.clone(),
        };
        if !stale.is_empty() {
            tx.refresh_origins(property, &heritable[&property], &stale, period, changeset)
                .await?;
        }
        affected.entry(property).or_default().extend(&objects);
        out.recomputed.insert(property, objects);
    }
    let list = |set: BTreeSet<Uuid>| set.into_iter().collect::<Vec<_>>();
    out.revalued = revalued.into_iter().map(|(p, s)| (p, list(s))).collect();
    out.affected = affected.into_iter().map(|(p, s)| (p, list(s))).collect();
    Ok(out)
}

/// Targets of an object or assignment restored as before a changeset: an object may
/// change place in the tree and carry other values; an assignment may give or take a
/// value to its subtree.
async fn restored<T: WriteTx>(
    tx: &mut T,
    (kind, id, property): (ObjectKind, Uuid, Option<Uuid>),
    heritable: &HashMap<Uuid, InheritTree>,
    targets: &mut HashMap<Uuid, BTreeSet<Uuid>>,
    period: Option<Period>,
) -> Result<(), StoreError> {
    match property {
        None if tree_of(kind).is_some() => {
            let mut objects = vec![id];
            if matches!(
                kind,
                ObjectKind::Prefix | ObjectKind::Entity | ObjectKind::Site | ObjectKind::Module(_)
            ) {
                objects.extend(tx.subtree(&[id], period).await?);
            }
            for (p, t) in heritable {
                if inherits_along(kind, t) {
                    targets.entry(*p).or_default().extend(&objects);
                }
            }
            inheritors(tx, &[id], targets, period).await?;
        }
        Some(property) if heritable.contains_key(&property) => {
            let below = tx.subtree(&[id], period).await?;
            targets.entry(property).or_default().extend(below);
        }
        _ => {}
    }
    Ok(())
}

/// Records the differential of resolved values (REQ-003-19): the heritable properties on
/// the objects whose origin was recomputed and on the heirs of the revalued origins, the
/// others on the objects assigned directly. New objects are left out (only existing
/// objects change value).
pub(crate) async fn diff<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    maintained: &Maintained,
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
            && !maintained.affected.contains_key(property)
        {
            direct.entry(*property).or_default().insert(*object);
        }
        if let Planned::Restore {
            id,
            property: Some(property),
            ..
        } = p
            && !maintained.affected.contains_key(property)
        {
            direct.entry(*property).or_default().insert(*id);
        }
    }
    for (property, objects) in &maintained.recomputed {
        tx.record_diff(*property, objects, true, &created, period)
            .await?;
    }
    // Heirs whose origin did not move: their origin's change of value, once for all.
    for (property, origins) in &maintained.revalued {
        let mut excluded = created.clone();
        excluded.extend(maintained.recomputed.get(property).into_iter().flatten());
        tx.record_value_diff(*property, origins, &excluded, period)
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
            Planned::Domain { id, .. } | Planned::Anchor { id, .. } => Some(*id),
            Planned::Prefix { object, .. } => Some(object.id),
            Planned::Range { object, .. } => Some(object.id),
            Planned::Address { object, .. } => Some(object.id),
            Planned::Module { plan, .. } if plan.creates => Some(plan.object),
            _ => None,
        })
        .collect()
}

/// Objects the changeset creates, whose origin it refreshed or that it assigns directly.
pub(crate) fn touched(planned: &[Planned], refreshed: &BTreeMap<Uuid, Vec<Uuid>>) -> Vec<Uuid> {
    let mut touched: BTreeSet<Uuid> = created(planned).into_iter().collect();
    touched.extend(refreshed.values().flatten());
    for p in planned {
        if let Planned::Assign { object, .. } | Planned::Unassign { object, .. } = p {
            touched.insert(*object);
        }
        if let Planned::Restore { id, .. } = p {
            touched.insert(*id);
        }
    }
    touched.into_iter().collect()
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
    let touched = touched(planned, refreshed);
    // A moved entity carries the objects it owns into or out of the scope of its
    // ancestors: those scopes are checked whole.
    let moved = planned.iter().any(|p| {
        matches!(
            p,
            Planned::Modify {
                kind: ObjectKind::Entity,
                change: Change::Parent(_),
                ..
            }
        )
    });
    // An obligation an administrator adds is checked whole, on the final state: the
    // objects the same changeset classifies count (REQ-008-28, REQ-010-21).
    let added: Vec<(Uuid, &ObligationScope)> = planned
        .iter()
        .filter_map(|p| match p {
            Planned::Modify {
                id,
                change:
                    Change::Obligation {
                        scope,
                        required: true,
                    },
                ..
            } => Some((*id, scope)),
            _ => None,
        })
        .collect();
    for (property, declaration) in obligated {
        let carriers: Vec<String> = declaration.carriers.iter().cloned().collect();
        let mut missing = BTreeSet::new();
        for scope in &declaration.obligations {
            let whole = (moved && matches!(scope, ObligationScope::Entity(_)))
                || added.contains(&(property, scope));
            missing.extend(
                tx.objects_without_value(
                    property,
                    &carriers,
                    declaration.inherit.is_some(),
                    scope,
                    (!whole).then_some(touched.as_slice()),
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
