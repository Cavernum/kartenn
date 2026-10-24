//! Rights on a changeset (spec 003, REQ-003-17, 18; spec 005, REQ-005-22; spec 024, M7).
//!
//! Every object written needs the edit permission of its kind; every assignment needs
//! `prop.<key>.edit` on its object, independently of the right to modify the object;
//! every existing object whose resolved value the changeset changes, directly or through
//! the structure, needs the same permission. Whoever applies the changeset needs, on the
//! same objects, the matching `apply` (someone else's changeset) or `apply_own` (one's
//! own): [`kartenn_core::authz::verb_permission`]. Rights are asked in groups, one call
//! per permission.

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::authz::{Verb, verb_permission};
use kartenn_core::period::Period;
use kartenn_core::properties::PropertyKey;
use kartenn_core::rules::{self, Violation};
use uuid::Uuid;

use crate::authz::{Authorizer, POLICY_ADMIN, Subject};
use crate::operations::ObjectKind;
use crate::pipeline::Planned;
use crate::port::{DiffCount, StoreError, WriteTx};

/// The edit permissions a changeset requires, with the objects concerned: what whoever
/// applies it needs the application of (spec 024, REQ-024-11).
pub(crate) type Required = Vec<(String, Vec<Uuid>)>;

/// Who is checked, and for what (spec 024, M7): the author edits, the one who applies
/// applies.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Check<'a> {
    /// The subject whose rights are checked.
    pub subject: &'a Subject,
    /// What it does with the changeset.
    pub verb: Verb,
}

/// Checks the rights of `check` on the assignments and on the effects of the changeset
/// (its differential); the objects refused are listed per property. Every edit
/// permission asked is added to `required` when given.
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
#[allow(clippy::too_many_lines)] // policies, guards, properties, administration, objects
pub(crate) async fn check<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    check: Check<'_>,
    planned: &[Planned],
    diff: &[DiffCount],
    period: Option<Period>,
    mut required: Option<&mut Required>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let subject = check.subject;
    // An approval policy decides what needs approval: administered, whatever the verb.
    let policies: Vec<Uuid> = planned
        .iter()
        .filter_map(|p| match p {
            Planned::Policy(policy) => Some(policy.id),
            _ => None,
        })
        .collect();
    if !policies.is_empty() {
        if let Some(required) = required.as_deref_mut() {
            required.push((POLICY_ADMIN.to_owned(), policies.clone()));
        }
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
    // A guard reveals a version to whoever writes the changeset; the read of the one who
    // applies it is not asked.
    if check.verb == Verb::Edit {
        check_guards(tx, authorizer, subject, planned, violations).await?;
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
        let edit = key.edit_permission();
        let permission = verb_permission(&edit, check.verb);
        let denied = authorizer
            .authorize_in(tx, subject, &permission, &objects)
            .await?;
        if let Some(required) = required.as_deref_mut() {
            required.push((edit, objects));
        }
        if denied.is_empty() {
            continue;
        }
        let message = format!("{} object(s) need {permission}", denied.len());
        violations
            .push(Violation::new(&rules::PROP_FORBIDDEN_EFFECT, message).with_subjects(denied));
    }
    crate::rbac::check(
        tx,
        authorizer,
        subject,
        planned,
        required.as_deref_mut(),
        violations,
    )
    .await?;
    check_objects(
        tx,
        authorizer,
        check,
        planned,
        Phase::After,
        required,
        violations,
    )
    .await
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

/// The permission to edit an object of `kind` (005, K12; spec 024, REQ-024-9);
/// `<type>.edit` for an object of a module (ADR 0026, clause 5).
fn edit_permission(kind: ObjectKind) -> String {
    match kind {
        ObjectKind::Domain => "core.domain.edit".into(),
        ObjectKind::Prefix => "core.prefix.edit".into(),
        ObjectKind::Range => "core.range.edit".into(),
        ObjectKind::Address => "core.address.edit".into(),
        ObjectKind::Entity => "core.entity.edit".into(),
        ObjectKind::Site => "core.site.edit".into(),
        ObjectKind::Property | ObjectKind::EnumValue => "core.property.admin".into(),
        ObjectKind::Module(kind) => kartenn_core::authz::type_edit_permission(kind.as_str()),
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

/// The objects `planned` writes in `phase`, by edit permission.
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
            // `<type>.edit`, checked on the same object (ADR 0026, amendment 3).
            let permission = match p {
                Planned::Module {
                    permission: Some(permission),
                    ..
                } => (*permission).to_owned(),
                _ => edit_permission(kind),
            };
            out.entry(permission).or_default().insert(id);
        }
    }
    out
}

/// Checks the right of `check` on the objects of the changeset in `phase` (REQ-005-22;
/// spec 024, REQ-024-10, 11): `core.<kind>.edit` on each for its author, its `apply` or
/// `apply_own` for whoever applies it, seen from the transaction. Every edit permission
/// asked is added to `required` when given.
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
pub(crate) async fn check_objects<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    check: Check<'_>,
    planned: &[Planned],
    phase: Phase,
    mut required: Option<&mut Required>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    for (edit, objects) in written(planned, phase) {
        let objects: Vec<Uuid> = objects.into_iter().collect();
        let permission = verb_permission(&edit, check.verb);
        let denied = authorizer
            .authorize_in(tx, check.subject, &permission, &objects)
            .await?;
        if let Some(required) = required.as_deref_mut() {
            required.push((edit, objects));
        }
        if denied.is_empty() {
            continue;
        }
        let message = format!("{} object(s) need {permission}", denied.len());
        violations.push(Violation::new(&rules::OBJECT_FORBIDDEN, message).with_subjects(denied));
    }
    Ok(())
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
