//! Imports in batches (spec 004, REQ-004-28 to 30; tasks 004-18, 19; 004/design,
//! § Imports).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

use kartenn_core::objects::Nature;
use uuid::Uuid;

use crate::audit::RequestContext;
use crate::authz::Authorizer;
use crate::expected::Sources;
use crate::module::ModuleRegistry;
use crate::operations::{ChangesetRequest, ObjectKind, Operation, Ref};
use crate::pipeline::{Outcome, RequestError, references, submit_in};
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
    /// `uploading`, `ready`, `running`, `paused`, `done` or `cancelled`.
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

/// The kind of the object a module operation creates, as its module declares it; or an
/// operation of the VLANs of the core (spec 026).
fn created_module_kind(modules: &ModuleRegistry, op: &Operation) -> Option<ObjectKind> {
    let Operation::Module(m) = op.inner() else {
        return None;
    };
    modules
        .operation_type(&m.module, &m.name)?
        .defines
        .and_then(ObjectKind::from_name)
}

/// The local names an operation uses: those of its references, and, in the body of a
/// module operation (REQ-008-23), every `{"local": n}`, the form of a [`Ref`].
fn uses(op: &Operation) -> Vec<u32> {
    fn walk(value: &serde_json::Value, out: &mut Vec<u32>) {
        match value {
            serde_json::Value::Object(map) => {
                if map.len() == 1
                    && let Some(local) = map
                        .get("local")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|n| u32::try_from(n).ok())
                {
                    out.push(local);
                    return;
                }
                map.values().for_each(|v| walk(v, out));
            }
            serde_json::Value::Array(items) => items.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    if let Operation::Module(m) = op {
        // The local a module operation defines is a number field, never an object.
        let mut out = Vec::new();
        m.body.values().for_each(|v| walk(v, &mut out));
        return out;
    }
    references(op)
        .into_iter()
        .filter_map(|(r, _)| match r {
            Ref::Local(local) => Some(local),
            Ref::Id(_) => None,
        })
        .collect()
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
        for local in uses(item.operation.inner()) {
            let Some(&d) = definer.get(&local) else {
                return Err(ImportError::UnknownReference { element: i, local });
            };
            if d != i {
                waiting[i] += 1;
                users[d].push(i);
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
    let id = Uuid::now_v7();
    let record = ImportRecord {
        id,
        author,
        author_subject: None,
        source: source.to_owned(),
        state: "ready".into(),
        report: serde_json::json!({}),
        batches: batches(items, batch_operations)?
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

/// Opens an import from `source`, `uploading` its chunks (REQ-006-17); it has no batch
/// until it starts.
///
/// # Errors
///
/// [`StoreError`] on infrastructure failure.
pub async fn open_import<S: Store>(
    store: &S,
    author: Uuid,
    source: &str,
) -> Result<Uuid, StoreError> {
    let id = Uuid::now_v7();
    let record = ImportRecord {
        id,
        author,
        author_subject: None,
        source: source.to_owned(),
        state: "uploading".into(),
        report: serde_json::json!({}),
        batches: Vec::new(),
    };
    store.create_import(&record).await?;
    Ok(id)
}

/// `items` ordered by dependency and cut in batches of at most `batch_operations`
/// operations, a creation counting two with its binding to its external identifier
/// (REQ-004-28).
///
/// # Errors
///
/// [`ImportError`] for elements that cannot be ordered.
pub fn batches(
    items: Vec<ImportItem>,
    batch_operations: usize,
) -> Result<Vec<Vec<ImportItem>>, ImportError> {
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
    Ok(batches)
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

/// The objects already known to the source among the locals `items` define or use.
async fn known<S: Store>(
    store: &S,
    source: &str,
    externals: &BTreeMap<u32, String>,
    items: &[ImportItem],
) -> Result<BTreeMap<u32, Uuid>, StoreError> {
    let mut locals: BTreeMap<String, u32> = BTreeMap::new();
    for item in items {
        let used = uses(item.operation.inner());
        for local in item.operation.defines().into_iter().chain(used) {
            if let Some(external) = externals.get(&local) {
                locals.insert(external.clone(), local);
            }
        }
    }
    let ids: Vec<String> = locals.keys().cloned().collect();
    let found = store.external_objects(source, &ids).await?;
    Ok(found
        .into_iter()
        .filter_map(|(external, id)| Some((*locals.get(&external)?, id)))
        .collect())
}

/// Applies the batches of an import not applied yet, in order, each as a changeset of its
/// author (REQ-004-28, 29): a creation already known to the source is left out (replay
/// without duplicates), references to objects of other batches become their
/// identifiers. A refused or held batch pauses the import with its report.
///
/// Only a `ready` import runs, or a `running` one whose instance stopped (REQ-006-18);
/// the state is read again before each batch, so a cancellation stops the import after
/// the batch in progress, leaving the batches applied (REQ-006-19).
///
/// # Errors
///
/// [`RequestError`] on infrastructure failure or a malformed batch.
#[allow(clippy::too_many_lines)] // one batch after the other, each recovered, built, submitted
pub async fn run_import<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    import: Uuid,
    max_operations: usize,
) -> Result<Option<ImportStatus>, RequestError> {
    const RUNNABLE: [&str; 2] = ["ready", "running"];
    let Some(record) = store.import(import).await? else {
        return Ok(None);
    };
    if !store
        .set_import(import, &RUNNABLE, "running", &serde_json::json!({}))
        .await?
    {
        return Ok(import_status(store, import).await?);
    }
    // Every local defined by the import, with its identifier in the source.
    let externals: BTreeMap<u32, String> = record
        .batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|i| Some((i.operation.defines()?, i.external_id.clone()?)))
        .collect();
    // The author's rights with the issuer and groups of its last start or resumption
    // (REQ-005-22), its identity alone without them.
    let author = record
        .author_subject
        .clone()
        .filter(|s| s.id == record.author)
        .unwrap_or_else(|| crate::authz::Subject::from(record.author));
    for batch in record.batches.iter().filter(|b| b.state != "applied") {
        let progress = store.import_progress(import).await?;
        if progress.is_none_or(|p| p.state != "running") {
            return Ok(import_status(store, import).await?);
        }
        // A batch applied before a stop kept it from being recorded: its changeset, found
        // by its motive, is recorded and nothing of it is replayed, not even the operations
        // that create nothing (an attachment), which the source does not know.
        let motive = format!("import {} batch {} ({import})", record.source, batch.index);
        if let Some(changeset) = store.applied_changeset(record.author, &motive).await? {
            store
                .set_batch(import, batch.index, "applied", Some(changeset))
                .await?;
            continue;
        }
        let known = known(store, &record.source, &externals, &batch.items).await?;
        let mut operations = Vec::new();
        for item in &batch.items {
            if let Some(local) = item.operation.defines() {
                if known.contains_key(&local) {
                    continue;
                }
                let kind = created_kind(&item.operation)
                    .or_else(|| created_module_kind(store.modules(), &item.operation))
                    .unwrap_or(ObjectKind::Address);
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
            motive,
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            // Spec 024: applied by its author who holds `approval.self`, pending otherwise.
            approval_self: true,
        };
        let report = match submit_in(
            store,
            authorizer,
            &Sources::none(),
            &RequestContext::of(&author),
            &request,
            max_operations,
        )
        .await?
        {
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
        store
            .set_import(import, &["running"], "paused", &report)
            .await?;
        return Ok(import_status(store, import).await?);
    }
    store
        .set_import(import, &["running"], "done", &serde_json::json!({}))
        .await?;
    Ok(import_status(store, import).await?)
}

/// Where an import stands (REQ-004-30), without reading its elements.
///
/// # Errors
///
/// [`StoreError`] on infrastructure failure.
pub async fn import_status<S: Store>(
    store: &S,
    import: Uuid,
) -> Result<Option<ImportStatus>, StoreError> {
    Ok(store
        .import_progress(import)
        .await?
        .map(|progress| ImportStatus {
            state: progress.state,
            applied: progress.applied,
            remaining: progress.batches - progress.applied,
            report: progress.report,
        }))
}
