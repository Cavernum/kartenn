//! Imports in batches (spec 004, REQ-004-28 to 30; tasks 004-18, 19; 004/design,
//! § Imports).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

use kartenn_core::objects::Nature;
use uuid::Uuid;

use crate::authz::Authorizer;
use crate::operations::{ChangesetRequest, ObjectKind, Operation, Ref};
use crate::pipeline::{Outcome, RequestError, references, submit};
use crate::port::{ImportBatch, ImportItem, ImportRecord, Store, StoreError};

/// Why an import cannot be created; nothing is recorded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ImportError {
    /// An element creates an object without its identifier in the source.
    #[error("element {0} creates an object without an external identifier")]
    MissingExternalId(usize),
    /// An element uses a local reference that no element defines.
    #[error("element {element} uses ${local}, which no element defines")]
    UnknownReference {
        /// Index of the element.
        element: usize,
        /// Local name.
        local: u32,
    },
    /// A local name is defined twice, or the references form a cycle.
    #[error("the local references are defined twice or form a cycle")]
    Order,
    /// Infrastructure failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Where an import stands (REQ-004-30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportStatus {
    /// `queued`, `running`, `paused` or `done`.
    pub state: String,
    /// Batches applied.
    pub applied: usize,
    /// Batches left.
    pub remaining: usize,
    /// The failure that paused it: batch, then its violations or the pending changeset.
    pub report: serde_json::Value,
}

/// Rank of an operation in the order of dependencies (004/design): entities, sites,
/// domains, containers from the shortest prefix, networks, ranges, addresses,
/// declarations, assignments, the rest.
fn rank(op: &Operation) -> (u8, u8) {
    match op.inner() {
        Operation::CreateEntity { .. } => (0, 0),
        Operation::CreateSite { .. } => (1, 0),
        Operation::CreateDomain { .. } => (2, 0),
        Operation::CreatePrefix { prefix, nature, .. } => {
            let length = prefix
                .rsplit_once('/')
                .and_then(|(_, l)| l.parse().ok())
                .unwrap_or(u8::MAX);
            (if *nature == Nature::Container { 3 } else { 4 }, length)
        }
        Operation::CreateRange { .. } => (5, 0),
        Operation::CreateAddress { .. } => (6, 0),
        Operation::DeclareProperty { .. } | Operation::AddEnumValue { .. } => (7, 0),
        Operation::AssignProperty { .. }
        | Operation::ClearProperty { .. }
        | Operation::UnassignProperty { .. } => (8, 0),
        _ => (9, 0),
    }
}

/// The kind of the object an operation creates.
fn created_kind(op: &Operation) -> Option<ObjectKind> {
    Some(match op.inner() {
        Operation::CreateDomain { .. } => ObjectKind::Domain,
        Operation::CreatePrefix { .. } => ObjectKind::Prefix,
        Operation::CreateRange { .. } => ObjectKind::Range,
        Operation::CreateAddress { .. } => ObjectKind::Address,
        Operation::CreateEntity { .. } => ObjectKind::Entity,
        Operation::CreateSite { .. } => ObjectKind::Site,
        Operation::DeclareProperty { .. } => ObjectKind::Property,
        Operation::AddEnumValue { .. } => ObjectKind::EnumValue,
        _ => return None,
    })
}

/// The elements in an order where every local reference is defined before it is used,
/// by rank otherwise.
fn order(items: Vec<ImportItem>) -> Result<Vec<ImportItem>, ImportError> {
    let mut definer: HashMap<u32, usize> = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        if let Some(local) = item.operation.defines() {
            if item.external_id.is_none() {
                return Err(ImportError::MissingExternalId(i));
            }
            if definer.insert(local, i).is_some() {
                return Err(ImportError::Order);
            }
        }
    }
    let mut waiting = vec![0_usize; items.len()];
    let mut users: Vec<Vec<usize>> = vec![Vec::new(); items.len()];
    for (i, item) in items.iter().enumerate() {
        for (r, _) in references(item.operation.inner()) {
            if let Ref::Local(local) = r {
                let Some(&d) = definer.get(&local) else {
                    return Err(ImportError::UnknownReference { element: i, local });
                };
                if d != i {
                    waiting[i] += 1;
                    users[d].push(i);
                }
            }
        }
    }
    let mut ready: BinaryHeap<Reverse<((u8, u8), usize)>> = BinaryHeap::new();
    for (i, item) in items.iter().enumerate() {
        if waiting[i] == 0 {
            ready.push(Reverse((rank(&item.operation), i)));
        }
    }
    let mut sorted = Vec::with_capacity(items.len());
    while let Some(Reverse((_, i))) = ready.pop() {
        sorted.push(i);
        for &u in &users[i] {
            waiting[u] -= 1;
            if waiting[u] == 0 {
                ready.push(Reverse((rank(&items[u].operation), u)));
            }
        }
    }
    if sorted.len() != items.len() {
        return Err(ImportError::Order);
    }
    let mut slots: Vec<Option<ImportItem>> = items.into_iter().map(Some).collect();
    Ok(sorted.into_iter().filter_map(|i| slots[i].take()).collect())
}

