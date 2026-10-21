//! Property declarations through changesets: administrator operations and module
//! synchronisation (spec 003, REQ-003-1 to 6; task 003-7; decisions P1, P8, P12).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use kartenn_core::objects::Slug;
use kartenn_core::period::Period;
use kartenn_core::properties::{
    CORE_CARRIERS, CurrentAssignment, Declarant, Declaration, InheritTree, ObligationScope,
    PropertyKey, PropertyUsage, REF_TARGETS, TypeSpec, check_evolution, is_module_kind,
};
use kartenn_core::rules::{self, Violation};
use kartenn_core::values::{Cardinality, Scalar, Value, ValueContext, ValueType};
use uuid::{Uuid, uuid};

use crate::authz::Authorizer;
use crate::module::ModuleRegistry;
use crate::operations::{
    ChangesetRequest, DeclarationInput, ObjectKind, ObligationInput, Operation, Ref, TypeInput,
};
use crate::pipeline::{Outcome, Planned, RequestError, submit};
use crate::port::{PropertyFacts, Store, StoreError, WriteError, WriteTx};

/// The closed list of the systems an external reference names (REQ-016-3).
pub const EXTERNAL_SYSTEM: &str = "core.external_system";

/// Author of the changesets kartenn writes on its own, such as module synchronisation
/// (P12); created by migration.
pub const SYSTEM_AUTHOR: Uuid = uuid!("00000000-0000-7000-8000-000000000001");

fn invalid(message: String) -> Violation {
    Violation::new(&rules::PROP_INVALID_DECLARATION, message)
}

fn type_spec(input: &TypeInput, modules: &ModuleRegistry) -> Result<TypeSpec, Violation> {
    Ok(match input {
        TypeInput::Enum { values_of } => TypeSpec::Enum {
            values_of: values_of.as_deref().map(PropertyKey::parse).transpose()?,
        },
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
        // A kind of the core, or a type of a module of the build (REQ-008-7).
        TypeInput::Ref { target } => TypeSpec::Ref {
            target: REF_TARGETS
                .iter()
                .copied()
                .find(|t| *t == target.as_str())
                .or_else(|| modules.object_type(target).map(|t| t.kind))
                .ok_or_else(|| invalid(format!("{target} is not a kind a reference can target")))?,
        },
        TypeInput::ExternalRef => TypeSpec::ExternalRef,
        TypeInput::Module { id } => TypeSpec::Module(id.clone()),
    })
}

