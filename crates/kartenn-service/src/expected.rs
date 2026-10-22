//! Sources of expected values (spec 003, REQ-003-21 to 24; task 003-13; P5, D23, D24;
//! version 2: spec 016, REQ-016-4, 5, G8).
//!
//! A source computes, from a frozen view of an object (core object and resolved
//! properties) and the state of the transaction under way, the values it expects for a
//! property: none, one or several candidates, each with an explanation. Kartenn uses them
//! three ways, per declaration: suggestions (every candidate), default values at creation
//! and anomalies when the resolved value differs (one candidate only). Verdicts always use
//! the resolved value (D23).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use kartenn_core::period::{Instant, Period};
use kartenn_core::properties::{Declaration, ExpectedMode};
use kartenn_core::rules::{self, Violation};
use kartenn_core::values::Raw;
use uuid::Uuid;

use crate::module::{BoxFuture, ModuleTx};
use crate::operations::{ObjectKind, RawInput};
use crate::pipeline::Planned;
use crate::port::{ObjectView, Store, StoreError, WriteError, WriteTx};

/// A value a source expects, as raw input, with why (REQ-003-21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// One value, or the elements of a set.
    pub values: Vec<RawInput>,
    /// Explanation (rule applied, evidence).
    pub explanation: String,
    /// What, within the source, gave the value (a rule), recorded in the provenance
    /// `source:<id>@<revision>#<detail>` of a default value (REQ-016-5).
    pub detail: Option<String>,
}

/// What a source expects for one object (REQ-016-5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Expected {
    /// Nothing.
    #[default]
    None,
    /// One candidate: a default value, and a deviation when the resolved value differs.
    One(Suggestion),
    /// Several candidates: neither default nor deviation; all of them are suggested.
    Candidates(Vec<Suggestion>),
}

impl Expected {
    /// The single candidate, if there is exactly one.
    #[must_use]
    pub fn single(&self) -> Option<&Suggestion> {
        match self {
            Self::One(s) => Some(s),
            Self::Candidates(c) if c.len() == 1 => c.first(),
            Self::None | Self::Candidates(_) => None,
        }
    }

    /// Every candidate.
    #[must_use]
    pub fn candidates(&self) -> &[Suggestion] {
        match self {
            Self::None => &[],
            Self::One(s) => std::slice::from_ref(s),
            Self::Candidates(c) => c,
        }
    }
}

/// The dates of an evaluation (REQ-016-4). Beside it a source receives the transaction
/// under way, so that the rules a changeset writes apply to the objects it creates; the
/// resolved values of the ancestors of an object are read on that transaction by the
/// grouped read of the adapter (REQ-016-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExpectedContext {
    /// Valid time of the evaluation; `None` for now (the transaction's own state).
    pub valid_at: Option<Instant>,
}

/// A source of expected values, compiled into a module (REQ-003-21, P5; REQ-016-4, 5).
pub trait ExpectedValueSource: Send + Sync {
    /// Identifier, as declarations name it.
    fn id(&self) -> &str;

    /// Whether an object created without its own assignment receives the default value
    /// even when it inherits a value (REQ-003-22, the default); a source that returns
    /// `false` gives a default only to an object that inherits nothing (REQ-010-52).
    fn defaults_when_inheriting(&self) -> bool {
        true
    }

    /// Revision, recorded in the provenance of the default values it gives and compared to
    /// the last evaluation of its deviations; computed at the call (the rules of a module
    /// change with its data).
    ///
    /// # Errors
    ///
    /// Infrastructure failure.
    fn revision<'a>(
        &'a self,
        ctx: ExpectedContext,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<String, StoreError>>;

    /// What it expects for `property` (its key) on `object`.
    ///
    /// # Errors
    ///
    /// Infrastructure failure.
    fn expected<'a>(
        &'a self,
        property: &'a str,
        object: &'a ObjectView,
        ctx: ExpectedContext,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Expected, StoreError>>;

