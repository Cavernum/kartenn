//! Pending changesets: approval policies, proposals, and their decisions (spec 004,
//! REQ-004-19 to 24, 39 to 43; tasks 004-11 to 14; 004/design, § Approbation et
//! propositions).

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::rules::{self, Violation};
use uuid::Uuid;

use crate::audit::{Outcome as AuditOutcome, RequestContext, denial};
use crate::authz::{Authorizer, Subject};
use crate::declarations::SYSTEM_AUTHOR;
use crate::expected::Sources;
use crate::module::{EndContext, Ending, ModuleRegistry};
use crate::operations::{ChangesetRequest, ModuleOperation, Operation};
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
    /// The scope identifiers of the policies (entities, sites, prefixes) the changeset
    /// touches (005).
    pub scopes_touched: BTreeSet<Uuid>,
    /// The roles the author holds (005).
    pub author_roles: BTreeSet<String>,
    /// The author's groups, as (issuer, group) (005).
    pub author_groups: BTreeSet<(String, String)>,
    /// The roles the changeset binds (005).
    pub bound_roles: BTreeSet<String>,
    /// Measures of the modules, `<module>.<measure>` (REQ-016-7); absent counts as zero.
    pub measures: BTreeMap<String, u64>,
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
        // The perimeter: a domain or a scope touched.
        (!policy.domains.is_empty() || !policy.scopes.is_empty()).then(|| {
            policy.domains.iter().any(|d| exposure.domains.contains(d))
                || policy
                    .scopes
                    .iter()
                    .any(|(_, s)| exposure.scopes_touched.contains(s))
        }),
        // The author: by person, role or group.
        (!policy.authors.is_empty()
            || !policy.author_roles.is_empty()
            || !policy.author_groups.is_empty())
        .then(|| {
            policy.authors.contains(&exposure.author)
                || policy
                    .author_roles
                    .iter()
                    .any(|r| exposure.author_roles.contains(r))
                || policy
                    .author_groups
                    .iter()
                    .any(|g| exposure.author_groups.contains(g))
        }),
        // A binding of a role.
        (!policy.bound_roles.is_empty()).then(|| {
            policy
                .bound_roles
                .iter()
                .any(|r| exposure.bound_roles.contains(r))
        }),
        // Measures of modules, each at its threshold (REQ-016-7).
        (!policy.module_triggers.is_empty()).then(|| {
            policy.module_triggers.iter().all(|(measure, threshold)| {
                exposure.measures.get(measure).copied().unwrap_or(0) >= *threshold
            })
        }),
    ];
    policy.enabled && triggers.iter().any(Option::is_some) && triggers.iter().flatten().all(|t| *t)
}

/// The first of `policies` (enabled, by name, as in force at submission) that holds the
/// changeset; never for the system author (P12), and none by default (REQ-004-39).
#[allow(clippy::too_many_arguments)] // what a changeset shows, gathered by the pipeline
pub(crate) async fn holding_policy<T: WriteTx>(
    tx: &mut T,
    policies: &[ApprovalPolicy],
    req: &ChangesetRequest,
    author: &Subject,
    planned: &[Planned],
    diff: &[DiffCount],
    domains: &[Uuid],
    measures: &BTreeMap<String, u64>,
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
    // The scopes and roles of the policies, resolved once (005, REQ-005-27).
    let mut scopes_touched = BTreeSet::new();
    let scopes: BTreeSet<&(String, Uuid)> = policies.iter().flat_map(|p| &p.scopes).collect();
    if !scopes.is_empty() {
        let mut objects: BTreeSet<Uuid> = planned.iter().map(Planned::object).collect();
        objects.extend(crate::rights::diff_objects(tx, diff).await?);
        let objects: Vec<Uuid> = objects.into_iter().collect();
        for (kind, scope) in scopes {
            if tx.scope_touched(kind, *scope, &objects).await? {
                scopes_touched.insert(*scope);
            }
        }
    }
    let author_roles = if policies.iter().any(|p| !p.author_roles.is_empty()) {
        tx.roles_of(author).await?.into_iter().collect()
    } else {
        BTreeSet::new()
    };
    let author_groups = author
        .issuer
        .iter()
        .flat_map(|i| author.groups.iter().map(move |g| (i.clone(), g.clone())))
        .collect();
    let exposure = Exposure {
        operations: req.operations.len(),
        diff_objects: diff.iter().map(|d| d.count).sum(),
        property_keys,
        domains: domains.iter().copied().collect(),
        author: req.author,
        scopes_touched,
        author_roles,
        author_groups,
        bound_roles: planned
            .iter()
            .filter_map(|p| match p {
                Planned::Rbac(crate::rbac::RbacOp::Bind { role, .. }) => Some(role.clone()),
                _ => None,
            })
            .collect(),
        measures: measures.clone(),
    };
    Ok(policies.iter().find(|p| holds(p, &exposure)).map(|p| p.id))
}

