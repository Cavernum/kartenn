//! Contributions of the modules to the differential of a changeset (ADR 0026, clause 13;
//! spec 008, REQ-008-26): after the differential of the property values, each module with
//! a contribution produces one section, in the order of the registry (dependencies
//! first), reading the sections of the modules before it. The sections are recorded in the
//! transaction, stored with the journal, and read by the rules of the modules.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::module::{DiffContext, DiffSection, ModuleRegistry};
use crate::operations::ObjectKind;
use crate::pipeline::Planned;
use crate::port::{DiffCount, StoreError, WriteTx};

/// The objects `planned` writes, of every type, each once, in the order of the
/// operations.
pub(crate) fn touched(planned: &[Planned]) -> Vec<(ObjectKind, Uuid)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for p in planned {
        let target = match p {
            Planned::Module { plan, .. } => Some((ObjectKind::Module(plan.kind), plan.object)),
            Planned::Close { kind, id, .. }
            | Planned::Modify { kind, id, .. }
            | Planned::Restore { kind, id, .. }
            | Planned::Anchor { kind, id, .. }
            | Planned::BindExternal {
                kind, object: id, ..
            }
            | Planned::Assign {
                kind, object: id, ..
            }
            | Planned::Unassign {
                kind, object: id, ..
            } => Some((*kind, *id)),
            Planned::Domain { id, .. } => Some((ObjectKind::Domain, *id)),
            Planned::Prefix { object, .. } => Some((ObjectKind::Prefix, object.id)),
            Planned::Range { object, .. } => Some((ObjectKind::Range, object.id)),
            Planned::Address { object, .. } => Some((ObjectKind::Address, object.id)),
            Planned::Declare { id, .. }
            | Planned::UpdateDeclaration { id, .. }
            | Planned::EnumValue { id, .. } => Some((ObjectKind::Property, *id)),
            _ => None,
        };
        if let Some(target) = target
            && seen.insert(target.1)
        {
            out.push(target);
        }
    }
    out
}

/// The sections of the modules, in the order of the registry, each recorded in the
/// transaction once produced.
///
/// # Errors
///
/// A failure of the store, or a section named outside the namespace of its module or
/// produced twice (a defect of the module).
pub(crate) async fn contribute<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    changeset: Uuid,
    properties: &[DiffCount],
    touched: &[(ObjectKind, Uuid)],
) -> Result<Vec<DiffSection>, StoreError> {
    let mut sections: Vec<DiffSection> = Vec::new();
    for module in modules.modules() {
        let Some(handler) = module.handler() else {
            continue;
        };
        let context = DiffContext {
            changeset,
            properties,
            touched,
            sections: &sections,
        };
        let Some(section) = handler.diff(tx.module_tx(), &context).await? else {
            continue;
        };
        let namespace = format!("{}.", module.name());
        let local = section.name.strip_prefix(&namespace).unwrap_or_default();
        if local.is_empty() || sections.iter().any(|s| s.name == section.name) {
            return Err(StoreError(format!(
                "the module {} produced the section {:?}, outside its namespace or twice",
                module.name(),
                section.name
            )));
        }
        tx.record_section(&section).await?;
        sections.push(section);
    }
    Ok(sections)
}
