//! Rights on a changeset (spec 003, REQ-003-17, 18; spec 005, REQ-005-22; spec 024, M7).
//!
//! Every object written needs the edit permission of its kind; every assignment needs
//! `prop.<key>.edit` on its object, independently of the right to modify the object;
//! every existing object whose resolved value the changeset changes, directly or through
//! the structure, needs the same permission. Whoever applies the changeset needs, on the
//! same objects, the matching `apply` (someone else's changeset) or `approval.self` (one's
//! own): [`kartenn_core::authz::verb_permission`]. Rights are asked in groups, one call
//! per permission. A declaration or a closed-list value is a setting of the declarant of
//! its property, written under `<declarant>.settings.*`; its sensitivity is the core's
//! (spec 027, REQ-027-5, 6).

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::authz::{Verb, verb_permission};
use kartenn_core::period::{Instant, Period};
use kartenn_core::properties::PropertyKey;
use kartenn_core::rules::{self, Rule, Violation};
use uuid::Uuid;

use crate::authz::{Authorizer, POLICY_ADMIN, Subject};
use crate::operations::ObjectKind;
use crate::pipeline::Planned;
use crate::port::{Change, DiffCount, StoreError, WriteTx};

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

/// The effects of the changeset: the objects of its differential, by property key. Read
/// once for the checks of the author and of whoever applies, which ask rights on the same
/// objects (task 003-20: read twice under `approval.self`, about 30 ms more for 1.2·10⁵
/// objects).
pub(crate) type Effects = BTreeMap<String, Vec<Uuid>>;

/// Reads the [`Effects`] of the differential recorded so far.
pub(crate) async fn effects<T: WriteTx>(
    tx: &mut T,
    diff: &[DiffCount],
) -> Result<Effects, StoreError> {
    let keys: BTreeSet<&str> = diff.iter().map(|d| d.property.as_str()).collect();
    let mut effects = Effects::new();
    for key in keys {
        effects.insert(key.to_owned(), tx.diff_objects(key).await?);
    }
    Ok(effects)
}

/// Checks the rights of `check` on the assignments and on the `effects` of the changeset
/// (its differential); the objects refused are listed per property. Every edit
/// permission asked is added to `required` when given.
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
#[allow(clippy::too_many_lines)] // policies, guards, properties, administration, objects
pub(crate) async fn check<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    check: Check<'_>,
    planned: &[Planned],
    effects: &Effects,
    period: Option<Period>,
    mut required: Option<&mut Required>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let subject = check.subject;
    let at = period.map(Period::start);
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
        check_guards(
            tx,
            authorizer,
            subject,
            planned,
            (Phase::After, at),
            violations,
        )
        .await?;
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
    for (key, affected) in effects {
        objects
            .entry(key.clone())
            .or_default()
            .extend(affected.iter().copied());
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
        let rule = (&rules::PROP_FORBIDDEN_EFFECT, permission.as_str());
        violations.push(refusal(tx, authorizer, check, rule, denied, at).await?);
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
        (Phase::After, at),
        required,
        violations,
    )
    .await
}

/// A guard reveals the version of an object: it needs the right to read it (006-16); so
/// does an object an operation of a module cites without writing it (its handler's
/// [`reads`](crate::module::ModuleHandler::reads), 009/M16). In `phase`: before the writes,
/// the reads on the objects that exist already (INV-63, task 002-29c2: no write answers
/// before them); after, the guards and the reads on the objects the changeset creates. A
/// refusal names the objects in force at `at` ([`refusal`]).
pub(crate) async fn check_guards<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    subject: &Subject,
    planned: &[Planned],
    (phase, at): (Phase, Option<Instant>),
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let check = Check {
        subject,
        verb: Verb::Edit,
    };
    let created: BTreeSet<Uuid> = crate::resolution::created(planned).into_iter().collect();
    let mut guarded: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    for p in planned {
        if phase == Phase::After
            && let Planned::Expect { kind, id, .. } = p
            && !matches!(kind, ObjectKind::Property | ObjectKind::EnumValue)
        {
            guarded
                .entry(crate::reads::read_permission(kind.name()))
                .or_default()
                .push(*id);
        }
        if let Planned::Module { handler, plan, .. } = p {
            for (permission, object) in handler.reads(plan) {
                if created.contains(&object) == (phase == Phase::After) {
                    guarded.entry(permission).or_default().push(object);
                }
            }
        }
    }
    for (permission, ids) in guarded {
        let denied = authorizer
            .authorize_in(tx, subject, &permission, &ids)
            .await?;
        if !denied.is_empty() {
            let rule = (&rules::OBJECT_FORBIDDEN, permission.as_str());
            violations.push(refusal(tx, authorizer, check, rule, denied, at).await?);
        }
    }
    Ok(())
}