fn refused(rule: &'static rules::Rule, message: String, id: Uuid) -> RequestError {
    RequestError::Review(Violation::new(rule, message).with_subjects([id]))
}

/// A refused review, with its `authz.access.denied` event when it is a refusal of rights
/// (REQ-012-8): the event is recorded before the refusal is returned.
pub(crate) async fn audited<S: Store>(
    store: &S,
    context: &RequestContext,
    error: RequestError,
    changeset: Uuid,
) -> RequestError {
    let RequestError::Review(violation) = &error else {
        return error;
    };
    match denial(context, violation, Some(changeset)) {
        Ok(None) => error,
        Ok(Some(event)) => match store.audit(&event).await {
            Ok(()) => error,
            Err(e) => RequestError::Audit(e),
        },
        Err(e) => e,
    }
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
    validator: &Subject,
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
    validator: &Subject,
    max_operations: usize,
) -> Result<Outcome, RequestError> {
    validate_pending_with(
        store,
        authorizer,
        sources,
        id,
        validator,
        Vec::new(),
        max_operations,
    )
    .await
}

/// Validates a pending changeset as [`validate_pending`], with `operations` the validator
/// joins to it: only operations a module declares « of review », on a proposal, planned
/// checked and applied with it in one changeset (REQ-016-11). Another operation refuses
/// the validation with `core.review.operation`; the changeset stays pending.
///
/// # Errors
///
/// As [`validate_pending`].
pub async fn validate_pending_with<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    sources: &Sources,
    id: Uuid,
    validator: &Subject,
    operations: Vec<Operation>,
    max_operations: usize,
) -> Result<Outcome, RequestError> {
    let mut found = pending(store, id).await?;
    if let Some(foreign) = operations
        .iter()
        .find(|op| !is_review(store.modules(), op) || kind(&found) != PendingKind::Proposal)
    {
        let name = match foreign {
            Operation::Module(op) => op.tag(),
            _ => "an operation of the core".to_owned(),
        };
        let message = format!("{name} is not an operation of review of a proposal");
        return Ok(Outcome::Refused {
            changeset: id,
            violations: vec![Violation::new(&rules::REVIEW_OPERATION, message).with_subjects([id])],
        });
    }
    found.request.operations.extend(operations);
    let context = RequestContext::of(validator);
    let validator_id = validator.id;
    if found.request.author == validator_id {
        let error = refused(
            &rules::APPROVAL_SELF,
            format!("{validator} is the author of changeset {id}"),
            id,
        );
        return Err(audited(store, &context, error, id).await);
    }
    let mode = Mode::Validation {
        id,
        validator: validator_id,
        kind: kind(&found),
    };
    // An approval checks the author's rights again, with the groups it had when
    // submitting (REQ-005-22).
    let author = found
        .pending
        .author
        .clone()
        .unwrap_or_else(|| Subject::from(found.request.author));
    run(
        store,
        authorizer,
        sources,
        &found.request,
        max_operations,
        mode,
        &context,
        &author,
    )
    .await
}

