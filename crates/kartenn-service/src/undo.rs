//! Inverse changesets (spec 004, REQ-004-34, 35; task 004-20; 004/design, § Annulation),
//! administration of the RBAC included (spec 005, REQ-005-25, K16; task 005-18).

use uuid::Uuid;

use crate::operations::{ChangesetRequest, ObjectKind, Operation, PeriodSpec, Ref};
use crate::port::{RbacBefore, RbacRow, RbacState, RbacTouched, Store, StoreError};

/// An object or assignment the inverse leaves alone, because a later changeset wrote it
/// over the period (REQ-004-35).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoConflict {
    /// Kind of the object.
    pub kind: ObjectKind,
    /// Object.
    pub id: Uuid,
    /// The property, for an assignment.
    pub property: Option<Uuid>,
    /// The later changeset.
    pub by: Uuid,
}

/// A role, a binding or an issuer attachment the inverse leaves alone, because a later
/// changeset wrote it (or bound a role it would close) (spec 005, K16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacConflict {
    /// Which one.
    pub row: RbacRow,
    /// The later changeset.
    pub by: Uuid,
}

/// The inverse of a changeset: a request to review and submit, its conflicts, and what it
/// does not undo (declarations, closed-list values, service accounts, identity states).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inverse {
    /// The request, not submitted.
    pub request: ChangesetRequest,
    /// Objects left alone.
    pub conflicts: Vec<UndoConflict>,
    /// Roles, bindings and attachments left alone.
    pub rbac_conflicts: Vec<RbacConflict>,
    /// Kinds and identifiers not undone.
    pub unsupported: Vec<(String, Uuid)>,
}

/// `op` over `period` from `start` on: plain when that is the period of the inverse,
/// wrapped otherwise.
fn over(op: Operation, period: PeriodSpec, start: i64) -> Operation {
    let period = PeriodSpec {
        start: period.start.max(start),
        end: period.end,
    };
    if period.start == start && period.end.is_none() {
        op
    } else {
        Operation::Within {
            period,
            operation: Box::new(op),
        }
    }
}

/// The operations that undo the administration a changeset wrote (spec 005, K16), in an
/// order that keeps each one valid: removals, closed roles, roles given back, then the
/// bindings and attachments given back (a binding finds its role).
fn rbac_inverse(
    touched: Vec<RbacTouched>,
    start: i64,
    conflicts: &mut Vec<RbacConflict>,
) -> Vec<Operation> {
    let mut stages: [Vec<Operation>; 4] = Default::default();
    for t in touched {
        if let Some(by) = t.later {
            conflicts.push(RbacConflict { row: t.row, by });
            continue;
        }
        let (stage, op) = match (t.row, t.before, t.after) {
            (RbacRow::Binding { id }, None, true) => (0, Operation::Unbind { id }),
            (RbacRow::Attachment { issuer, entity }, None, true) => (
                0,
                Operation::DetachIssuer {
                    issuer,
                    entity: entity.map(Ref::Id),
                },
            ),
            (RbacRow::Role { key }, None, true) => (1, Operation::CloseRole { key }),
            (
                RbacRow::Role { key },
                Some(RbacBefore {
                    period,
                    state: RbacState::Role { labels, entries },
                }),
                _,
            ) => (
                2,
                over(
                    Operation::SetRole {
                        key,
                        labels,
                        entries,
                    },
                    period,
                    start,
                ),
            ),
            (
                RbacRow::Binding { .. },
                Some(RbacBefore {
                    period,
                    state:
                        RbacState::Binding {
                            subject,
                            role,
                            scope,
                        },
                }),
                false,
            ) => {
                let bind = Operation::Bind {
                    local: None,
                    subject,
                    role,
                    scope,
                };
                (3, over(bind, period, start))
            }
            (RbacRow::Attachment { issuer, entity }, Some(RbacBefore { period, .. }), false) => {
                let attach = Operation::AttachIssuer {
                    issuer,
                    entity: entity.map(Ref::Id),
                };
                (3, over(attach, period, start))
            }
            // Created then ended by the changeset, or still in force as before.
            _ => continue,
        };
        stages[stage].push(op);
    }
    stages.into_iter().flatten().collect()
}

/// The inverse of the applied changeset `changeset`, authored by `author`; `None` if it is
/// not applied (REQ-004-34). An object it created is closed (an assignment removed);
/// another is restored as before it; one written since by another changeset is a
/// conflict, left out.
///
/// # Errors
///
/// [`StoreError`] on infrastructure failure.
pub async fn inverse<S: Store>(
    store: &S,
    changeset: Uuid,
    author: Uuid,
) -> Result<Option<Inverse>, StoreError> {
    let Some(facts) = store.undo_facts(changeset).await? else {
        return Ok(None);
    };
    let mut operations = Vec::new();
    let mut conflicts = Vec::new();
    for t in facts.touched {
        if let Some(by) = t.later {
            conflicts.push(UndoConflict {
                kind: t.kind,
                id: t.id,
                property: t.property,
                by,
            });
            continue;
        }
        operations.push(match (t.created, t.property) {
            (true, None) => Operation::Close {
                kind: t.kind,
                id: Ref::Id(t.id),
                expected: None,
                cascade: false,
            },
            (true, Some(property)) => Operation::UnassignProperty {
                object: Ref::Id(t.id),
                kind: t.kind,
                property: Ref::Id(property),
            },
            (false, property) => Operation::Restore {
                kind: t.kind,
                id: Ref::Id(t.id),
                property,
                before: changeset,
            },
        });
    }
    let mut rbac_conflicts = Vec::new();
    operations.extend(rbac_inverse(facts.rbac, facts.start, &mut rbac_conflicts));
    Ok(Some(Inverse {
        request: ChangesetRequest {
            author,
            motive: format!("undo changeset {changeset}"),
            period: Some(PeriodSpec {
                start: facts.start,
                end: None,
            }),
            operations,
            simulate: false,
            idempotency_key: None,
            // Spec 024, REQ-024-13: applied by its author who may, pending otherwise.
            apply_own: true,
        },
        conflicts,
        rbac_conflicts,
        unsupported: facts.unsupported,
    }))
}
