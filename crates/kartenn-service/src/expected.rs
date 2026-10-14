//! Sources of expected values (spec 003, REQ-003-21 to 24; task 003-13; P5, D23, D24).
//!
//! A source computes, from a frozen view of an object (core object and resolved
//! properties), the value it expects for a property, with an explanation. Kartenn uses it
//! three ways, per declaration: suggestions, default values at creation, and anomalies
//! when the resolved value differs. Verdicts always use the resolved value (D23).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use kartenn_core::period::Period;
use kartenn_core::properties::{Declaration, ExpectedMode};
use kartenn_core::rules::{self, Violation};
use kartenn_core::values::Raw;
use uuid::Uuid;

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
}

/// A source of expected values, compiled into a module (REQ-003-21, P5).
pub trait ExpectedValueSource: Send + Sync {
    /// Identifier, as declarations name it.
    fn id(&self) -> &str;
    /// Version, recorded in the provenance of the default values it gives.
    fn version(&self) -> &str;
    /// The value expected for `property` (its key) on `object`, if any.
    fn expected(&self, property: &str, object: &ObjectView) -> Option<Suggestion>;
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
    sources: &Sources,
    violations: &mut Vec<Violation>,
) {
    for p in planned {
        if let Planned::Declare { declaration, .. } | Planned::UpdateDeclaration { declaration, .. } =
            p
            && let Some(expected) = &declaration.expected
            && sources.get(&expected.source).is_none()
        {
            violations.push(Violation::new(
                &rules::PROP_INVALID_DECLARATION,
                format!(
                    "{}: no source of expected values {}",
                    declaration.key, expected.source
                ),
            ));
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

/// Default values (REQ-003-22): every created object of a carrier kind without an
/// assignment of the property in the changeset receives the expected value, with the
/// provenance `source:<id>@<version>`. Returns the assignments written, to resolve them
/// like the others.
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
        for (object, kind) in &created {
            if !declaration.carriers.contains(kind.name())
                || assigned.contains(&(*object, property))
            {
                continue;
            }
            let Some(view) = tx.object_view(*object, period).await? else {
                continue;
            };
            let Some(suggestion) = source.expected(&declaration.key.to_string(), &view) else {
                continue;
            };
            let values = raws(&suggestion.values);
            let provenance = format!("source:{}@{}", source.id(), source.version());
            crate::resolution::assign(
                tx,
                *object,
                *kind,
                property,
                Some(&values),
                &provenance,
                period,
                changeset,
            )
            .await?;
            written.push(Planned::Assign {
                object: *object,
                kind: *kind,
                property,
                values: Some(values),
            });
        }
    }
    Ok(written)
}

/// Deviations (REQ-003-24): on every touched object of a carrier kind, a resolved value
/// that differs from the expected one is a warning citing the property, both values and
/// the explanation; it becomes an anomaly of the object. Returns the objects evaluated.
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
    let mut evaluated = Vec::new();
    for object in touched {
        let Some(view) = tx.object_view(*object, period).await? else {
            continue;
        };
        evaluated.push(*object);
        for (property, declaration) in &checked {
            let key = declaration.key.to_string();
            let source = declaration
                .expected
                .as_ref()
                .and_then(|e| sources.get(&e.source));
            let (Some(source), true) = (source, declaration.carriers.contains(&view.kind)) else {
                continue;
            };
            let Some(suggestion) = source.expected(&key, &view) else {
                continue;
            };
            let expected = match crate::resolution::parse(
                tx,
                declaration,
                *property,
                *object,
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
                .resolved_value(*object, *property, declaration, period)
                .await?;
            if resolved.as_ref() != Some(&expected) {
                let shown = view.properties.get(&key).cloned().flatten();
                violations.push(
                    Violation::new(
                        &rules::PROP_EXPECTED_MISMATCH,
                        format!(
                            "{key}: resolved {}, expected {:?} by {}@{} ({})",
                            shown.as_deref().unwrap_or("no value"),
                            suggestion.values,
                            source.id(),
                            source.version(),
                            suggestion.explanation
                        ),
                    )
                    .with_subjects([*object]),
                );
            }
        }
    }
    Ok(evaluated)
}

/// The value a property's source expects for an object at the start of `period`, written
/// nowhere (REQ-003-23). `None` without a source, an object or an expectation.
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
) -> Result<Option<Suggestion>, StoreError> {
    let mut tx = store.begin().await?;
    let declaration = tx.declaration(property, period).await?;
    let view = tx.object_view(object, period).await?;
    tx.rollback().await?;
    Ok(suggest_for(sources, declaration.as_ref(), view.as_ref()))
}

/// The value a property's source expects for an object being entered (not stored yet),
/// described by its view (REQ-003-23).
#[must_use]
pub fn suggest_for(
    sources: &Sources,
    declaration: Option<&Declaration>,
    view: Option<&ObjectView>,
) -> Option<Suggestion> {
    let declaration = declaration?;
    let source = sources.get(&declaration.expected.as_ref()?.source)?;
    source.expected(&declaration.key.to_string(), view?)
}
