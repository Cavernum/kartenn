//! Explicit cascade of closures (spec 004, REQ-004-25, 26; task 004-15).
//!
//! A closure asked with `cascade` is preceded by the closure of what depends on the
//! object, recursively, and by the removal of the reference assignments that target
//! them; everything then goes through the same rules and rights.

use std::collections::BTreeSet;

use kartenn_core::period::Period;
use uuid::Uuid;

use crate::operations::ObjectKind;
use crate::pipeline::Planned;
use crate::port::{StoreError, WriteTx};

/// Operations with their periods (REQ-004-2), and the closures a cascade added.
type Expanded = (Vec<Planned>, Vec<Option<Period>>, Vec<(ObjectKind, Uuid)>);

/// The planned operations with every cascade expanded, each added operation on the period
/// of the closure that brings it, and the closures added.
pub(crate) async fn expand<T: WriteTx>(
    tx: &mut T,
    planned: Vec<Planned>,
    periods: Vec<Option<Period>>,
) -> Result<Expanded, StoreError> {
    if !planned
        .iter()
        .any(|p| matches!(p, Planned::Close { cascade: true, .. }))
    {
        return Ok((planned, periods, Vec::new()));
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
    let mut out_periods = Vec::with_capacity(planned.len());
    let mut added = Vec::new();
    for (p, period) in planned.into_iter().zip(periods) {
        if let Planned::Close {
            kind,
            id,
            cascade: true,
            ..
        } = &p
        {
            let mut order = Vec::new();
            dependents(tx, *kind, *id, period, &mut closed, &mut order).await?;
            let mut unassigned = BTreeSet::new();
            for (k, i) in order.iter().copied().chain(std::iter::once((*kind, *id))) {
                for (object, object_kind, property) in tx.referencing(i, period).await? {
                    if !closed.contains(&object) && unassigned.insert((object, property)) {
                        out.push(Planned::Unassign {
                            object,
                            kind: object_kind,
                            property,
                        });
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
                    out_periods.push(period);
                    added.push((k, i));
                }
            }
        }
        out.push(p);
        out_periods.push(period);
    }
    Ok((out, out_periods, added))
}

/// The dependents of `(kind, id)`, deepest first, skipping those already closed.
async fn dependents<T: WriteTx>(
    tx: &mut T,
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
        for (child_kind, child) in tx.dependents(k, i, period).await? {
            if closed.insert(child) {
                stack.push((child_kind, child, false));
            }
        }
    }
    Ok(())
}