/// Before the writes, the author's rights on the objects the changeset closes or changes
/// ([`check_objects`]) and the reads its modules ask on objects that exist already
/// ([`check_guards`]); `origins`, the operation of each of `planned`. INV-63 (tasks
/// 002-29c2, 29d2): an operation whose read is refused answers by that refusal alone, the
/// rights on what it would change not added to it: found through the object its author
/// may not read (the namespace a cluster holds under a name), they would tell him that
/// this object exists. `at`, the start of the period of the changeset.
///
/// # Panics
///
/// When `origins` and `planned` differ in length: zipped, the longer would be cut, and
/// what it planned would escape the check of the edit without a refusal (task 002-29d7).
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
pub(crate) async fn check_before<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    edit: Check<'_>,
    author: &Subject,
    (planned, origins): (&[Planned], &[usize]),
    at: Option<Instant>,
    required: Option<&mut Required>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    assert_eq!(
        planned.len(),
        origins.len(),
        "one origin for each operation planned"
    );
    let created: BTreeSet<Uuid> = crate::resolution::created(planned).into_iter().collect();
    let mut asked: BTreeMap<String, Vec<(Uuid, usize)>> = BTreeMap::new();
    for (p, origin) in planned.iter().zip(origins) {
        if let Planned::Module { handler, plan, .. } = p {
            for (permission, object) in handler.reads(plan) {
                if !created.contains(&object) {
                    asked.entry(permission).or_default().push((object, *origin));
                }
            }
        }
    }
    let mut unread: BTreeSet<usize> = BTreeSet::new();
    for (permission, pairs) in asked {
        let ids: Vec<Uuid> = pairs.iter().map(|(id, _)| *id).collect();
        let denied: BTreeSet<Uuid> = authorizer
            .authorize_in(tx, author, &permission, &ids)
            .await?
            .into_iter()
            .collect();
        unread.extend(
            pairs
                .iter()
                .filter(|(id, _)| denied.contains(id))
                .map(|(_, origin)| *origin),
        );
    }
    let kept: Vec<Planned> = planned
        .iter()
        .zip(origins)
        .filter(|(_, origin)| !unread.contains(origin))
        .map(|(p, _)| p.clone())
        .collect();
    check_objects(
        tx,
        authorizer,
        edit,
        &kept,
        (Phase::Before, at),
        required,
        violations,
    )
    .await?;
    check_guards(
        tx,
        authorizer,
        author,
        planned,
        (Phase::Before, at),
        violations,
    )
    .await
}

/// The refusal of `permission` to `check` on `denied`, under `rule`. INV-63 (tasks
/// 002-29d4, 29d6): it names, and counts, only the objects among `denied` its subject
/// reads ([`read_among`]); the others, out of his reading or not in force (one a plan
/// draws), are neither named nor counted, so that the refusal tells him neither which
/// object a plan completed for him, nor whether it exists, nor how many objects out of his
/// reading a write reaches. The lacks of whoever applies (`verb` other than edit) keep
/// every object, for a review that needs them (REQ-024-11): `pipeline.rs` tells them
/// through [`read_among`], and their message counts as that presentation names.
async fn refusal<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    check: Check<'_>,
    (rule, permission): (&'static Rule, &str),
    denied: Vec<Uuid>,
    at: Option<Instant>,
) -> Result<Violation, StoreError> {
    let shown = read_among(tx, authorizer, check.subject, &denied, at).await?;
    let message = needed(shown.len(), permission);
    let subjects = if check.verb == Verb::Edit {
        shown
    } else {
        denied
    };
    Ok(Violation::new(rule, message).with_subjects(subjects))
}

/// The message of a refusal of `permission` that names `named` objects: none named, it
/// says that its objects are out of the reading of whom it answers.
fn needed(named: usize, permission: &str) -> String {
    if named == 0 {
        format!("objects you do not read need {permission}")
    } else {
        format!("{named} object(s) need {permission}")
    }
}