/// The input as a declaration, checked on its own (REQ-003-1 to 3). An administrator
/// declares on the kinds of the core and the types of the modules of the build, along the
/// trees of the core or of a module of the build (REQ-008-7).
///
/// # Errors
///
/// The first violation found.
pub(crate) fn declaration(
    input: &DeclarationInput,
    modules: &ModuleRegistry,
) -> Result<Declaration, Violation> {
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
            .find(|c| !CORE_CARRIERS.contains(&c.as_str()) && modules.object_type(c).is_none())
        {
            return Err(invalid(format!(
                "{key}: {unknown} is not a known object kind"
            )));
        }
        if let Some(InheritTree::Module(tree)) = &inherit
            && modules.get(tree).is_none()
        {
            return Err(invalid(format!("{key}: {tree} is not a known tree")));
        }
    }
    // Only the objects of a tree inherit along it (003-18): a carrier outside it would
    // resolve nothing. The objects of a module inherit along the tree of their module,
    // whatever the tree declared (REQ-008-6).
    if let Some(tree @ (InheritTree::Addressing | InheritTree::Entity | InheritTree::Site)) =
        &inherit
        && let Some(outside) = carriers
            .iter()
            .find(|c| !tree.kinds().contains(&c.as_str()) && !is_module_kind(c))
    {
        return Err(invalid(format!(
            "{key}: {outside} is not an object of the {} tree",
            tree.as_str()
        )));
    }
    let obligations: BTreeSet<ObligationScope> = input.obligations.iter().map(scope).collect();
    let declaration = Declaration {
        key,
        declarant,
        spec: type_spec(&input.value_type, modules)?,
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
    modules: &ModuleRegistry,
) -> Result<Planned, Violation> {
    match op {
        Operation::DeclareProperty { declaration: d, .. } => Ok(Planned::Declare {
            id,
            declaration: declaration(d, modules)?,
        }),
        Operation::UpdateProperty {
            id: target,
            declaration: d,
        } => Ok(Planned::UpdateDeclaration {
            id: resolve(*target),
            declaration: declaration(d, modules)?,
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
        TypeSpec::Enum { values_of } => TypeInput::Enum {
            values_of: values_of.as_ref().map(ToString::to_string),
        },
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
        TypeSpec::ExternalRef => TypeInput::ExternalRef,
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

/// The scopes `declaration` adds to `old` name a domain or an entity that exists over
/// the whole of `period` (003-18).
async fn check_scopes<T: WriteTx>(
    tx: &mut T,
    old: Option<&Declaration>,
    declaration: &Declaration,
    period: Option<Period>,
) -> Result<(), WriteError> {
    for scope in &declaration.obligations {
        let (id, kind) = match scope {
            ObligationScope::Domain(id) => (*id, "domain"),
            ObligationScope::Entity(id) => (*id, "entity"),
            ObligationScope::ObjectType(_) => continue,
        };
        if old.is_some_and(|o| o.obligations.contains(scope)) {
            continue;
        }
        if tx.object_kind_covering(id, period).await? != Some(kind) {
            return Err(WriteError::Rule(invalid(format!(
                "{}: the obligation names {id}, not a {kind} over the whole period",
                declaration.key
            ))));
        }
    }
    Ok(())
}

/// A closed list that borrows its values (REQ-016-2) borrows them from a closed list
/// valid at the start of `period` that borrows from no other, and is not itself a lender.
async fn check_values_of<T: WriteTx>(
    tx: &mut T,
    declaration: &Declaration,
    period: Option<Period>,
) -> Result<(), WriteError> {
    let TypeSpec::Enum {
        values_of: Some(owner),
    } = &declaration.spec
    else {
        return Ok(());
    };
    let refused = |why: String| {
        Err(WriteError::Rule(Violation::new(
            &rules::PROP_VALUES_OF,
            format!(
                "{} cannot borrow the values of {owner}: {why}",
                declaration.key
            ),
        )))
    };
    match tx.declaration_by_key(&owner.to_string(), period).await? {
        None => return refused("it is not declared".into()),
        Some((
            _,
            Declaration {
                spec: TypeSpec::Enum { values_of: None },
                ..
            },
        )) => {}
        Some((
            _,
            Declaration {
                spec: TypeSpec::Enum { values_of: Some(_) },
                ..
            },
        )) => return refused("it borrows its own values".into()),
        Some(_) => return refused("it is not a closed list".into()),
    }
    let borrowers = tx
        .borrowers_of(&declaration.key.to_string(), period)
        .await?;
    if borrowers.is_empty() {
        Ok(())
    } else {
        refused(format!("{} borrow its own values", borrowers.join(", ")))
    }
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
    check_scopes(tx, None, declaration, period).await?;
    check_values_of(tx, declaration, period).await?;
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
    check_scopes(tx, Some(&old), new, period).await?;
    check_values_of(tx, new, period).await?;
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
        Some(Declaration {
            spec: TypeSpec::Enum { values_of: None },
            ..
        }) => {
            tx.insert_enum_value(id, property, key.as_str(), period, changeset)
                .await
        }
        Some(Declaration {
            key: borrower,
            spec: TypeSpec::Enum {
                values_of: Some(owner),
            },
            ..
        }) => Err(WriteError::Rule(
            Violation::new(
                &rules::PROP_VALUES_OF,
                format!("{borrower} borrows the values of {owner}: add the value to {owner}"),
            )
            .with_subjects([id, property]),
        )),
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

/// The scope of an obligation as received.
pub(crate) fn scope(input: &ObligationInput) -> ObligationScope {
    match input {
        ObligationInput::Domain { id } => ObligationScope::Domain(*id),
        ObligationInput::ObjectType { kind } => ObligationScope::ObjectType(kind.clone()),
        ObligationInput::Entity { id } => ObligationScope::Entity(*id),
    }
}

/// Checks an administrator's obligation before it is written: its scope names a domain
/// or an entity that exists over the whole of `period` (003-18), and the property is
/// declared there (REQ-008-28).
pub(crate) async fn check_admin_obligation<T: WriteTx>(
    tx: &mut T,
    property: Uuid,
    scope: &ObligationScope,
    period: Option<Period>,
) -> Result<Declaration, WriteError> {
    let declaration = tx
        .declaration(property, period)
        .await?
        .ok_or(WriteError::NoVersion(property))?;
    let probe = Declaration {
        obligations: [scope.clone()].into(),
        ..declaration.clone()
    };
    check_scopes(tx, None, &probe, period).await?;
    Ok(declaration)
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
    core_declarations_for(ModuleRegistry::empty())
}

/// [`core_declarations`] completed by the modules of the build (spec 008, REQ-008-6):
/// every type of a module carries `core.owner`, inherited along the tree of its module
/// and required where the type says so; the types that say so carry `core.site`.
///
/// # Panics
///
/// Never: the keys are valid literals.
#[must_use]
pub fn core_declarations_for(modules: &ModuleRegistry) -> ModuleDeclarations {
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
                    carriers: ["domain", "prefix"]
                        .into_iter()
                        .chain(modules.object_types().map(|t| t.kind))
                        .map(String::from)
                        .collect(),
                    inherit: Some(InheritTree::Addressing),
                    obligations: std::iter::once("domain")
                        .chain(
                            modules
                                .object_types()
                                .filter(|t| t.owner_required)
                                .map(|t| t.kind),
                        )
                        .map(|k| ObligationScope::ObjectType(k.into()))
                        .collect(),
                    expected: None,
                },
                Vec::new(),
            ),
            (
                // The systems of external references (REQ-016-3), delivered empty and
                // administrable; a closed list needs a carrier, which never uses it.
                Declaration {
                    key: key(EXTERNAL_SYSTEM),
                    declarant: core(),
                    spec: TypeSpec::ENUM,
                    cardinality: Cardinality::Single,
                    carriers: BTreeSet::from(["entity".to_owned()]),
                    inherit: None,
                    obligations: BTreeSet::new(),
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
                    carriers: std::iter::once("prefix")
                        .chain(
                            modules
                                .object_types()
                                .filter(|t| t.carries_site)
                                .map(|t| t.kind),
                        )
                        .map(String::from)
                        .collect(),
                    inherit: None,
                    obligations: BTreeSet::new(),
                    expected: None,
                },
                Vec::new(),
            ),
        ],
    }
}
