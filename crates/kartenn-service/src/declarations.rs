//! Property declarations through changesets: administrator operations and module
//! synchronisation (spec 003, REQ-003-1 to 6; task 003-7; decisions P1, P8, P12).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use kartenn_core::objects::Slug;
use kartenn_core::period::Period;
use kartenn_core::properties::{
    CORE_CARRIERS, CurrentAssignment, Declarant, Declaration, InheritTree, ObligationScope,
    PropertyKey, PropertyUsage, REF_TARGETS, TypeSpec, check_evolution,
};
use kartenn_core::rules::{self, Violation};
use kartenn_core::values::{Cardinality, Scalar, Value, ValueContext, ValueType};
use uuid::{Uuid, uuid};

use crate::authz::Authorizer;
use crate::operations::{
    ChangesetRequest, DeclarationInput, ObjectKind, ObligationInput, Operation, Ref, TypeInput,
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
    // Origins are maintained along the addressing tree only, for now (003, task 18).
    if let Some(tree @ (InheritTree::Entity | InheritTree::Site)) = &inherit {
        return Err(invalid(format!(
            "{key}: inheritance along the {} tree is not maintained yet",
            tree.as_str()
        )));
    }
    let mut obligations = BTreeSet::new();
    for o in &input.obligations {
        obligations.insert(match o {
            ObligationInput::Domain { id } => ObligationScope::Domain(*id),
            ObligationInput::ObjectType { kind } => ObligationScope::ObjectType(kind.clone()),
            // The objects of an entity follow the entity tree (003, task 18).
            ObligationInput::Entity { .. } => {
                return Err(invalid(format!(
                    "{key}: an obligation scoped by entity is not available yet"
                )));
            }
        });
    }
    let declaration = Declaration {
        key,
        declarant,
        spec: type_spec(&input.value_type)?,
        cardinality: input.cardinality,
        carriers,
        inherit,
        obligations,
        expected: input.expected.clone(),
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
        obligations: declaration
            .obligations
            .iter()
            .map(|o| match o {
                ObligationScope::Domain(id) => ObligationInput::Domain { id: *id },
                ObligationScope::Entity(id) => ObligationInput::Entity { id: *id },
                ObligationScope::ObjectType(kind) => {
                    ObligationInput::ObjectType { kind: kind.clone() }
                }
            })
            .collect(),
        expected: declaration.expected.clone(),
    }
}

/// Objects without a value for each scope `declaration` adds to `old` (all its scopes
/// for a new declaration), from the start of `period` on (REQ-003-16).
async fn missing_values<T: WriteTx>(
    tx: &mut T,
    id: Uuid,
    old: Option<&Declaration>,
    declaration: &Declaration,
    period: Option<Period>,
) -> Result<BTreeMap<ObligationScope, Vec<Uuid>>, StoreError> {
    let carriers: Vec<String> = declaration.carriers.iter().cloned().collect();
    // Origins are materialised under the current declaration.
    let heritable = old.unwrap_or(declaration).inherit.is_some();
    let mut out = BTreeMap::new();
    for scope in &declaration.obligations {
        if old.is_some_and(|o| o.obligations.contains(scope)) {
            continue;
        }
        let missing = tx
            .objects_without_value(id, &carriers, heritable, scope, None, period)
            .await?;
        out.insert(scope.clone(), missing);
    }
    Ok(out)
}

/// Declares a property; refused if a scope of its obligations already holds objects,
/// which have no value yet (REQ-003-16).
pub(crate) async fn declare<T: WriteTx>(
    tx: &mut T,
    id: Uuid,
    declaration: &Declaration,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    tx.insert_declaration(id, declaration, period, changeset)
        .await?;
    let missing: Vec<Uuid> = missing_values(tx, id, None, declaration, period)
        .await?
        .into_values()
        .flatten()
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(WriteError::Rule(required(&declaration.key, missing)))
    }
}

/// The violation listing objects left without a mandatory value.
pub(crate) fn required(key: &PropertyKey, objects: Vec<Uuid>) -> Violation {
    Violation::new(
        &rules::PROP_REQUIRED,
        format!(
            "{key} is mandatory here: {} object(s) have no value",
            objects.len()
        ),
    )
    .with_subjects(objects)
}

/// Use of a property read from storage, as the compatibility checker sees it.
struct Usage(PropertyFacts, BTreeMap<ObligationScope, Vec<Uuid>>);

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

    fn objects_without_value(&self, scope: &ObligationScope) -> Vec<Uuid> {
        // Read beforehand for every scope the new declaration adds.
        self.1.get(scope).cloned().unwrap_or_default()
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
    let missing = missing_values(tx, id, Some(&old), new, period).await?;
    match check_evolution(&old, new, &Usage(facts, missing)) {
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
                cascade: false,
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
pub async fn sync_modules<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
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
        outcomes.push(submit(store, authorizer, &request, max_operations).await?);
    }
    Ok(outcomes)
}

/// Declarations of the core (002/design, ADR 0014), synchronised like a module's (P12):
/// `core.owner`, the owning entity, carried by domains and prefixes, inherited along the
/// addressing tree and required on every domain (REQ-002-11, 38); `core.site`, the site of
/// a network, not inherited (REQ-002-20).
///
/// # Panics
///
/// Never: the keys are valid literals.
#[must_use]
pub fn core_declarations() -> ModuleDeclarations {
    let core = || Declarant::Module("core".into());
    let key = |k| PropertyKey::parse(k).expect("core keys are valid literals");
    ModuleDeclarations {
        module: "core".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        properties: vec![
            (
                Declaration {
                    key: key("core.owner"),
                    declarant: core(),
                    spec: TypeSpec::Ref { target: "entity" },
                    cardinality: Cardinality::Single,
                    carriers: ["domain", "prefix"].map(String::from).into(),
                    inherit: Some(InheritTree::Addressing),
                    obligations: [ObligationScope::ObjectType("domain".into())].into(),
                    expected: None,
                },
                Vec::new(),
            ),
            (
                Declaration {
                    key: key("core.site"),
                    declarant: core(),
                    spec: TypeSpec::Ref { target: "site" },
                    cardinality: Cardinality::Single,
                    carriers: ["prefix".to_owned()].into(),
                    inherit: None,
                    obligations: BTreeSet::new(),
                    expected: None,
                },
                Vec::new(),
            ),
        ],
    }
}