    /// What it expects for `property` on each of `objects`, in their order, in one call
    /// (REQ-016-4): one [`Self::expected`] per object unless the source reads them at once.
    ///
    /// # Errors
    ///
    /// Infrastructure failure.
    fn expected_many<'a>(
        &'a self,
        property: &'a str,
        objects: &'a [ObjectView],
        ctx: ExpectedContext,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Vec<Expected>, StoreError>> {
        Box::pin(async move {
            let mut out = Vec::with_capacity(objects.len());
            for object in objects {
                out.push(self.expected(property, object, ctx, &mut *tx).await?);
            }
            Ok(out)
        })
    }
}

/// The sources available to the pipeline, by identifier.
#[derive(Clone, Default)]
pub struct Sources {
    by_id: BTreeMap<String, Arc<dyn ExpectedValueSource>>,
}

impl fmt::Debug for Sources {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.by_id.keys()).finish()
    }
}

impl Sources {
    /// No source (v0.1 ships none).
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Adds a source.
    #[must_use]
    pub fn with(mut self, source: Arc<dyn ExpectedValueSource>) -> Self {
        self.by_id.insert(source.id().to_owned(), source);
        self
    }

    /// The source of an identifier.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Arc<dyn ExpectedValueSource>> {
        self.by_id.get(id)
    }
}

/// Declarations naming a source the registry does not hold are refused (local check).
pub(crate) fn check_declared_sources(
    planned: &[Planned],
    origins: &[usize],
    sources: &Sources,
    violations: &mut Vec<Violation>,
) {
    for (p, &origin) in planned.iter().zip(origins) {
        if let Planned::Declare { declaration, .. } | Planned::UpdateDeclaration { declaration, .. } =
            p
            && let Some(expected) = &declaration.expected
            && sources.get(&expected.source).is_none()
            // A module names a source of its own, which it provides with its code: its
            // synchronisation runs before the sources of the deployment are assembled.
            && !matches!(&declaration.declarant, kartenn_core::properties::Declarant::Module(m)
                if expected.source.strip_prefix(m.as_str()).is_some_and(|s| s.starts_with('.')))
        {
            violations.push(
                Violation::new(
                    &rules::PROP_INVALID_DECLARATION,
                    format!(
                        "{}: no source of expected values {}",
                        declaration.key, expected.source
                    ),
                )
                .at(origin),
            );
        }
    }
}

/// Kind of a created object.
fn created(planned: &[Planned]) -> Vec<(Uuid, ObjectKind)> {
    planned
        .iter()
        .filter_map(|p| match p {
            Planned::Domain { id, .. } => Some((*id, ObjectKind::Domain)),
            Planned::Prefix { object, .. } => Some((object.id, ObjectKind::Prefix)),
            Planned::Range { object, .. } => Some((object.id, ObjectKind::Range)),
            Planned::Address { object, .. } => Some((object.id, ObjectKind::Address)),
            _ => None,
        })
        .collect()
}

fn raws(values: &[RawInput]) -> Vec<Raw> {
    values.iter().map(RawInput::to_raw).collect()
}

/// The provenance of a default value: `source:<id>@<revision>`, and `#<detail>` when the
/// source names what gave it (REQ-016-5).
fn provenance(source: &str, revision: &str, suggestion: &Suggestion) -> String {
    match &suggestion.detail {
        Some(detail) => format!("source:{source}@{revision}#{detail}"),
        None => format!("source:{source}@{revision}"),
    }
}

