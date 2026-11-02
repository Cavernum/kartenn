//! The placements of a simulation (spec 021, A12; 006 design « `?simulate=true` »): the
//! objects whose parent the changeset changes at the present, each with its parent before
//! and after, deduced (prefixes, ranges, addresses, D34) or declared (entities, sites,
//! VLAN groups and VLANs, objects of a module in a tree).
//!
//! Read from what the simulation already holds, never by a read of the whole tree: the
//! objects of the changeset, read before its writes and after them; then, for each prefix
//! whose value changes, appears or goes, the direct children of its old and new values on
//! the state after the writes, their parent before deduced from that state with the
//! prefixes of the changeset as they were. An object whose parent changes is one of these
//! (the parent it leaves or the one it gains is such a prefix). An object created is placed
//! from no parent (`before` `None`); an object closed is not placed: it leaves the tree.
//!
//! Under the read filter of the tree (precised by task 021-24a): an object its reader may
//! not read is not placed, and a parent it may not read is answered `None`, as at the top
//! of the tree it reads; a placement left without a change of parent is dropped. The bound
//! counts readable objects only: at most [`LIMIT`] readable objects are examined beyond
//! those of the changeset, and at most [`LIMIT`] placements answered; past either,
//! [`Placements::truncated`] says that some may be missing.

use std::collections::{BTreeMap, HashMap, HashSet};

use kartenn_core::addr::Prefix;
use uuid::Uuid;

use crate::authz::{Authorizer, Subject};
use crate::module::ModuleRegistry;
use crate::operations::{ModuleKind, ObjectKind};
use crate::pipeline::Planned;
use crate::port::{StoreError, Supposed, TreePlace, WriteTx};

/// The most placements a simulation answers, and the most readable objects outside the
/// changeset it examines for them: an insert between a prefix and 10⁴ children places
/// them all.
pub const LIMIT: usize = 1000;

/// The children read at once from the new or old value of a prefix.
const PAGE: i64 = 1001;

/// An object whose parent the changeset changes, at the present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// The object.
    pub object: Uuid,
    /// Its kind.
    pub kind: ObjectKind,
    /// Its parent before; `None` at the root, created, or not readable.
    pub before: Option<Uuid>,
    /// Its parent after; `None` at the root, or not readable.
    pub after: Option<Uuid>,
}

/// The placements of a simulation: those of the objects of the changeset first, in the
/// order of its operations, then those of the objects it moves, in address order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Placements {
    /// The placements, at most [`LIMIT`].
    pub items: Vec<Placement>,
    /// Some may be missing: past [`LIMIT`] readable ones.
    pub truncated: bool,
}

/// Who reads the placements: the author of the simulation, under the authorizer of the
/// pipeline.
pub(crate) struct Reader<'a, A> {
    /// The authorizer.
    pub(crate) authorizer: &'a A,
    /// The author.
    pub(crate) subject: &'a Subject,
}

/// The objects of a changeset in a tree, and their places before its writes.
#[derive(Debug, Default)]
pub(crate) struct Prior {
    touched: Vec<Uuid>,
    places: HashMap<Uuid, TreePlace>,
    /// Among those objects and their parents, before the writes, those the reader may not
    /// read.
    hidden: HashSet<Uuid>,
}

/// Whether an object of `kind` may have a place in a tree (an object of a module, as its
/// type and handler say: [`places`]).
const fn in_tree(kind: ObjectKind) -> bool {
    !matches!(
        kind,
        ObjectKind::Domain | ObjectKind::Property | ObjectKind::EnumValue
    )
}

/// The places of `objects`: the declared parents of the objects of a module read from its
/// handler where its tree is not that of their records; an object of a module that its
/// handler does not place and whose type has no parent is in no tree (a flow, a policy).
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
        .filter(|p| p.kind != ObjectKind::Domain)
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
        let mut placed = HashSet::new();
        if let Some(handler) = objects.first().and_then(|(k, _)| modules.handler_of(*k)) {
            for (id, parent) in handler.parents(tx.module_tx(), &objects).await? {
                if let Some(place) = found.get_mut(&id) {
                    place.parent = parent;
                    placed.insert(id);
                }
            }
        }
        for (kind, id) in objects {
            let record = modules
                .object_type(kind.as_str())
                .is_some_and(|t| t.parent.is_some());
            if !placed.contains(&id) && !record {
                found.remove(&id);
            }
        }
    }
    Ok(found)
}

/// The kind of each of `ids` valid at the instant of the transaction.
async fn kinds<T: WriteTx>(
    tx: &mut T,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, ObjectKind>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(tx
        .tree_places(ids, Supposed::default())
        .await?
        .into_iter()
        .map(|p| (p.id, p.kind))
        .collect())
}

/// Among `objects`, valid at the instant of the transaction, those `reader` may not read
/// (`core.<kind>.read`, or the read permission of a type of a module).
async fn hidden<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    reader: &Reader<'_, A>,
    objects: &[(ObjectKind, Uuid)],
) -> Result<HashSet<Uuid>, StoreError> {
    let mut by_permission: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    for (kind, id) in objects {
        by_permission
            .entry(crate::reads::read_permission(kind.name()))
            .or_default()
            .push(*id);
    }
    let mut denied = HashSet::new();
    for (permission, ids) in by_permission {
        denied.extend(
            reader
                .authorizer
                .authorize_in(tx, reader.subject, &permission, &ids)
                .await?,
        );
    }
    Ok(denied)
}

