//! Rights on properties (spec 003, REQ-003-17, 18; task 003-11; P6, P7).
//!
//! Every assignment needs `prop.<key>.write` on its object, independently of the right to
//! modify the object; every existing object whose resolved value the changeset changes,
//! directly or through the structure, needs the same permission. Rights are asked in
//! groups, one call per property.

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::period::Period;
use kartenn_core::properties::PropertyKey;
use kartenn_core::rules::{self, Violation};
use uuid::Uuid;

use crate::authz::{Authorizer, POLICY_ADMIN};
use crate::pipeline::Planned;
use crate::port::{DiffCount, StoreError, WriteTx};

/// Write permissions a subject lacks, with the objects concerned.
pub(crate) type Missing = Vec<(String, Vec<Uuid>)>;

/// Checks the rights of `subject` on the assignments and on the effects of the changeset
/// (its differential); the objects refused are listed per property. When `may_propose`,
/// a lack covered by `prop.<key>.propose` is returned instead of refused: the changeset
/// becomes a proposal (REQ-004-41).
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
pub(crate) async fn check<T: WriteTx, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    subject: Uuid,
    planned: &[Planned],
    diff: &[DiffCount],
    period: Option<Period>,
    may_propose: bool,
    violations: &mut Vec<Violation>,
) -> Result<Missing, StoreError> {
    let mut missing = Missing::new();
    // An approval policy decides what needs approval: never proposable.
    let policies: Vec<Uuid> = planned
        .iter()
        .filter_map(|p| match p {
            Planned::Policy(policy) => Some(policy.id),
            _ => None,
        })
        .collect();
    if !policies.is_empty() {
        let denied = authorizer
            .authorize_many(subject, POLICY_ADMIN, &policies)
            .await?;
        if !denied.is_empty() {
            violations.push(
                Violation::new(
                    &rules::POLICY_FORBIDDEN,
                    format!("approval policies need {POLICY_ADMIN}"),
                )
                .with_subjects(denied),
            );
        }
    }
    let mut objects: BTreeMap<String, BTreeSet<Uuid>> = BTreeMap::new();
    for p in planned {
        let assigned = match p {
            Planned::Assign {
                object, property, ..
            }
            | Planned::Unassign {
                object, property, ..
            } => Some((object, property)),
            Planned::Restore {
                id,
                property: Some(property),
                ..
            } => Some((id, property)),
            _ => None,
        };
        if let Some((object, property)) = assigned
            && let Some(declaration) = tx.declaration(*property, period).await?
        {
            objects
                .entry(declaration.key.to_string())
                .or_default()
                .insert(*object);
        }
    }
    let keys: BTreeSet<&str> = diff.iter().map(|d| d.property.as_str()).collect();
    for key in keys {
        objects
            .entry(key.to_owned())
            .or_default()
            .extend(tx.diff_objects(key).await?);
    }
    for (key, objects) in objects {
        let Ok(key) = PropertyKey::parse(&key) else {
            continue;
        };
        let objects: Vec<Uuid> = objects.into_iter().collect();
        let denied = authorizer
            .authorize_many(subject, &key.write_permission(), &objects)
            .await?;
        if denied.is_empty() {
            continue;
        }
        if may_propose
            && authorizer
                .authorize_many(subject, &key.propose_permission(), &denied)
                .await?
                .is_empty()
        {
            missing.push((key.write_permission(), denied));
            continue;
        }
        let message = format!("{} object(s) need {}", denied.len(), key.write_permission());
        violations
            .push(Violation::new(&rules::PROP_FORBIDDEN_EFFECT, message).with_subjects(denied));
    }
    // A proposal needs every lack covered; a single refusal refuses the changeset.
    if violations
        .iter()
        .any(|v| v.rule.id == rules::PROP_FORBIDDEN_EFFECT.id)
    {
        missing.clear();
    }
    Ok(missing)
}

/// Objects of the differential recorded so far, all properties.
pub(crate) async fn diff_objects<T: WriteTx>(
    tx: &mut T,
    diff: &[DiffCount],
) -> Result<BTreeSet<Uuid>, StoreError> {
    let mut objects = BTreeSet::new();
    let keys: BTreeSet<&str> = diff.iter().map(|d| d.property.as_str()).collect();
    for key in keys {
        objects.extend(tx.diff_objects(key).await?);
    }
    Ok(objects)
}