/// The views of `objects` at the start of `period`, with their kinds; objects without a
/// view are left out.
async fn views<T: WriteTx>(
    tx: &mut T,
    objects: &[(Uuid, Option<ObjectKind>)],
    period: Option<Period>,
) -> Result<Vec<(ObjectView, Option<ObjectKind>)>, StoreError> {
    // Read together: a changeset of 10⁴ objects read one by one took a second (REQ-010-54).
    let ids: Vec<Uuid> = objects.iter().map(|(o, _)| *o).collect();
    let kinds: BTreeMap<Uuid, Option<ObjectKind>> = objects.iter().copied().collect();
    Ok(tx
        .object_views(&ids, period)
        .await?
        .into_iter()
        .map(|view| {
            let kind = kinds.get(&view.id).copied().flatten();
            (view, kind)
        })
        .collect())
}

/// Default values (REQ-003-22, REQ-016-5): every created object of a carrier kind without
/// an assignment of the property in the changeset receives the expected value when the
/// source gives exactly one candidate, with the provenance `source:<id>@<revision>`. The
/// objects of a property are evaluated in one call. Returns the assignments written, to
/// resolve them like the others.
pub(crate) async fn defaults<T: WriteTx>(
    tx: &mut T,
    sources: &Sources,
    planned: &[Planned],
    period: Option<Period>,
    changeset: Uuid,
) -> Result<Vec<Planned>, WriteError> {
    let created = created(planned);
    if created.is_empty() {
        return Ok(Vec::new());
    }
    let assigned: BTreeSet<(Uuid, Uuid)> = planned
        .iter()
        .filter_map(|p| match p {
            Planned::Assign {
                object, property, ..
            }
            | Planned::Unassign {
                object, property, ..
            } => Some((*object, *property)),
            _ => None,
        })
        .collect();
    let mut written = Vec::new();
    for (property, declaration) in tx.with_expected(period).await? {
        let Some(expected) = &declaration.expected else {
            continue;
        };
        let Some(source) = sources.get(&expected.source) else {
            continue;
        };
        if expected.mode != ExpectedMode::Default {
            continue;
        }
        let wanted: Vec<(Uuid, Option<ObjectKind>)> = created
            .iter()
            .filter(|(object, kind)| {
                declaration.carriers.contains(kind.name())
                    && !assigned.contains(&(*object, property))
            })
            .map(|(o, k)| (*o, Some(*k)))
            .collect();
        let key = declaration.key.to_string();
        // For a source that leaves inheriting objects alone (REQ-010-52), the origins of
        // the objects just created are materialised first, so that their view shows what
        // they inherit; the maintenance after the defaults recomputes them.
        let skip_inheriting = !source.defaults_when_inheriting();
        if skip_inheriting && let Some(tree) = &declaration.inherit {
            let targets: Vec<Uuid> = wanted.iter().map(|(o, _)| *o).collect();
            if !targets.is_empty() {
                tx.refresh_origins(property, tree, &targets, period, changeset)
                    .await?;
            }
        }
        let found: Vec<_> = views(tx, &wanted, period)
            .await?
            .into_iter()
            .filter(|(view, _)| {
                !skip_inheriting
                    || declaration.inherit.is_none()
                    || !matches!(view.properties.get(&key), Some(Some(v)) if v != "null" && v != "[]")
            })
            .collect();
        if found.is_empty() {
            continue;
        }
        let objects: Vec<ObjectView> = found.iter().map(|(v, _)| v.clone()).collect();
        let ctx = ExpectedContext {
            valid_at: period.map(Period::start),
        };
        let revision = source.revision(ctx, tx.module_tx()).await?;
        let answers = source
            .expected_many(&key, &objects, ctx, tx.module_tx())
            .await?;
        for ((view, kind), answer) in found.iter().zip(answers) {
            let (Some(suggestion), Some(kind)) = (answer.single(), kind) else {
                continue;
            };
            let values = raws(&suggestion.values);
            crate::resolution::assign(
                tx,
                view.id,
                *kind,
                property,
                Some(&values),
                &provenance(source.id(), &revision, suggestion),
                period,
                changeset,
            )
            .await?;
            written.push(Planned::Assign {
                object: view.id,
                kind: *kind,
                property,
                values: Some(values),
            });
        }
    }
    Ok(written)
}