/// Among `ids` (with the kinds known), those `reader` may not read; an identifier of
/// unknown kind is not readable.
async fn hidden_ids<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    reader: &Reader<'_, A>,
    known: &HashMap<Uuid, ObjectKind>,
    ids: impl IntoIterator<Item = Uuid>,
) -> Result<HashSet<Uuid>, StoreError> {
    let mut unknown = HashSet::new();
    let mut objects = Vec::new();
    for id in ids {
        match known.get(&id) {
            Some(kind) => objects.push((*kind, id)),
            None => {
                unknown.insert(id);
            }
        }
    }
    let mut denied = hidden(tx, reader, &objects).await?;
    denied.extend(unknown);
    Ok(denied)
}

/// The children of `regions` (domain, value) outside `changed` that `reader` may read, at
/// most [`LIMIT`], page by page: an unreadable one costs no budget, so that the bound says
/// nothing of what it may not read; `true` if a readable one is left out.
async fn children<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    reader: &Reader<'_, A>,
    regions: &[(Uuid, Prefix)],
    changed: &HashSet<Uuid>,
) -> Result<(Vec<(ObjectKind, Uuid)>, bool), StoreError> {
    let mut seen = HashSet::new();
    let mut moved: Vec<(ObjectKind, Uuid)> = Vec::new();
    for (domain, value) in regions {
        let mut cursor = None;
        loop {
            let page = tx.tree_children(*domain, value, cursor, PAGE).await?;
            cursor = page.last().map(|(_, id)| *id);
            let fresh: Vec<(ObjectKind, Uuid)> = page
                .iter()
                .filter(|(_, id)| !changed.contains(id) && seen.insert(*id))
                .copied()
                .collect();
            let denied = hidden(tx, reader, &fresh).await?;
            for (kind, id) in fresh {
                if denied.contains(&id) {
                    continue;
                }
                if moved.len() == LIMIT {
                    return Ok((moved, true));
                }
                moved.push((kind, id));
            }
            if usize::try_from(PAGE).map_or(true, |size| page.len() < size) {
                break;
            }
        }
    }
    Ok((moved, false))
}

/// The placements of `found` (each with whether its object is created) as their reader
/// sees them, `hidden` what it may not read: an object hidden is left out, a parent hidden
/// is `None`, and what is left without a change of parent is dropped.
fn shown(found: Vec<(Placement, bool)>, hidden: &HashSet<Uuid>) -> Vec<Placement> {
    let visible = |parent: Option<Uuid>| parent.filter(|p| !hidden.contains(p));
    found
        .into_iter()
        .filter(|(p, _)| !hidden.contains(&p.object))
        .filter_map(|(p, created)| {
            let (before, after) = (visible(p.before), visible(p.after));
            (created || before != after).then_some(Placement { before, after, ..p })
        })
        .collect()
}

/// The objects of `planned` in a tree, read before the writes, with what `reader` may not
/// read among them and their parents.
pub(crate) async fn prior<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    modules: &ModuleRegistry,
    reader: &Reader<'_, A>,
    planned: &[Planned],
) -> Result<Prior, StoreError> {
    let touched: Vec<Uuid> = crate::sections::touched(planned)
        .into_iter()
        .filter(|(kind, _)| in_tree(*kind))
        .map(|(_, id)| id)
        .collect();
    let places = places(tx, modules, &touched, Supposed::default()).await?;
    let parents: Vec<Uuid> = places.values().filter_map(|p| p.parent).collect();
    let mut known = kinds(tx, &parents).await?;
    known.extend(places.values().map(|p| (p.id, p.kind)));
    let ids: Vec<Uuid> = places.keys().copied().chain(parents).collect();
    let hidden = hidden_ids(tx, reader, &known, ids).await?;
    Ok(Prior {
        touched,
        places,
        hidden,
    })
}

/// The placements of the changeset whose objects `prior` read, after its writes, as
/// `reader` may read them.
pub(crate) async fn placements<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    modules: &ModuleRegistry,
    reader: &Reader<'_, A>,
    prior: Prior,
) -> Result<Placements, StoreError> {
    let Prior {
        touched,
        places: before,
        hidden: hidden_before,
    } = prior;
    let after = places(tx, modules, &touched, Supposed::default()).await?;
    // Each placement found, and whether its object is created.
    let mut found: Vec<(Placement, bool)> = Vec::new();
    for id in &touched {
        // Closed, or not valid at the present: it leaves the tree.
        let Some(now) = after.get(id) else { continue };
        let then = before.get(id).map(|p| p.parent);
        if then != Some(now.parent) {
            let placement = Placement {
                object: *id,
                kind: now.kind,
                before: then.flatten(),
                after: now.parent,
            };
            found.push((placement, then.is_none()));
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
    let changed: HashSet<Uuid> = touched.iter().copied().collect();
    let (moved, mut truncated) = children(tx, reader, &regions, &changed).await?;
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
                let placement = Placement {
                    object: id,
                    kind,
                    before: *before,
                    after: *after,
                };
                found.push((placement, false));
            }
        }
    }
    // What the reader may not read after the writes, among the objects placed and their
    // parents still valid; the prefixes written were read before them.
    let mut known: HashMap<Uuid, ObjectKind> =
        found.iter().map(|(p, _)| (p.object, p.kind)).collect();
    let parents: Vec<Uuid> = found
        .iter()
        .flat_map(|(p, _)| [p.before, p.after])
        .flatten()
        .filter(|id| !hidden_before.contains(id))
        .collect();
    let valid = kinds(tx, &parents).await?;
    let checked: Vec<Uuid> = known.keys().copied().chain(valid.keys().copied()).collect();
    known.extend(valid);
    let mut hidden = hidden_ids(tx, reader, &known, checked).await?;
    hidden.extend(hidden_before);
    let mut items = shown(found, &hidden);
    if items.len() > LIMIT {
        items.truncate(LIMIT);
        truncated = true;
    }
    Ok(Placements { items, truncated })
}
