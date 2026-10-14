//! Property declarations through changesets: administrator operations and module
//! synchronisation (spec 003, REQ-003-1 to 6; task 003-7; decisions P1, P8, P12).

use std::collections::{BTreeSet, HashMap};

use kartenn_core::objects::Slug;
use kartenn_core::period::Period;
use kartenn_core::properties::{
    CORE_CARRIERS, CurrentAssignment, Declarant, Declaration, InheritTree, ObligationScope,
    PropertyKey, PropertyUsage, REF_TARGETS, TypeSpec, check_evolution,
};
use kartenn_core::rules::{self, Violation};
use kartenn_core::values::{Scalar, Value, ValueContext, ValueType};
use uuid::{Uuid, uuid};

use crate::operations::{
    ChangesetRequest, DeclarationInput, ObjectKind, Operation, Ref, TypeInput,
};
use crate::pipeline::{Outcome, Planned, RequestError, submit};
use crate::port::{PropertyFacts, Store, StoreError, WriteError, WriteTx};

/// Author of the changesets kartenn writes on its own, such as module synchronisation
/// (P12); created by migration.
pub const SYSTEM_AUTHOR: Uuid = uuid!("00000000-0000-7000-8000-000000000001");

fn invalid(message: String) -> Violation {
    Violation::new(&rules::PROP_INVALID_DECLARATION, message)
}

fn type_spec(input: &TypeInput) -> Result<TypeSpec, Violation> {
    Ok(match input {
        TypeInput::Enum => TypeSpec::Enum,
        TypeInput::Text {
            min_len,
            max_len,
            pattern,
        } => TypeSpec::Text {
            min_len: *min_len,
            max_len: *max_len,
            pattern: pattern.clone(),
        },
        TypeInput::Int { min, max } => TypeSpec::Int {
            min: *min,
            max: *max,
        },
        TypeInput::Bool => TypeSpec::Bool,
        TypeInput::Date => TypeSpec::Date,
        TypeInput::Fqdn => TypeSpec::Fqdn,
        TypeInput::Ref { target } => TypeSpec::Ref {
            target: REF_TARGETS
                .iter()
                .find(|t| **t == target.as_str())
                .ok_or_else(|| invalid(format!("{target} is not a kind a reference can target")))?,
        },
        TypeInput::Module { id } => TypeSpec::Module(id.clone()),
    })
}

/// The input as a declaration, checked on its own (REQ-003-1 to 3). An administrator
/// declares on core object kinds and core trees only, until modules register theirs.
///
/// # Errors
///
/// The first violation found.
pub(crate) fn declaration(input: &DeclarationInput) -> Result<Declaration, Violation> {
    let key = PropertyKey::parse(&input.key)?;
    let declarant = input
        .module
        .clone()
        .map_or(Declarant::Administrator, Declarant::Module);
    let carriers: BTreeSet<String> = input.carriers.iter().cloned().collect();
    let inherit = input.inherit.as_deref().map(InheritTree::from_name);
    if declarant == Declarant::Administrator {
        if let Some(unknown) = carriers
            .iter()
            .find(|c| !CORE_CARRIERS.contains(&c.as_str()))
        {
            return Err(invalid(format!(
                "{key}: {unknown} is not a known object kind"
            )));
        }
        if let Some(InheritTree::Module(tree)) = &inherit {
            return Err(invalid(format!("{key}: {tree} is not a known tree")));
        }
    }
    let declaration = Declaration {
        key,
        declarant,
        spec: type_spec(&input.value_type)?,
        cardinality: input.cardinality,
        carriers,
        inherit,
        obligations: BTreeSet::new(),
    };
    declaration.validate()?;
    Ok(declaration)
}

/// Plans a declaration operation (local checks only).
///
/// # Errors
///
/// The first violation found.
pub(crate) fn plan(
    op: &Operation,
    id: Uuid,
    resolve: impl Fn(Ref) -> Uuid,
) -> Result<Planned, Violation> {
    match op {
        Operation::DeclareProperty { declaration: d, .. } => Ok(Planned::Declare {
            id,
            declaration: declaration(d)?,
        }),
        Operation::UpdateProperty {
            id: target,
            declaration: d,
        } => Ok(Planned::UpdateDeclaration {
            id: resolve(*target),
            declaration: declaration(d)?,
        }),
        Operation::AddEnumValue { property, key, .. } => Slug::parse(key)
            .map(|key| Planned::EnumValue {
                id,
                property: resolve(*property),
                key,
            })
            .map_err(|_| {
                Violation::new(
                    &rules::PROP_ENUM_VALUE_INVALID,
                    format!("{key:?} is not a closed-list key [a-z0-9-]"),
                )
                .with_subjects([id])
            }),
        other => unreachable!("not a declaration operation: {other:?}"),
    }
}