/// Whether `operation` is one a module declares « of review » (REQ-016-11).
fn is_review(modules: &ModuleRegistry, operation: &Operation) -> bool {
    let Operation::Module(op) = operation else {
        return false;
    };
    modules
        .get(&op.module)
        .is_some_and(|m| m.review_operations().contains(&op.name.as_str()))
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
    validator: &Subject,
    motive: &str,
) -> Result<(), RequestError> {
    let found = pending(store, id).await?;
    let context = RequestContext::of(validator);
    if found.request.author == validator.id {
        let error = refused(
            &rules::APPROVAL_SELF,
            format!("{validator} is the author of changeset {id}"),
            id,
        );
        return Err(audited(store, &context, error, id).await);
    }
    if motive.trim().is_empty() {
        return Err(RequestError::MotiveRequired);
    }
    let denied = may_decide(authorizer, &found, validator).await?;
    if !denied.is_empty() {
        let error = RequestError::Review(
            Violation::new(
                &rules::APPROVAL_FORBIDDEN,
                format!("{validator} could not validate changeset {id}"),
            )
            .with_subjects(denied),
        );
        return Err(audited(store, &context, error, id).await);
    }
    let event = decision(
        &context,
        &kartenn_core::audit::CHANGESET_REJECTED,
        id,
        Some(motive),
    )?;
    let ended = Ended {
        found: &found,
        how: Ending::Rejected,
        by: validator.id,
        motive: Some(motive),
    };
    end(store, &ended, &event).await
}

/// A pending changeset that ends without being applied, as a decision.
struct Ended<'a> {
    found: &'a PendingChangeset,
    how: Ending,
    by: Uuid,
    motive: Option<&'a str>,
}

/// Ends a pending changeset in one transaction: its state, its events, then the modules
/// whose operations it holds, any failure of which rolls it all back (REQ-016-9).
async fn end<S: Store>(
    store: &S,
    ended: &Ended<'_>,
    event: &kartenn_core::audit::AuditEvent,
) -> Result<(), RequestError> {
    let id = ended.found.id;
    let state = match ended.how {
        Ending::Rejected => "rejected",
        Ending::Withdrawn => "withdrawn",
        Ending::Expired => "expired",
    };
    let mut tx = store.begin().await?;
    if !tx
        .close_pending(id, state, ended.by, ended.motive, event)
        .await?
    {
        tx.rollback().await?;
        return Err(refused(
            &rules::CHANGESET_NOT_PENDING,
            format!("changeset {id} is not pending"),
            id,
        ));
    }
    let notified = notify(
        &mut tx,
        store.modules(),
        ended.found,
        ended.how,
        Some(ended.by),
        ended.motive,
    )
    .await;
    match notified {
        Ok(()) => tx.commit().await.map_err(RequestError::from),
        Err(e) => {
            tx.rollback().await?;
            Err(e.into())
        }
    }
}

/// Tells the modules whose operations `found` holds that it ended (REQ-016-9), then
/// writes the events they emitted.
async fn notify<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    found: &PendingChangeset,
    how: Ending,
    by: Option<Uuid>,
    motive: Option<&str>,
) -> Result<(), StoreError> {
    let mut by_module: BTreeMap<&str, Vec<&ModuleOperation>> = BTreeMap::new();
    for op in &found.request.operations {
        if let Operation::Module(op) = op {
            by_module.entry(op.module.as_str()).or_default().push(op);
        }
    }
    for (module, operations) in by_module {
        let Some(handler) = modules.get(module).and_then(|m| m.handler()) else {
            continue;
        };
        let context = EndContext {
            changeset: found.id,
            how,
            motive,
            by,
            operations,
        };
        handler.ended(tx.module_tx(), &context).await?;
    }
    tx.journal_ended(found.id).await
}

/// Withdraws a pending changeset, by its author only (REQ-004-23).
///
/// # Errors
///
/// [`RequestError::Review`] when the changeset is not pending or `author` is not its
/// author.
pub async fn withdraw<S: Store>(store: &S, id: Uuid, author: Uuid) -> Result<(), RequestError> {
    let found = pending(store, id).await?;
    let context = RequestContext::person(author);
    if found.request.author != author {
        let error = refused(
            &rules::CHANGESET_NOT_AUTHOR,
            format!("only the author of changeset {id} withdraws it"),
            id,
        );
        return Err(audited(store, &context, error, id).await);
    }
    let event = decision(
        &context,
        &kartenn_core::audit::CHANGESET_WITHDRAWN,
        id,
        None,
    )?;
    let ended = Ended {
        found: &found,
        how: Ending::Withdrawn,
        by: author,
        motive: None,
    };
    end(store, &ended, &event).await
}

