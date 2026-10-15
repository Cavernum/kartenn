//! Pending changesets: approval policies, proposals, and their decisions (spec 004,
//! REQ-004-19 to 24, 39 to 43; tasks 004-11 to 14; 004/design, § Approbation et
//! propositions).

use std::collections::BTreeSet;

use kartenn_core::rules::{self, Violation};
use uuid::Uuid;

use crate::authz::Authorizer;
use crate::declarations::SYSTEM_AUTHOR;
use crate::expected::Sources;
use crate::operations::ChangesetRequest;
use crate::pipeline::{APPROVE_PERMISSION, Mode, Outcome, PendingKind, Planned, RequestError, run};
use crate::port::{ApprovalPolicy, DiffCount, PendingChangeset, Store, StoreError, WriteTx};

/// What a changeset shows to the approval policies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exposure {
    /// Number of operations.
    pub operations: usize,
    /// Objects of the differential, counted per property.
    pub diff_objects: u64,
    /// Properties assigned or changed by effect.
    pub property_keys: BTreeSet<String>,
    /// Addressing domains touched.
    pub domains: BTreeSet<Uuid>,
    /// The author.
    pub author: Uuid,
}

/// Whether `policy` holds a changeset: every trigger it defines matches, and it defines
/// at least one (REQ-004-40).
#[must_use]
pub fn holds(policy: &ApprovalPolicy, exposure: &Exposure) -> bool {
    let triggers = [
        policy
            .min_operations
            .map(|m| exposure.operations >= m as usize),
        policy.min_diff_objects.map(|m| exposure.diff_objects >= m),
        (!policy.property_keys.is_empty()).then(|| {
            policy
                .property_keys
                .iter()
                .any(|k| exposure.property_keys.contains(k))
        }),
        (!policy.domains.is_empty())
            .then(|| policy.domains.iter().any(|d| exposure.domains.contains(d))),
        (!policy.authors.is_empty()).then(|| policy.authors.contains(&exposure.author)),
    ];
    policy.enabled && triggers.iter().any(Option::is_some) && triggers.iter().flatten().all(|t| *t)
}

/// The first of `policies` (enabled, by name, as in force at submission) that holds the
/// changeset; never for the system author (P12), and none by default (REQ-004-39).
pub(crate) async fn holding_policy<T: WriteTx>(
    tx: &mut T,
    policies: &[ApprovalPolicy],
    req: &ChangesetRequest,
    planned: &[Planned],
    diff: &[DiffCount],
    domains: &[Uuid],
) -> Result<Option<Uuid>, StoreError> {
    if req.author == SYSTEM_AUTHOR {
        return Ok(None);
    }
    if policies.is_empty() {
        return Ok(None);
    }
    let mut property_keys: BTreeSet<String> = diff.iter().map(|d| d.property.clone()).collect();
    for p in planned {
        if let Planned::Assign { property, .. }
        | Planned::Unassign { property, .. }
        | Planned::Restore {
            property: Some(property),
            ..
        } = p
            && let Some(declaration) = tx.declaration(*property, None).await?
        {
            property_keys.insert(declaration.key.to_string());
        }
    }
    let exposure = Exposure {
        operations: req.operations.len(),
        diff_objects: diff.iter().map(|d| d.count).sum(),
        property_keys,
        domains: domains.iter().copied().collect(),
        author: req.author,
    };
    Ok(policies.iter().find(|p| holds(p, &exposure)).map(|p| p.id))
}

fn refused(rule: &'static rules::Rule, message: String, id: Uuid) -> RequestError {
    RequestError::Review(Violation::new(rule, message).with_subjects([id]))
}

/// A pending changeset, or why it cannot be decided.
async fn pending<S: Store>(store: &S, id: Uuid) -> Result<PendingChangeset, RequestError> {
    match store.pending(id).await? {
        Some(p) if p.state == "pending" => Ok(p),
        _ => Err(refused(
            &rules::CHANGESET_NOT_PENDING,
            format!("changeset {id} is not pending"),
            id,
        )),
    }
}

fn kind(found: &PendingChangeset) -> PendingKind {
    if found.kind == "proposal" {
        PendingKind::Proposal
    } else {
        PendingKind::Approval
    }
}

/// Whether `validator` may decide a pending changeset: the approval permission on its
/// perimeter for an approval, the write permissions it requires for a proposal.
async fn may_decide<A: Authorizer>(
    authorizer: &A,
    found: &PendingChangeset,
    validator: Uuid,
) -> Result<Vec<Uuid>, StoreError> {
    match kind(found) {
        PendingKind::Approval => {
            authorizer
                .authorize_many(validator, APPROVE_PERMISSION, &found.pending.perimeter)
                .await
        }
        PendingKind::Proposal => {
            let mut denied = Vec::new();
            for (permission, objects) in &found.pending.required {
                denied.extend(
                    authorizer
                        .authorize_many(validator, permission, objects)
                        .await?,
                );
            }
            Ok(denied)
        }
    }
}