/// The input describing a declaration, the reverse of [`declaration`].
fn input(declaration: &Declaration) -> DeclarationInput {
    let value_type = match &declaration.spec {
        TypeSpec::Enum => TypeInput::Enum,
        TypeSpec::Text {
            min_len,
            max_len,
            pattern,
        } => TypeInput::Text {
            min_len: *min_len,
            max_len: *max_len,
            pattern: pattern.clone(),
        },
        TypeSpec::Int { min, max } => TypeInput::Int {
            min: *min,
            max: *max,
        },
        TypeSpec::Bool => TypeInput::Bool,
        TypeSpec::Date => TypeInput::Date,
        TypeSpec::Fqdn => TypeInput::Fqdn,
        TypeSpec::Ref { target } => TypeInput::Ref {
            target: (*target).to_owned(),
        },
        TypeSpec::Module(id) => TypeInput::Module { id: id.clone() },
    };
    DeclarationInput {
        key: declaration.key.to_string(),
        value_type,
        cardinality: declaration.cardinality,
        carriers: declaration.carriers.iter().cloned().collect(),
        inherit: declaration.inherit.as_ref().map(|t| t.as_str().to_owned()),
        module: match &declaration.declarant {
            Declarant::Module(m) => Some(m.clone()),
            Declarant::Administrator => None,
        },
    }
}

/// Use of a property read from storage, as the compatibility checker sees it.
struct Usage(PropertyFacts);

impl ValueContext for Usage {
    fn enum_value(&self, key: &str) -> Option<Uuid> {
        self.0.enum_values.get(key).copied()
    }

    fn object_kind(&self, id: Uuid) -> Option<&'static str> {
        self.0.ref_kinds.get(&id).copied()
    }
}

impl PropertyUsage for Usage {
    fn assignments(&self) -> Vec<CurrentAssignment> {
        self.0.assignments.clone()
    }

    fn inheriting_objects(&self) -> Vec<Uuid> {
        self.0.inheriting.clone()
    }

    fn objects_without_value(&self, _scope: &ObligationScope) -> Vec<Uuid> {
        // Obligations do not change through declarations before 003-10: the checker never
        // asks (old and new declarations carry the same, empty, obligations).
        Vec::new()
    }

    fn revalidate(&self, ty: &dyn ValueType, value: &Value) -> Result<(), String> {
        let scalars: Vec<_> = match value {
            Value::One(s) => vec![s],
            Value::Set(set) => set.iter().collect(),
        };
        for raw in scalars.into_iter().filter_map(Scalar::to_raw) {
            ty.parse(&raw, self)?;
        }
        Ok(())
    }
}

/// Replaces the declaration of a property if compatible with its use (REQ-003-6); the
/// refusal is pushed to `violations`.
pub(crate) async fn update<T: WriteTx>(
    tx: &mut T,
    id: Uuid,
    new: &Declaration,
    period: Option<Period>,
    changeset: Uuid,
    violations: &mut Vec<Violation>,
) -> Result<(), WriteError> {
    let Some(old) = tx.declaration(id, period).await? else {
        return Err(WriteError::NoVersion(id));
    };
    if old == *new {
        return Ok(());
    }
    let facts = tx.property_facts(id, old.spec.id(), period).await?;
    match check_evolution(&old, new, &Usage(facts)) {
        Ok(()) => tx.replace_declaration(id, new, period, changeset).await,
        Err(v) => {
            violations.push(v.with_subjects([id]));
            Ok(())
        }
    }
}

