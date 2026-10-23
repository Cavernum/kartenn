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

use crate::authz::{Authorizer, POLICY_ADMIN, Subject};
use crate::operations::ObjectKind;
use crate::pipeline::Planned;
use crate::port::{DiffCount, StoreError, WriteTx};

/// Write permissions a subject lacks, with the objects concerned.
pub(crate) type Missing = Vec<(String, Vec<Uuid>)>;

/// Checks the rights of `subject` on the assignments and on the effects of the changeset
/// (its differential); the objects refused are listed per property. When `may_propose`,
/// a lack covered by `prop.<key>.propose` is returned instead of refused: the changeset
/// becomes a proposal (REQ-004-41).
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
pub(crate) async fn check<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    subject: &Subject,
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
            .authorize_in(tx, subject, POLICY_ADMIN, &policies)
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
    check_guards(tx, authorizer, subject, planned, violations).await?;
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
            .authorize_in(tx, subject, &key.write_permission(), &objects)
            .await?;
        if denied.is_empty() {
            continue;
        }
        if may_propose
            && authorizer
                .authorize_in(tx, subject, &key.propose_permission(), &denied)
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
    crate::rbac::check(tx, authorizer, subject, planned, violations).await?;
    missing.extend(
        check_objects(
            tx,
            authorizer,
            subject,
            planned,
            Phase::After,
            may_propose,
            violations,
        )
        .await?,
    );
    // A proposal needs every lack covered; a single refusal refuses the changeset.
    if violations.iter().any(|v| {
        v.rule.id == rules::PROP_FORBIDDEN_EFFECT.id || v.rule.id == rules::OBJECT_FORBIDDEN.id
    }) {
        missing.clear();
    }
    Ok(missing)
}

/// A guard reveals the version of an object: it needs the right to read it (006-16); so
/// does an object an operation of a module cites without writing it (its handler's
/// [`reads`](crate::module::ModuleHandler::reads), 009/M16).
async fn check_guards<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    subject: &Subject,
    planned: &[Planned],
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let mut guarded: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    for p in planned {
        if let Planned::Expect { kind, id, .. } = p
            && !matches!(kind, ObjectKind::Property | ObjectKind::EnumValue)
        {
            guarded
                .entry(crate::reads::read_permission(kind.name()))
                .or_default()
                .push(*id);
        }
        if let Planned::Module { handler, plan, .. } = p {
            for (permission, object) in handler.reads(plan) {
                guarded.entry(permission).or_default().push(object);
            }
        }
    }
    for (permission, ids) in guarded {
        let denied = authorizer
            .authorize_in(tx, subject, &permission, &ids)
            .await?;
        if !denied.is_empty() {
            let message = format!("{} object(s) need {permission}", denied.len());
            violations
                .push(Violation::new(&rules::OBJECT_FORBIDDEN, message).with_subjects(denied));
        }
    }
    Ok(())
}

/// The permission to write an object of `kind` (005, K12); `<type>.write` for an object of
/// a module (ADR 0026, clause 5).
fn write_permission(kind: ObjectKind) -> String {
    match kind {
        ObjectKind::Domain => "core.domain.write".into(),
        ObjectKind::Prefix => "core.prefix.write".into(),
        ObjectKind::Range => "core.range.write".into(),
        ObjectKind::Address => "core.address.write".into(),
        ObjectKind::Entity => "core.entity.write".into(),
        ObjectKind::Site => "core.site.write".into(),
        ObjectKind::Property | ObjectKind::EnumValue => "core.property.admin".into(),
        ObjectKind::Module(kind) => kartenn_core::authz::type_write_permission(kind.as_str()),
    }
}

/// When the objects of a changeset are checked: the existing ones it closes or changes
/// before its writes (closed, they are no longer valid now); those it creates after
/// (they exist in its transaction, with their inherited owner).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Before,
    After,
}

/// The objects `planned` writes in `phase`, by write permission.
fn written(planned: &[Planned], phase: Phase) -> BTreeMap<String, BTreeSet<Uuid>> {
    let mut out: BTreeMap<String, BTreeSet<Uuid>> = BTreeMap::new();
    for p in planned {
        let target = match (phase, p) {
            (_, Planned::Module { plan, .. }) if plan.creates == (phase == Phase::After) => {
                Some((ObjectKind::Module(plan.kind), plan.object))
            }
            (
                Phase::Before,
                Planned::Close { kind, id, .. }
                | Planned::Modify { kind, id, .. }
                | Planned::Restore {
                    kind,
                    id,
                    property: None,
                    ..
                }
                | Planned::BindExternal {
                    kind, object: id, ..
                },
            )
            | (Phase::After, Planned::Anchor { kind, id, .. }) => Some((*kind, *id)),
            (Phase::After, Planned::Domain { id, .. }) => Some((ObjectKind::Domain, *id)),
            (Phase::After, Planned::Prefix { object, .. }) => Some((ObjectKind::Prefix, object.id)),
            (Phase::After, Planned::Range { object, .. }) => Some((ObjectKind::Range, object.id)),
            (Phase::After, Planned::Address { object, .. }) => {
                Some((ObjectKind::Address, object.id))
            }
            (
                Phase::After,
                Planned::Declare { id, .. }
                | Planned::UpdateDeclaration { id, .. }
                | Planned::EnumValue { id, .. }
                | Planned::EnumLabels { property: id, .. },
            ) => Some((ObjectKind::Property, *id)),
            _ => None,
        };
        if let Some((kind, id)) = target {
            // An operation of a module may require its own permission in place of
            // `<type>.write`, checked on the same object (ADR 0026, amendment 3).
            let permission = match p {
                Planned::Module {
                    permission: Some(permission),
                    ..
                } => (*permission).to_owned(),
                _ => write_permission(kind),
            };
            out.entry(permission).or_default().insert(id);
        }
    }
    out
}

/// Checks the right of `subject` to write the objects of the changeset in `phase`
/// (REQ-005-22): `core.<kind>.write` on each, seen from the transaction; when
/// `may_propose`, a lack covered by `core.<kind>.propose` is returned instead of refused
/// (the changeset becomes a proposal).
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
pub(crate) async fn check_objects<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    subject: &Subject,
    planned: &[Planned],
    phase: Phase,
    may_propose: bool,
    violations: &mut Vec<Violation>,
) -> Result<Missing, StoreError> {
    let mut missing = Missing::new();
    for (permission, objects) in written(planned, phase) {
        let objects: Vec<Uuid> = objects.into_iter().collect();
        let denied = authorizer
            .authorize_in(tx, subject, &permission, &objects)
            .await?;
        if denied.is_empty() {
            continue;
        }
        let propose = permission
            .strip_suffix(".write")
            .map(|p| format!("{p}.propose"));
        if may_propose
            && let Some(propose) = propose
            && authorizer
                .authorize_in(tx, subject, &propose, &denied)
                .await?
                .is_empty()
        {
            missing.push((permission.clone(), denied));
            continue;
        }
        let message = format!("{} object(s) need {permission}", denied.len());
        violations.push(Violation::new(&rules::OBJECT_FORBIDDEN, message).with_subjects(denied));
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