/// Validates a pending changeset (REQ-004-21, 22, 42): its operations run again on the
/// current state under the rights of `validator`; `Applied`, or `FailedRevalidation`
/// with the violations.
///
/// # Errors
///
/// [`RequestError::Review`] when the changeset is not pending, the validator is its
/// author or lacks the rights (it stays pending); other [`RequestError`]s as a
/// submission.
pub async fn validate_pending<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    sources: &Sources,
    id: Uuid,
    validator: Uuid,
    max_operations: usize,
) -> Result<Outcome, RequestError> {
    let found = pending(store, id).await?;
    if found.request.author == validator {
        return Err(refused(
            &rules::APPROVAL_SELF,
            format!("{validator} is the author of changeset {id}"),
            id,
        ));
    }
    let mode = Mode::Validation {
        id,
        validator,
        kind: kind(&found),
    };
    run(
        store,
        authorizer,
        sources,
        &found.request,
        max_operations,
        mode,
    )
    .await
}

/// Rejects a pending changeset with a motive, given back to its author (REQ-004-23, 43).
///
/// # Errors
///
/// [`RequestError::Review`] when the changeset is not pending, the motive is empty, the
/// validator is its author or could not validate it.
pub async fn reject<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    id: Uuid,
    validator: Uuid,
    motive: &str,
) -> Result<(), RequestError> {
    let found = pending(store, id).await?;
    if found.request.author == validator {
        return Err(refused(
            &rules::APPROVAL_SELF,
            format!("{validator} is the author of changeset {id}"),
            id,
        ));
    }
    if motive.trim().is_empty() {
        return Err(RequestError::MotiveRequired);
    }
    let denied = may_decide(authorizer, &found, validator).await?;
    if !denied.is_empty() {
        return Err(RequestError::Review(
            Violation::new(
                &rules::APPROVAL_FORBIDDEN,
                format!("{validator} could not validate changeset {id}"),
            )
            .with_subjects(denied),
        ));
    }
    if store
        .close_pending(id, "rejected", validator, Some(motive))
        .await?
    {
        Ok(())
    } else {
        Err(refused(
            &rules::CHANGESET_NOT_PENDING,
            format!("changeset {id} is not pending"),
            id,
        ))
    }
}

/// Withdraws a pending changeset, by its author only (REQ-004-23).
///
/// # Errors
///
/// [`RequestError::Review`] when the changeset is not pending or `author` is not its
/// author.
pub async fn withdraw<S: Store>(store: &S, id: Uuid, author: Uuid) -> Result<(), RequestError> {
    let found = pending(store, id).await?;
    if found.request.author != author {
        return Err(refused(
            &rules::CHANGESET_NOT_AUTHOR,
            format!("only the author of changeset {id} withdraws it"),
            id,
        ));
    }
    if store.close_pending(id, "withdrawn", author, None).await? {
        Ok(())
    } else {
        Err(refused(
            &rules::CHANGESET_NOT_PENDING,
            format!("changeset {id} is not pending"),
            id,
        ))
    }
}

/// The job that expires the changesets pending past their deadline (REQ-004-23).
///
/// # Errors
///
/// [`StoreError`] on infrastructure failure.
pub async fn expire_pending<S: Store>(store: &S) -> Result<Vec<Uuid>, StoreError> {
    store.expire_pending().await
}

/// The proposals `maintainer` may validate: every write permission they require is
/// held (REQ-004-43), oldest first.
///
/// # Errors
///
/// [`StoreError`] on infrastructure failure.
pub async fn proposals_for<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    maintainer: Uuid,
) -> Result<Vec<PendingChangeset>, StoreError> {
    let mut out = Vec::new();
    for found in store.pending_proposals().await? {
        if found.request.author != maintainer
            && may_decide(authorizer, &found, maintainer).await?.is_empty()
        {
            out.push(found);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> ApprovalPolicy {
        ApprovalPolicy {
            id: Uuid::from_u128(1),
            name: "big".into(),
            min_operations: None,
            min_diff_objects: None,
            property_keys: Vec::new(),
            domains: Vec::new(),
            authors: Vec::new(),
            enabled: true,
        }
    }

    #[test]
    fn every_defined_trigger_must_match() {
        let exposure = Exposure {
            operations: 3,
            diff_objects: 4_096,
            property_keys: ["policy.environment".to_owned()].into(),
            domains: [Uuid::from_u128(7)].into(),
            author: Uuid::from_u128(9),
        };
        // No trigger: holds nothing.
        assert!(!holds(&policy(), &exposure));
        let volume = ApprovalPolicy {
            min_diff_objects: Some(500),
            ..policy()
        };
        assert!(holds(&volume, &exposure));
        assert!(!holds(
            &volume,
            &Exposure {
                diff_objects: 3,
                ..exposure.clone()
            }
        ));
        // Conjunction: volume and another domain.
        let both = ApprovalPolicy {
            domains: vec![Uuid::from_u128(8)],
            ..volume.clone()
        };
        assert!(!holds(&both, &exposure));
        let keys = ApprovalPolicy {
            property_keys: vec!["policy.environment".into()],
            authors: vec![Uuid::from_u128(9)],
            ..policy()
        };
        assert!(holds(&keys, &exposure));
        assert!(!holds(
            &ApprovalPolicy {
                enabled: false,
                ..keys
            },
            &exposure
        ));
    }
}