/// Adds a closed-list value to a property of type `enum` (REQ-003-5, 7).
pub(crate) async fn add_enum_value<T: WriteTx>(
    tx: &mut T,
    id: Uuid,
    property: Uuid,
    key: &Slug,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    match tx.declaration(property, period).await? {
        Some(d) if d.spec == TypeSpec::Enum => {
            tx.insert_enum_value(id, property, key.as_str(), period, changeset)
                .await
        }
        Some(d) => Err(WriteError::Rule(
            Violation::new(
                &rules::PROP_ENUM_VALUE_INVALID,
                format!("{} is not a closed list", d.key),
            )
            .with_subjects([id, property]),
        )),
        None => Err(WriteError::NoVersion(property)),
    }
}

/// Final-state checks of closures (REQ-003-6, P8): a closed property keeps no assignment,
/// a closed value is no longer assigned.
pub(crate) async fn check_closure<T: WriteTx>(
    tx: &mut T,
    kind: ObjectKind,
    id: Uuid,
    period: Option<Period>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let (objects, why) = match kind {
        ObjectKind::Property => (
            tx.objects_assigned(id, period).await?,
            "close its assignments in the same changeset first",
        ),
        ObjectKind::EnumValue => (
            tx.objects_carrying(id, period).await?,
            "the value is still assigned",
        ),
        _ => return Ok(()),
    };
    if !objects.is_empty() {
        violations.push(
            Violation::new(
                &rules::PROP_INCOMPATIBLE_CHANGE,
                format!("{id}: {why} ({} object(s))", objects.len()),
            )
            .with_subjects(std::iter::once(id).chain(objects)),
        );
    }
    Ok(())
}

/// Declarations of a module, as its code describes them (P12).
#[derive(Debug, Clone)]
pub struct ModuleDeclarations {
    /// Module name, the namespace of its keys.
    pub module: String,
    /// Module version, recorded in the motive.
    pub version: String,
    /// Declarations, each with the keys of its closed-list values.
    pub properties: Vec<(Declaration, Vec<String>)>,
}

/// Operations bringing the stored declarations of a module in line with its code.
fn sync_operations(
    module: &ModuleDeclarations,
    stored: &[(Uuid, Declaration, Vec<String>)],
) -> Vec<Operation> {
    let by_key: HashMap<String, &(Uuid, Declaration, Vec<String>)> =
        stored.iter().map(|s| (s.1.key.to_string(), s)).collect();
    let mut operations = Vec::new();
    let mut local = 0;
    for (declaration, values) in &module.properties {
        let key = declaration.key.to_string();
        let (property, known): (Ref, &[String]) =
            if let Some((id, current, keys)) = by_key.get(&key) {
                if current != declaration {
                    operations.push(Operation::UpdateProperty {
                        id: Ref::Id(*id),
                        declaration: input(declaration),
                    });
                }
                (Ref::Id(*id), keys)
            } else {
                local += 1;
                operations.push(Operation::DeclareProperty {
                    local: Some(local),
                    declaration: input(declaration),
                });
                (Ref::Local(local), &[])
            };
        for value in values.iter().filter(|v| !known.contains(v)) {
            operations.push(Operation::AddEnumValue {
                local: None,
                property,
                key: value.clone(),
            });
        }
    }
    let in_code: BTreeSet<String> = module
        .properties
        .iter()
        .map(|(d, _)| d.key.to_string())
        .collect();
    for (id, current, _) in stored {
        if !in_code.contains(&current.key.to_string()) {
            operations.push(Operation::Close {
                kind: ObjectKind::Property,
                id: Ref::Id(*id),
                expected: None,
            });
        }
    }
    operations
}

/// Synchronises the declarations of each module with its code, one system changeset per
/// module with differences (P12). Called by the migration job; a refusal must fail the
/// deployment.
///
/// # Errors
///
/// [`RequestError`] on infrastructure failure.
pub async fn sync_modules<S: Store>(
    store: &S,
    modules: &[ModuleDeclarations],
    max_operations: usize,
) -> Result<Vec<Outcome>, RequestError> {
    let mut outcomes = Vec::new();
    for module in modules {
        let mut tx = store.begin().await?;
        let stored = tx.declarations_in(&module.module).await?;
        tx.rollback().await?;
        let operations = sync_operations(module, &stored);
        if operations.is_empty() {
            continue;
        }
        let request = ChangesetRequest {
            author: SYSTEM_AUTHOR,
            motive: format!("module sync {}@{}", module.module, module.version),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        outcomes.push(submit(store, &request, max_operations).await?);
    }
    Ok(outcomes)
}
