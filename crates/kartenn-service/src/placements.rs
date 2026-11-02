//! The placements of a simulation (spec 021, A12; 006 design « `?simulate=true` »): the
//! objects whose parent the changeset changes at the present, each with its parent before
//! and after, deduced (prefixes, ranges, addresses, D34) or declared (entities, sites,
//! VLAN groups and VLANs, objects of a module).
//!
//! Read from what the simulation already holds, never by a read of the whole tree: the
//! objects of the changeset, read before its writes and after them; then, for each prefix
//! whose value changes, appears or goes, the direct children of its old and new values on
//! the state after the writes, their parent before deduced from that state with the
//! prefixes of the changeset as they were. An object whose parent changes is one of these
//! (the parent it leaves or the one it gains is such a prefix). An object created is placed
//! from no parent (`before` `None`); an object closed is not placed: it leaves the tree.
//!
//! Bounded: at most [`LIMIT`] objects are examined beyond those of the changeset, and at
//! most [`LIMIT`] placements answered; past either, [`Placements::truncated`] says that
//! some may be missing.

use std::collections::{BTreeMap, HashMap, HashSet};

use kartenn_core::addr::Prefix;
use uuid::Uuid;

use crate::module::ModuleRegistry;
use crate::operations::{ModuleKind, ObjectKind};
use crate::pipeline::Planned;
use crate::port::{StoreError, Supposed, TreePlace, WriteTx};

/// The most placements a simulation answers, and the most objects outside the changeset it
/// examines for them: an insert between a prefix and 10⁴ children places them all.
pub const LIMIT: usize = 1000;

/// An object whose parent the changeset changes, at the present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// The object.
    pub object: Uuid,
    /// Its kind.
    pub kind: ObjectKind,
    /// Its parent before; `None` at the root, or created.
    pub before: Option<Uuid>,
    /// Its parent after; `None` at the root.
    pub after: Option<Uuid>,
}

/// The placements of a simulation: those of the objects of the changeset first, in the
/// order of its operations, then those of the objects it moves, in address order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Placements {
    /// The placements, at most [`LIMIT`].
    pub items: Vec<Placement>,
    /// Some may be missing: past [`LIMIT`].
    pub truncated: bool,
}

/// The objects of a changeset in a tree, and their places before its writes.
#[derive(Debug, Default)]
pub(crate) struct Prior {
    touched: Vec<Uuid>,
    places: HashMap<Uuid, TreePlace>,
}

/// Whether an object of `kind` has a place in a tree.
const fn in_tree(kind: ObjectKind) -> bool {
    !matches!(
        kind,
        ObjectKind::Domain | ObjectKind::Property | ObjectKind::EnumValue
    )
}

/// The places of `objects`, the declared parents of the objects of a module read from its
/// handler where its tree is not that of their records.
async fn places<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    objects: &[Uuid],
    supposed: Supposed<'_>,
) -> Result<HashMap<Uuid, TreePlace>, StoreError> {
    if objects.is_empty() {
        return Ok(HashMap::new());
    }
    let mut found: HashMap<Uuid, TreePlace> = tx
        .tree_places(objects, supposed)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let mut by_module: BTreeMap<String, Vec<(ModuleKind, Uuid)>> = BTreeMap::new();
    for place in found.values() {
        if let ObjectKind::Module(kind) = place.kind {
            by_module
                .entry(kind.module().to_owned())
                .or_default()
                .push((kind, place.id));
        }
    }
    for objects in by_module.into_values() {
        let Some(handler) = objects.first().and_then(|(k, _)| modules.handler_of(*k)) else {
            continue;
        };
        for (id, parent) in handler.parents(tx.module_tx(), &objects).await? {
            if let Some(place) = found.get_mut(&id) {
                place.parent = parent;
            }
        }
    }
    Ok(found)
}

/// The objects of `planned` in a tree, read before the writes.
pub(crate) async fn prior<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    planned: &[Planned],
) -> Result<Prior, StoreError> {
    let touched: Vec<Uuid> = crate::sections::touched(planned)
        .into_iter()
        .filter(|(kind, _)| in_tree(*kind))
        .map(|(_, id)| id)
        .collect();
    let places = places(tx, modules, &touched, Supposed::default()).await?;
    Ok(Prior { touched, places })
}

/// The placements of the changeset whose objects `prior` read, after its writes.
pub(crate) async fn placements<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    prior: Prior,
) -> Result<Placements, StoreError> {
    let Prior {
        touched,
        places: before,
    } = prior;
    let after = places(tx, modules, &touched, Supposed::default()).await?;
    let mut items = Vec::new();
    for id in &touched {
        // Closed, or not valid at the present: it leaves the tree.
        let Some(now) = after.get(id) else { continue };
        let then = before.get(id).map(|p| p.parent);
        if then != Some(now.parent) {
            items.push(Placement {
                object: *id,
                kind: now.kind,
                before: then.flatten(),
                after: now.parent,
            });
        }
    }
    // The values of the prefixes of the changeset that change, appear or go: their old
    // children may leave them, their new ones join them.
    let prefix =
        |places: &HashMap<Uuid, TreePlace>, id: &Uuid| places.get(id).and_then(|p| p.prefix);
    let mut regions: Vec<(Uuid, Prefix)> = Vec::new();
    let mut written: Vec<Uuid> = Vec::new();
    for id in &touched {
        let (old, new) = (prefix(&before, id), prefix(&after, id));
        if old.is_some() || new.is_some() {
            written.push(*id);
        }
        if old != new {
            for region in old.into_iter().chain(new) {
                if !regions.contains(&region) {
                    regions.push(region);
                }
            }
        }
    }
    let mut truncated = false;
    let changed: HashSet<Uuid> = touched.iter().copied().collect();
    let mut seen = HashSet::new();
    let mut moved: Vec<(ObjectKind, Uuid)> = Vec::new();
    for (domain, value) in &regions {
        let budget = LIMIT.saturating_sub(moved.len());
        if budget == 0 {
            truncated = true;
            break;
        }
        let limit = i64::try_from(budget + 1).unwrap_or(i64::MAX);
        let children = tx.tree_children(*domain, value, limit).await?;
        truncated |= children.len() > budget;
        for (kind, id) in children.into_iter().take(budget) {
            if !changed.contains(&id) && seen.insert(id) {
                moved.push((kind, id));
            }
        }
    }
    if !moved.is_empty() {
        let ids: Vec<Uuid> = moved.iter().map(|(_, id)| *id).collect();
        let now: HashMap<Uuid, Option<Uuid>> = tx
            .tree_places(&ids, Supposed::default())
            .await?
            .into_iter()
            .map(|p| (p.id, p.parent))
            .collect();
        // Before the writes: the prefixes of the changeset as they were, the others as
        // they are (the changeset did not write them).
        let with: Vec<(Uuid, Uuid, Prefix)> = written
            .iter()
            .filter_map(|id| prefix(&before, id).map(|(domain, value)| (*id, domain, value)))
            .collect();
        let supposed = Supposed {
            without: &written,
            with: &with,
        };
        let then: HashMap<Uuid, Option<Uuid>> = tx
            .tree_places(&ids, supposed)
            .await?
            .into_iter()
            .map(|p| (p.id, p.parent))
            .collect();
        for (kind, id) in moved {
            if let (Some(before), Some(after)) = (then.get(&id), now.get(&id))
                && before != after
            {
                items.push(Placement {
                    object: id,
                    kind,
                    before: *before,
                    after: *after,
                });
            }
        }
    }
    if items.len() > LIMIT {
        items.truncate(LIMIT);
        truncated = true;
    }
    Ok(Placements { items, truncated })
}