/// The objects among `ids` that `subject` reads, in their order (INV-63, tasks 002-29d4,
/// 29d6): in force at `at`, the start of the period of the changeset (now when `None`),
/// and readable by him; an object without a read permission (a property) counts as read.
pub(crate) async fn read_among<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    subject: &Subject,
    ids: &[Uuid],
    at: Option<Instant>,
) -> Result<Vec<Uuid>, StoreError> {
    let mut by_read: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    let mut read: BTreeSet<Uuid> = BTreeSet::new();
    for (id, kind) in tx.kinds_in_force(ids, at).await? {
        match crate::pipeline::read_permission(kind) {
            Some(permission) => by_read.entry(permission).or_default().push(id),
            None => {
                read.insert(id);
            }
        }
    }
    for (permission, ids) in by_read {
        let unread: BTreeSet<Uuid> = authorizer
            .authorize_in(tx, subject, &permission, &ids)
            .await?
            .into_iter()
            .collect();
        read.extend(ids.into_iter().filter(|id| !unread.contains(id)));
    }
    Ok(ids.iter().copied().filter(|id| read.contains(id)).collect())
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
        // The settings of an unknown declarant: the core's (spec 027); [`settings`] reads
        // the declarant of a property.
        ObjectKind::Property | ObjectKind::EnumValue => {
            kartenn_core::authz::settings_permission(kartenn_core::authz::CORE_DECLARANT)
        }
        ObjectKind::VlanGroup => "core.vlan_group.edit".into(),
        ObjectKind::Vlan => "core.vlan.edit".into(),
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

/// The settings permission a write of a declaration or of a closed-list value needs (spec
/// 027, REQ-027-5, 6): that of the declarant of its property, read from its key
/// ([`PropertyKey::settings_permission`]); the core's for a change of sensitivity, whatever
/// the declarant (P6).
async fn settings<T: WriteTx>(tx: &mut T, planned: &Planned) -> Result<String, StoreError> {
    let key = match planned {
        Planned::Modify {
            change: Change::Sensitive(_),
            ..
        } => None,
        Planned::Declare { declaration, .. } | Planned::UpdateDeclaration { declaration, .. } => {
            Some(declaration.key.clone())
        }
        Planned::EnumValue { property, .. } | Planned::EnumLabels { property, .. } => tx
            .settings_key(*property)
            .await?
            .and_then(|k| PropertyKey::parse(&k).ok()),
        other => tx
            .settings_key(other.object())
            .await?
            .and_then(|k| PropertyKey::parse(&k).ok()),
    };
    Ok(key.map_or_else(
        || edit_permission(ObjectKind::Property),
        |key| key.settings_permission(),
    ))
}

/// The objects `planned` writes in `phase`, by edit permission.
async fn written<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    phase: Phase,
) -> Result<BTreeMap<String, BTreeSet<Uuid>>, StoreError> {
    let mut out: BTreeMap<String, BTreeSet<Uuid>> = BTreeMap::new();
    for p in planned {
        let target = match (phase, p) {
            (_, Planned::Module { plan, .. }) if plan.creates == (phase == Phase::After) => {
                Some((ObjectKind::from_qualified(plan.kind), plan.object))
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
                _ if matches!(kind, ObjectKind::Property | ObjectKind::EnumValue) => {
                    settings(tx, p).await?
                }
                _ => edit_permission(kind),
            };
            out.entry(permission).or_default().insert(id);
        }
    }
    Ok(out)
}

/// Checks the right of `check` on the objects of the changeset in `phase` (REQ-005-22;
/// spec 024, REQ-024-10, 11): `core.<kind>.edit` on each for its author, its `apply` or
/// `approval.self` for whoever applies it, seen from the transaction. Every edit permission
/// asked is added to `required` when given. A refusal names the objects in force at `at`,
/// the start of the period of the changeset ([`refusal`]).
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
pub(crate) async fn check_objects<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    check: Check<'_>,
    planned: &[Planned],
    (phase, at): (Phase, Option<Instant>),
    mut required: Option<&mut Required>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    for (edit, objects) in written(tx, planned, phase).await? {
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
        let rule = (&rules::OBJECT_FORBIDDEN, permission.as_str());
        violations.push(refusal(tx, authorizer, check, rule, denied, at).await?);
    }
    Ok(())
}

/// Objects of the differential, all properties.
pub(crate) fn diff_objects(effects: &Effects) -> BTreeSet<Uuid> {
    effects.values().flatten().copied().collect()
}