/// The event of a decision on a pending changeset.
fn decision(
    context: &RequestContext,
    kind: &'static kartenn_core::audit::EventKind,
    id: Uuid,
    motive: Option<&str>,
) -> Result<kartenn_core::audit::AuditEvent, RequestError> {
    let mut event = context
        .event(kind, AuditOutcome::Success)
        .map_err(|e| RequestError::Audit(StoreError(e.to_string())))?
        .target("changeset", [id])
        .changeset(id);
    if let Some(motive) = motive {
        event = event.motive(motive);
    }
    Ok(event)
}

/// The job that expires the changesets pending past their deadline (REQ-004-23).
///
/// # Errors
///
/// [`StoreError`] on infrastructure failure.
pub async fn expire_pending<S: Store>(store: &S) -> Result<Vec<Uuid>, StoreError> {
    let mut tx = store.begin().await?;
    let expired = match tx.expire_pending().await {
        Ok(expired) => expired,
        Err(e) => {
            tx.rollback().await?;
            return Err(e);
        }
    };
    // The modules learn of each expiry in the same transaction (REQ-016-9): a failure
    // leaves every changeset pending, for the next run.
    for found in &expired {
        if let Err(e) = notify(&mut tx, store.modules(), found, Ending::Expired, None, None).await {
            tx.rollback().await?;
            return Err(e);
        }
    }
    tx.commit().await?;
    Ok(expired.into_iter().map(|found| found.id).collect())
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
    maintainer: &Subject,
) -> Result<Vec<PendingChangeset>, StoreError> {
    let mut out = Vec::new();
    for found in store.pending_proposals().await? {
        if found.request.author != maintainer.id
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
            scopes: Vec::new(),
            author_roles: Vec::new(),
            author_groups: Vec::new(),
            bound_roles: Vec::new(),
            module_triggers: Vec::new(),
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
            scopes_touched: [Uuid::from_u128(5)].into(),
            author_roles: ["auditor".to_owned()].into(),
            author_groups: [("idp".to_owned(), "netops".to_owned())].into(),
            bound_roles: ["administrator".to_owned()].into(),
            measures: [("policy.opens".to_owned(), 2)].into(),
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
        // 005: a binding of the administrator role (REQ-005-25).
        let second = ApprovalPolicy {
            bound_roles: vec!["administrator".into()],
            ..policy()
        };
        assert!(holds(&second, &exposure));
        assert!(!holds(
            &second,
            &Exposure {
                bound_roles: ["reader".to_owned()].into(),
                ..exposure.clone()
            }
        ));
        // 016: measures of modules, each at its threshold (REQ-016-7).
        let opens = ApprovalPolicy {
            module_triggers: vec![("policy.opens".into(), 1)],
            ..policy()
        };
        assert!(holds(&opens, &exposure));
        let unmeasured = ApprovalPolicy {
            module_triggers: vec![("policy.opens".into(), 1), ("flow.breaks".into(), 1)],
            ..policy()
        };
        assert!(
            !holds(&unmeasured, &exposure),
            "an absent measure counts as zero"
        );
        // 005: a scope touched; an author by role or by group.
        let scoped = ApprovalPolicy {
            scopes: vec![("entity".into(), Uuid::from_u128(5))],
            ..policy()
        };
        assert!(holds(&scoped, &exposure));
        let elsewhere = ApprovalPolicy {
            scopes: vec![("entity".into(), Uuid::from_u128(6))],
            ..policy()
        };
        assert!(!holds(&elsewhere, &exposure));
        for authors in [
            ApprovalPolicy {
                author_roles: vec!["auditor".into()],
                ..policy()
            },
            ApprovalPolicy {
                author_groups: vec![("idp".into(), "netops".into())],
                ..policy()
            },
        ] {
            assert!(holds(&authors, &exposure));
        }
        assert!(!holds(
            &ApprovalPolicy {
                author_roles: vec!["administrator".into()],
                ..policy()
            },
            &exposure
        ));
    }
}
