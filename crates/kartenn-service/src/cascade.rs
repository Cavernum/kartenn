//! Explicit cascade of closures (spec 004, REQ-004-25, 26; task 004-15), and the
//! references the modules declare to the objects closed (spec 008, REQ-008-10; ADR 0026,
//! clause 12).
//!
//! A closure asked with `cascade` is preceded by the closure of what depends on the
//! object, recursively, and by the removal of the reference assignments that target
//! them; the objects of the modules that reference it are closed or cleared there, as
//! the policy of each reference says; everything then goes through the same rules and
//! rights. Without the cascade, or under a policy of refusal, a closure that leaves a
//! living object of a module referencing the object is refused, listing them.

use std::collections::BTreeSet;
use std::sync::Arc;

use kartenn_core::period::Period;
use kartenn_core::rules::{self, Violation};
use uuid::Uuid;

use crate::module::{ModuleHandler, ModuleRegistry, OnClose, Reference};
use crate::operations::{ModuleKind, ObjectKind};
use crate::pipeline::Planned;
use crate::port::{StoreError, WriteTx};

/// Operations with the index of the operation each comes from, their periods
/// (REQ-004-2), and the closures a cascade added.
type Expanded = (
    Vec<Planned>,
    Vec<usize>,
    Vec<Option<Period>>,
    Vec<(ObjectKind, Uuid)>,
);

/// A living object of a module that references a closed object, with the reference and the
/// handler of the module that declares it.
type Referrer = (Arc<dyn ModuleHandler>, &'static Reference, Uuid);

/// The living objects of the modules that reference `id`, of kind `kind`, from the start
/// of `period` on, by the references the modules declare to that kind (REQ-008-10).
pub(crate) async fn referrers<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    kind: ObjectKind,
    id: Uuid,
    period: Option<Period>,
) -> Result<Vec<Referrer>, StoreError> {
    let mut found = Vec::new();
    for module in modules.modules() {
        let references = module.references();
        if !references.iter().any(|r| r.to == kind.name()) {
            continue;
        }
        let Some(handler) = module.handler() else {
            continue;
        };
        for reference in references.iter().filter(|r| r.to == kind.name()) {
            for source in handler
                .referrers(tx.module_tx(), reference, id, period)
                .await?
            {
                found.push((Arc::clone(&handler), reference, source));
            }
        }
    }
    Ok(found)
}

/// The planned operations with every cascade expanded, each added operation on the period
/// of the closure that brings it and ascribed to its operation, and the closures added.
pub(crate) async fn expand<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    planned: Vec<Planned>,
    origins: Vec<usize>,
    periods: Vec<Option<Period>>,
) -> Result<Expanded, StoreError> {
    if !planned
        .iter()
        .any(|p| matches!(p, Planned::Close { cascade: true, .. }))
    {
        return Ok((planned, origins, periods, Vec::new()));
    }
    // Objects the changeset closes already, then those the cascade adds.
    let mut closed: BTreeSet<Uuid> = planned
        .iter()
        .filter_map(|p| match p {
            Planned::Close { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    let mut out = Vec::with_capacity(planned.len());
    let mut out_origins = Vec::with_capacity(planned.len());
    let mut out_periods = Vec::with_capacity(planned.len());
    let mut added = Vec::new();
    for ((p, origin), period) in planned.into_iter().zip(origins).zip(periods) {
        if let Planned::Close {
            kind,
            id,
            cascade: true,
            ..
        } = &p
        {
            let mut order = Vec::new();
            dependents(tx, modules, *kind, *id, period, &mut closed, &mut order).await?;
            let mut unassigned = BTreeSet::new();
            let mut cleared = BTreeSet::new();
            for (k, i) in order.iter().copied().chain(std::iter::once((*kind, *id))) {
                for (object, object_kind, property) in tx.referencing(i, period).await? {
                    if !closed.contains(&object) && unassigned.insert((object, property)) {
                        out.push(Planned::Unassign {
                            object,
                            kind: object_kind,
                            property,
                        });
                        out_origins.push(origin);
                        out_periods.push(period);
                    }
                }
                // The references of the modules whose policy clears them.
                for (handler, reference, source) in referrers(tx, modules, k, i, period).await? {
                    if reference.on_close != OnClose::Nullify
                        || closed.contains(&source)
                        || !cleared.insert((source, reference.from, i))
                    {
                        continue;
                    }
                    // Without a way to clear it, the reference stays: the closure is
                    // refused on the final state.
                    if let Some(plan) = handler.nullify(reference, source, i) {
                        out.push(Planned::Module {
                            handler,
                            plan: Arc::new(plan),
                            permission: None,
                        });
                        out_origins.push(origin);
                        out_periods.push(period);
                    }
                }
                if i != *id {
                    out.push(Planned::Close {
                        kind: k,
                        id: i,
                        expected: None,
                        cascade: false,
                    });
                    out_origins.push(origin);
                    out_periods.push(period);
                    added.push((k, i));
                }
            }
        }
        out.push(p);
        out_origins.push(origin);
        out_periods.push(period);
    }
    Ok((out, out_origins, out_periods, added))
}

/// The dependents of `(kind, id)`, deepest first, skipping those already closed: those of
/// the core, and the objects of the modules that reference them under a policy of cascade.
async fn dependents<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    kind: ObjectKind,
    id: Uuid,
    period: Option<Period>,
    closed: &mut BTreeSet<Uuid>,
    order: &mut Vec<(ObjectKind, Uuid)>,
) -> Result<(), StoreError> {
    // Iterative depth-first walk: (object, children visited).
    let mut stack = vec![(kind, id, false)];
    while let Some((k, i, visited)) = stack.pop() {
        if visited {
            if i != id {
                order.push((k, i));
            }
            continue;
        }
        stack.push((k, i, true));
        let mut children = tx.dependents(k, i, period).await?;
        for (_, reference, source) in referrers(tx, modules, k, i, period).await? {
            if reference.on_close == OnClose::Cascade
                && let Some(from) = ModuleKind::parse(reference.from)
            {
                children.push((ObjectKind::Module(from), source));
            }
        }
        for (child_kind, child) in children {
            if closed.insert(child) {
                stack.push((child_kind, child, false));
            }
        }
    }
    Ok(())
}

/// REQ-008-10: on the final state, no closed object is still referenced by a living object
/// of a module; the refusal lists them (with or without the cascade, which closes or clears
/// what its policy allows).
pub(crate) async fn check_referrers<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    planned: &[Planned],
    periods: &[Option<Period>],
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    if modules.is_empty() {
        return Ok(());
    }
    for (p, &period) in planned.iter().zip(periods) {
        let Planned::Close { kind, id, .. } = p else {
            continue;
        };
        let mut sources: Vec<Uuid> = referrers(tx, modules, *kind, *id, period)
            .await?
            .into_iter()
            .map(|(_, _, source)| source)
            .collect();
        sources.sort_unstable();
        sources.dedup();
        if !sources.is_empty() {
            violations.push(
                Violation::new(
                    &rules::OBJECT_DEPENDENTS,
                    format!(
                        "{} {id} is still referenced by {} object(s) of a module",
                        kind.name(),
                        sources.len()
                    ),
                )
                .with_subjects(std::iter::once(*id).chain(sources)),
            );
        }
    }
    Ok(())
}
