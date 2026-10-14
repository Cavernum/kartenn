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

use crate::authz::Authorizer;
use crate::pipeline::Planned;
use crate::port::{DiffCount, StoreError, WriteTx};

/// Checks the rights of `author` on the assignments and on the effects of the changeset
/// (its differential); the objects refused are listed per property. A refusal that the
/// author could turn into a proposal (`prop.<key>.propose`) says so: the switch to a
/// pending proposal comes with the approval workflow (004-14).
pub(crate) async fn check<T: WriteTx, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    author: Uuid,
    planned: &[Planned],
    diff: &[DiffCount],
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let mut objects: BTreeMap<String, BTreeSet<Uuid>> = BTreeMap::new();
    for p in planned {
        if let Planned::Assign {
            object, property, ..
        }
        | Planned::Unassign {
            object, property, ..
        } = p
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
            .authorize_many(author, &key.write_permission(), &objects)
            .await?;
        if denied.is_empty() {
            continue;
        }
        let not_proposable = authorizer
            .authorize_many(author, &key.propose_permission(), &denied)
            .await?;
        let message = if not_proposable.is_empty() {
            format!(
                "{} object(s) need {}; the author may only propose (proposals come with 004-14)",
                denied.len(),
                key.write_permission()
            )
        } else {
            format!("{} object(s) need {}", denied.len(), key.write_permission())
        };
        violations
            .push(Violation::new(&rules::PROP_FORBIDDEN_EFFECT, message).with_subjects(denied));
    }
    Ok(())
}
