//! Inverse changesets (spec 004, REQ-004-34, 35; task 004-20; 004/design, § Annulation).

use uuid::Uuid;

use crate::operations::{ChangesetRequest, ObjectKind, Operation, PeriodSpec, Ref};
use crate::port::{Store, StoreError};

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

/// The inverse of a changeset: a request to review and submit, its conflicts, and what it
/// does not undo (declarations, closed-list values).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inverse {
    /// The request, not submitted.
    pub request: ChangesetRequest,
    /// Objects left alone.
    pub conflicts: Vec<UndoConflict>,
    /// Kinds and identifiers not undone.
    pub unsupported: Vec<(String, Uuid)>,
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
        },
        conflicts,
        unsupported: facts.unsupported,
    }))
}