/// Creates an import of `items` from `source` (REQ-004-28): ordered by dependency and cut
/// in batches of at most `batch_operations` operations (a creation counts two, with its
/// binding to its external identifier). Nothing is applied yet.
///
/// # Errors
///
/// [`ImportError`] for elements that cannot be ordered; nothing is recorded.
pub async fn create_import<S: Store>(
    store: &S,
    author: Uuid,
    source: &str,
    items: Vec<ImportItem>,
    batch_operations: usize,
) -> Result<Uuid, ImportError> {
    let sorted = order(items)?;
    let mut batches: Vec<Vec<ImportItem>> = Vec::new();
    let mut current = Vec::new();
    let mut size = 0;
    for item in sorted {
        let cost = if item.operation.defines().is_some() {
            2
        } else {
            1
        };
        if !current.is_empty() && size + cost > batch_operations.max(2) {
            batches.push(std::mem::take(&mut current));
            size = 0;
        }
        size += cost;
        current.push(item);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    let id = Uuid::now_v7();
    let record = ImportRecord {
        id,
        author,
        source: source.to_owned(),
        state: "queued".into(),
        report: serde_json::json!({}),
        batches: batches
            .into_iter()
            .enumerate()
            .map(|(i, items)| ImportBatch {
                index: i32::try_from(i).unwrap_or(i32::MAX),
                items,
                state: "queued".into(),
                changeset: None,
            })
            .collect(),
    };
    store.create_import(&record).await?;
    Ok(id)
}

/// Replaces in `op` the local references found in `known` by identifiers.
fn resolve_known(op: &Operation, known: &BTreeMap<u32, Uuid>) -> Result<Operation, StoreError> {
    fn walk(value: &mut serde_json::Value, known: &BTreeMap<u32, Uuid>) {
        match value {
            serde_json::Value::Object(map) => {
                // A reference is `{"local": n}` alone; a definition is a number field.
                if map.len() == 1
                    && let Some(local) = map
                        .get("local")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|n| u32::try_from(n).ok())
                    && let Some(id) = known.get(&local)
                {
                    *value = serde_json::json!({ "id": id });
                    return;
                }
                for v in map.values_mut() {
                    walk(v, known);
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|v| walk(v, known)),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(op).map_err(|e| StoreError(e.to_string()))?;
    walk(&mut value, known);
    serde_json::from_value(value).map_err(|e| StoreError(e.to_string()))
}

/// Applies the batches of an import not applied yet, in order, each as a changeset of its
/// author (REQ-004-28, 29): a creation already known to the source is left out (replay
/// without duplicates), references to objects of other batches become their
/// identifiers. A refused or held batch pauses the import with its report.
///
/// # Errors
///
/// [`RequestError`] on infrastructure failure or a malformed batch.
pub async fn run_import<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    import: Uuid,
    max_operations: usize,
) -> Result<Option<ImportStatus>, RequestError> {
    let Some(record) = store.import(import).await? else {
        return Ok(None);
    };
    // Every local defined by the import, with its identifier in the source.
    let externals: BTreeMap<u32, String> = record
        .batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|i| Some((i.operation.defines()?, i.external_id.clone()?)))
        .collect();
    store
        .set_import(import, "running", &serde_json::json!({}))
        .await?;
    for batch in record.batches.iter().filter(|b| b.state != "applied") {
        let mut known: BTreeMap<u32, Uuid> = BTreeMap::new();
        for (local, external) in &externals {
            if let Some(id) = store.external_object(&record.source, external).await? {
                known.insert(*local, id);
            }
        }
        let mut operations = Vec::new();
        for item in &batch.items {
            if let Some(local) = item.operation.defines() {
                if known.contains_key(&local) {
                    continue;
                }
                let kind = created_kind(&item.operation).unwrap_or(ObjectKind::Address);
                operations.push(resolve_known(&item.operation, &known)?);
                operations.push(Operation::BindExternal {
                    object: Ref::Local(local),
                    kind,
                    source: record.source.clone(),
                    external_id: item.external_id.clone().unwrap_or_default(),
                });
            } else {
                operations.push(resolve_known(&item.operation, &known)?);
            }
        }
        if operations.is_empty() {
            store
                .set_batch(import, batch.index, "applied", None)
                .await?;
            continue;
        }
        let request = ChangesetRequest {
            author: record.author,
            motive: format!("import {} batch {}", record.source, batch.index),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        let report = match submit(store, authorizer, &request, max_operations).await? {
            Outcome::Applied { changeset, .. } => {
                store
                    .set_batch(import, batch.index, "applied", Some(changeset))
                    .await?;
                continue;
            }
            Outcome::Refused {
                changeset,
                violations,
            }
            | Outcome::FailedRevalidation {
                changeset,
                violations,
            } => serde_json::json!({
                "batch": batch.index,
                "changeset": changeset,
                "violations": violations
                    .iter()
                    .map(|v| serde_json::json!({ "rule": v.rule.id, "message": v.message, "objects": v.subjects }))
                    .collect::<Vec<_>>(),
            }),
            Outcome::Pending { changeset, .. } => {
                serde_json::json!({ "batch": batch.index, "pending": changeset })
            }
            Outcome::Replayed { changeset, .. } => {
                serde_json::json!({ "batch": batch.index, "replayed": changeset })
            }
            Outcome::Simulated { .. } => serde_json::json!({ "batch": batch.index }),
        };
        store.set_batch(import, batch.index, "failed", None).await?;
        store.set_import(import, "paused", &report).await?;
        return Ok(import_status(store, import).await?);
    }
    store
        .set_import(import, "done", &serde_json::json!({}))
        .await?;
    Ok(import_status(store, import).await?)
}

/// Where an import stands (REQ-004-30).
///
/// # Errors
///
/// [`StoreError`] on infrastructure failure.
pub async fn import_status<S: Store>(
    store: &S,
    import: Uuid,
) -> Result<Option<ImportStatus>, StoreError> {
    Ok(store.import(import).await?.map(|record| {
        let applied = record
            .batches
            .iter()
            .filter(|b| b.state == "applied")
            .count();
        ImportStatus {
            state: record.state,
            applied,
            remaining: record.batches.len() - applied,
            report: record.report,
        }
    }))
}