/// Deviations (REQ-003-24, REQ-016-5): on every touched object of a carrier kind, when the
/// source gives exactly one candidate and the resolved value differs, a warning citing the
/// property, both values and the explanation; it becomes an anomaly of the object. The
/// objects of a property are evaluated in one call. Returns the objects evaluated.
pub(crate) async fn check<T: WriteTx>(
    tx: &mut T,
    sources: &Sources,
    touched: &[Uuid],
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<Vec<Uuid>, StoreError> {
    let checked: Vec<(Uuid, Declaration)> = tx
        .with_expected(period)
        .await?
        .into_iter()
        .filter(|(_, d)| {
            d.expected
                .as_ref()
                .is_some_and(|e| e.mode == ExpectedMode::Check)
        })
        .collect();
    if checked.is_empty() {
        return Ok(Vec::new());
    }
    let touched: Vec<(Uuid, Option<ObjectKind>)> = touched.iter().map(|o| (*o, None)).collect();
    let all: Vec<ObjectView> = views(tx, &touched, period)
        .await?
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let evaluated: Vec<Uuid> = all.iter().map(|v| v.id).collect();
    for (property, declaration) in &checked {
        let key = declaration.key.to_string();
        let Some(source) = declaration
            .expected
            .as_ref()
            .and_then(|e| sources.get(&e.source))
        else {
            continue;
        };
        let objects: Vec<ObjectView> = all
            .iter()
            .filter(|v| declaration.carriers.contains(&v.kind))
            .cloned()
            .collect();
        if objects.is_empty() {
            continue;
        }
        let ctx = ExpectedContext {
            valid_at: period.map(Period::start),
        };
        let revision = source.revision(ctx, tx.module_tx()).await?;
        let answers = source
            .expected_many(&key, &objects, ctx, tx.module_tx())
            .await?;
        for (view, answer) in objects.iter().zip(answers) {
            let Some(suggestion) = answer.single() else {
                continue;
            };
            let expected = match crate::resolution::parse(
                tx,
                declaration,
                *property,
                view.id,
                &raws(&suggestion.values),
                period,
            )
            .await
            {
                Ok(value) => value,
                Err(WriteError::Store(e)) => return Err(e),
                // A source proposing an invalid value expects nothing usable.
                Err(_) => continue,
            };
            let resolved = tx
                .resolved_value(view.id, *property, declaration, period)
                .await?;
            if resolved.as_ref() != Some(&expected) {
                let shown = view.properties.get(&key).cloned().flatten();
                violations.push(
                    Violation::new(
                        &rules::PROP_EXPECTED_MISMATCH,
                        format!(
                            "{key}: resolved {}, expected {:?} by {}@{revision} ({})",
                            shown.as_deref().unwrap_or("no value"),
                            suggestion.values,
                            source.id(),
                            suggestion.explanation
                        ),
                    )
                    .with_subjects([view.id]),
                );
            }
        }
    }
    Ok(evaluated)
}

/// What a property's source expects for an object at the start of `period`, written
/// nowhere (REQ-003-23, REQ-016-5): every candidate. [`Expected::None`] without a source,
/// an object or an expectation.
///
/// # Errors
///
/// Infrastructure failure.
pub async fn suggest<S: Store>(
    store: &S,
    sources: &Sources,
    property: Uuid,
    object: Uuid,
    period: Option<Period>,
) -> Result<Expected, StoreError> {
    let mut tx = store.begin().await?;
    let declaration = tx.declaration(property, period).await?;
    let view = tx.object_view(object, period).await?;
    let found = match (declaration, view) {
        (Some(declaration), Some(view)) => {
            let ctx = ExpectedContext {
                valid_at: period.map(Period::start),
            };
            suggest_for(
                sources,
                Some(&declaration),
                Some(&view),
                ctx,
                tx.module_tx(),
            )
            .await?
        }
        _ => Expected::None,
    };
    tx.rollback().await?;
    Ok(found)
}

/// What a property's source expects for an object being entered (not stored yet),
/// described by its view (REQ-003-23).
///
/// # Errors
///
/// Infrastructure failure.
pub async fn suggest_for(
    sources: &Sources,
    declaration: Option<&Declaration>,
    view: Option<&ObjectView>,
    ctx: ExpectedContext,
    tx: &mut dyn ModuleTx,
) -> Result<Expected, StoreError> {
    let (Some(declaration), Some(view)) = (declaration, view) else {
        return Ok(Expected::None);
    };
    let Some(source) = declaration
        .expected
        .as_ref()
        .and_then(|e| sources.get(&e.source))
    else {
        return Ok(Expected::None);
    };
    source
        .expected(&declaration.key.to_string(), view, ctx, tx)
        .await
}

/// Re-evaluates the deviations of every source of the registry whose revision changed since
/// the last evaluation (REQ-003-24): all carrier objects of its properties in `check`
/// mode, by batches of `batch` objects (one call of the source per property and batch,
/// REQ-016-4), each journaled as a system changeset that opens and closes the anomalies;
/// then records the revision. Returns the objects evaluated.
///
/// # Errors
///
/// [`RequestError`] on infrastructure failure.
pub async fn reevaluate_outdated<S: Store>(
    store: &S,
    sources: &Sources,
    batch: usize,
) -> Result<usize, crate::RequestError> {
    let mut tx = store.begin().await?;
    let mut outdated: BTreeMap<String, (String, BTreeSet<String>)> = BTreeMap::new();
    for (_, declaration) in tx.with_expected(None).await? {
        let Some(expected) = declaration
            .expected
            .filter(|e| e.mode == ExpectedMode::Check)
        else {
            continue;
        };
        let Some(source) = sources.get(&expected.source) else {
            continue;
        };
        let revision = source
            .revision(ExpectedContext::default(), tx.module_tx())
            .await?;
        if tx.source_version(source.id()).await?.as_deref() == Some(revision.as_str()) {
            continue;
        }
        outdated
            .entry(source.id().to_owned())
            .or_insert_with(|| (revision, BTreeSet::new()))
            .1
            .extend(declaration.carriers);
    }
    tx.rollback().await?;
    let mut evaluated = 0;
    for (source, (version, carriers)) in outdated {
        let mut tx = store.begin().await?;
        let carriers: Vec<String> = carriers.into_iter().collect();
        let objects = tx.objects_of_kinds(&carriers, None).await?;
        tx.rollback().await?;
        for chunk in objects.chunks(batch.max(1)) {
            let mut tx = store.begin().await?;
            let mut violations = Vec::new();
            let checked = check(&mut tx, sources, chunk, None, &mut violations).await?;
            let request = crate::ChangesetRequest {
                author: crate::declarations::SYSTEM_AUTHOR,
                motive: format!("expected re-evaluation {source}@{version}"),
                period: None,
                operations: Vec::new(),
                simulate: false,
                idempotency_key: None,
            };
            let reevaluated: Vec<(Uuid, &'static str)> = checked
                .iter()
                .map(|o| (*o, rules::PROP_EXPECTED_MISMATCH.id))
                .collect();
            let journal = crate::port::Journal {
                id: Uuid::now_v7(),
                request: &request,
                state: "applied",
                kind: "direct",
                validator: None,
                pending: None,
                violations: &violations,
                domains: &[],
                keys: &[],
                objects: &checked,
                reevaluated: &reevaluated,
                diff: &[],
                cascaded: &[],
                audit: &[],
                on_behalf_of: None,
            };
            tx.journal_applied(&journal).await?;
            tx.commit().await?;
            evaluated += checked.len();
        }
        let mut tx = store.begin().await?;
        tx.set_source_version(&source, &version).await?;
        tx.commit().await?;
    }
    Ok(evaluated)
}
