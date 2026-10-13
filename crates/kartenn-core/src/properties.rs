//! Property declarations and their compatible evolution (spec 003, REQ-003-1 to 6).

use std::collections::BTreeSet;
use std::fmt;

use uuid::Uuid;

use crate::rules::{self, Violation};
use crate::values::{
    BoolType, Cardinality, DateType, EnumType, FqdnType, IntType, RefType, TextType, Value,
    ValueType,
};

/// Namespace of administrator properties (003/P1).
pub const CUSTOM_NAMESPACE: &str = "custom";

/// Key of a property: `<namespace>.<name>`, each part `[a-z][a-z0-9_]*` (REQ-003-2).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropertyKey {
    namespace: String,
    name: String,
}

fn is_identifier(part: &str) -> bool {
    let mut bytes = part.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl PropertyKey {
    /// Parses `<namespace>.<name>`.
    ///
    /// # Errors
    ///
    /// A `prop.declaration.invalid` violation.
    pub fn parse(text: &str) -> Result<Self, Violation> {
        match text.split_once('.') {
            Some((ns, name)) if is_identifier(ns) && is_identifier(name) => Ok(Self {
                namespace: ns.to_owned(),
                name: name.to_owned(),
            }),
            _ => Err(Violation::new(
                &rules::PROP_INVALID_DECLARATION,
                format!("{text:?} is not a property key <namespace>.<name>"),
            )),
        }
    }

    /// Namespace (module name or `custom`).
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Permission required to assign the property (REQ-003-17): `prop.<key>.write`.
    #[must_use]
    pub fn write_permission(&self) -> String {
        format!("prop.{self}.write")
    }

    /// Permission to propose an assignment (004/C7): `prop.<key>.propose`.
    #[must_use]
    pub fn propose_permission(&self) -> String {
        format!("prop.{self}.propose")
    }
}

impl fmt::Display for PropertyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.namespace, self.name)
    }
}

/// Who declares a property (003/P1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declarant {
    /// A module, in its own namespace, with any compiled type.
    Module(String),
    /// An administrator, in the `custom` namespace, with built-in types only.
    Administrator,
}

/// Type of a property and its parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeSpec {
    /// Closed list.
    Enum,
    /// Text with bounds and an optional pattern.
    Text {
        /// Minimum length in code points.
        min_len: usize,
        /// Maximum length in code points.
        max_len: usize,
        /// Anchored pattern.
        pattern: Option<String>,
    },
    /// Integer within bounds.
    Int {
        /// Minimum.
        min: i64,
        /// Maximum.
        max: i64,
    },
    /// Boolean.
    Bool,
    /// Date.
    Date,
    /// Host name.
    Fqdn,
    /// Reference to an object kind.
    Ref {
        /// Target kind.
        target: &'static str,
    },
    /// Type compiled by a module (never available to administrators).
    Module(String),
}

impl TypeSpec {
    fn id(&self) -> &str {
        match self {
            Self::Enum => "enum",
            Self::Text { .. } => "text",
            Self::Int { .. } => "int",
            Self::Bool => "bool",
            Self::Date => "date",
            Self::Fqdn => "fqdn",
            Self::Ref { .. } => "ref",
            Self::Module(id) => id,
        }
    }

    /// The value type of a built-in specification.
    ///
    /// # Errors
    ///
    /// When parameters are invalid or the type is compiled by a module (resolved by the
    /// module registry, not here).
    pub fn value_type(&self) -> Result<Box<dyn ValueType>, String> {
        Ok(match self {
            Self::Enum => Box::new(EnumType),
            Self::Text {
                min_len,
                max_len,
                pattern,
            } => Box::new(TextType::new(*min_len, *max_len, pattern.as_deref())?),
            Self::Int { min, max } if min <= max => Box::new(IntType {
                min: *min,
                max: *max,
            }),
            Self::Int { min, max } => return Err(format!("minimum {min} is above maximum {max}")),
            Self::Bool => Box::new(BoolType),
            Self::Date => Box::new(DateType),
            Self::Fqdn => Box::new(FqdnType),
            Self::Ref { target } => Box::new(RefType { target }),
            Self::Module(id) => return Err(format!("type {id} is provided by a module")),
        })
    }
}

/// Tree along which a heritable property propagates (003/P3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InheritTree {
    /// Domain → containers → network → range or address.
    Addressing,
    /// Entity tree.
    Entity,
    /// Site tree.
    Site,
    /// A tree declared by a module.
    Module(String),
}

/// Scope of an obligation (003/P4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObligationScope {
    /// Every carrier object of an addressing domain.
    Domain(Uuid),
    /// Every carrier object owned by an entity.
    Entity(Uuid),
    /// Every object of a type.
    ObjectType(String),
}

/// A property declaration (REQ-003-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// Key.
    pub key: PropertyKey,
    /// Who declares it.
    pub declarant: Declarant,
    /// Type and parameters.
    pub spec: TypeSpec,
    /// One value or a set.
    pub cardinality: Cardinality,
    /// Object types that carry it.
    pub carriers: BTreeSet<String>,
    /// Tree of inheritance, if heritable.
    pub inherit: Option<InheritTree>,
    /// Scopes where a resolved value is mandatory.
    pub obligations: BTreeSet<ObligationScope>,
}

impl Declaration {
    /// Checks the declaration on its own (REQ-003-1 to 3).
    ///
    /// # Errors
    ///
    /// The first violation found.
    pub fn validate(&self) -> Result<(), Violation> {
        let forbidden = |msg: String| Err(Violation::new(&rules::PROP_NAMESPACE_FORBIDDEN, msg));
        match &self.declarant {
            Declarant::Module(m) if self.key.namespace() != m => {
                return forbidden(format!(
                    "module {m} cannot declare {} outside its namespace",
                    self.key
                ));
            }
            Declarant::Module(_) if self.key.namespace() == CUSTOM_NAMESPACE => {
                return forbidden(format!("{} is reserved for administrators", self.key));
            }
            Declarant::Administrator if self.key.namespace() != CUSTOM_NAMESPACE => {
                return forbidden(format!(
                    "administrators declare in {CUSTOM_NAMESPACE}.*, not {}",
                    self.key
                ));
            }
            Declarant::Administrator if matches!(self.spec, TypeSpec::Module(_)) => {
                return forbidden(format!("type {} is not a built-in type", self.spec.id()));
            }
            _ => {}
        }
        if self.carriers.is_empty() {
            return Err(Violation::new(
                &rules::PROP_INVALID_DECLARATION,
                format!("{} has no carrier type", self.key),
            ));
        }
        if !matches!(self.spec, TypeSpec::Module(_)) {
            self.spec.value_type().map_err(|e| {
                Violation::new(
                    &rules::PROP_INVALID_DECLARATION,
                    format!("{}: {e}", self.key),
                )
            })?;
        }
        Ok(())
    }
}

/// A current assignment, as reported by storage to the compatibility checker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentAssignment {
    /// Object carrying the assignment.
    pub object: Uuid,
    /// Its type.
    pub object_type: String,
    /// Assigned value; `None` for an explicit "no value".
    pub value: Option<Value>,
}

/// What the checker needs to know about the current use of a property.
pub trait PropertyUsage {
    /// Current assignments of the property.
    fn assignments(&self) -> Vec<CurrentAssignment>;
    /// Objects whose resolved value is inherited (no assignment of their own).
    fn inheriting_objects(&self) -> Vec<Uuid>;
    /// Objects of `scope` without a resolved value.
    fn objects_without_value(&self, scope: &ObligationScope) -> Vec<Uuid>;
    /// Re-validates a stored value under new type parameters.
    ///
    /// # Errors
    ///
    /// The constraint the value no longer meets.
    fn revalidate(&self, ty: &dyn ValueType, value: &Value) -> Result<(), String>;
}

fn incompatible(
    key: &PropertyKey,
    why: &str,
    objects: impl IntoIterator<Item = Uuid>,
) -> Violation {
    Violation::new(&rules::PROP_INCOMPATIBLE_CHANGE, format!("{key}: {why}")).with_subjects(objects)
}

/// Checks that `new` can replace `old` given the current use of the property
/// (REQ-003-6, one check per row of the 003 design table).
///
/// # Errors
///
/// A `prop.declaration.incompatible` violation listing the objects concerned, or the
/// validation error of `new`.
pub fn check_evolution(
    old: &Declaration,
    new: &Declaration,
    usage: &dyn PropertyUsage,
) -> Result<(), Violation> {
    new.validate()?;
    let key = &new.key;
    if old.key != new.key || old.declarant != new.declarant {
        return Err(incompatible(
            key,
            "the key and the declarant never change",
            [],
        ));
    }
    let assignments = usage.assignments();
    if (old.spec.id() != new.spec.id() || old.cardinality != new.cardinality)
        && !assignments.is_empty()
    {
        return Err(incompatible(
            key,
            "type or cardinality changes while assignments exist",
            assignments.iter().map(|a| a.object),
        ));
    }
    if old.spec != new.spec
        && let Ok(ty) = new.spec.value_type()
    {
        let invalid: Vec<Uuid> = assignments
            .iter()
            .filter(|a| {
                a.value
                    .as_ref()
                    .is_some_and(|v| usage.revalidate(ty.as_ref(), v).is_err())
            })
            .map(|a| a.object)
            .collect();
        if !invalid.is_empty() {
            return Err(incompatible(
                key,
                "stricter parameters invalidate existing values",
                invalid,
            ));
        }
    }
    let removed: BTreeSet<&String> = old.carriers.difference(&new.carriers).collect();
    let on_removed: Vec<Uuid> = assignments
        .iter()
        .filter(|a| removed.contains(&a.object_type))
        .map(|a| a.object)
        .collect();
    if !on_removed.is_empty() {
        return Err(incompatible(
            key,
            "a carrier type with assignments is removed",
            on_removed,
        ));
    }
    if old.inherit.is_some() && old.inherit != new.inherit {
        let heirs = usage.inheriting_objects();
        if !heirs.is_empty() {
            return Err(incompatible(
                key,
                "inheritance changes while objects inherit a value",
                heirs,
            ));
        }
    }
    for scope in new.obligations.difference(&old.obligations) {
        let missing = usage.objects_without_value(scope);
        if !missing.is_empty() {
            return Err(incompatible(
                key,
                &format!("objects of {scope:?} have no value"),
                missing,
            ));
        }
    }
    Ok(())
}

/// Checks that a closed-list value can be closed: no current assignment contains it
/// (REQ-003-6).
///
/// # Errors
///
/// A `prop.declaration.incompatible` violation listing the objects carrying it.
pub fn check_enum_value_closure(
    key: &PropertyKey,
    value: Uuid,
    usage: &dyn PropertyUsage,
) -> Result<(), Violation> {
    use crate::values::Scalar;
    let carrying: Vec<Uuid> = usage
        .assignments()
        .into_iter()
        .filter(|a| match &a.value {
            Some(Value::One(Scalar::Enum(v))) => *v == value,
            Some(Value::Set(s)) => s.contains(&Scalar::Enum(value)),
            _ => false,
        })
        .map(|a| a.object)
        .collect();
    if carrying.is_empty() {
        Ok(())
    } else {
        Err(incompatible(key, "the value is still assigned", carrying))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::{Raw, Scalar, ValueContext};

    fn key(t: &str) -> PropertyKey {
        PropertyKey::parse(t).unwrap()
    }

    fn decl(k: &str, declarant: Declarant, spec: TypeSpec) -> Declaration {
        Declaration {
            key: key(k),
            declarant,
            spec,
            cardinality: Cardinality::Single,
            carriers: BTreeSet::from(["address".to_owned(), "prefix".to_owned()]),
            inherit: None,
            obligations: BTreeSet::new(),
        }
    }

    fn text_spec(max_len: usize) -> TypeSpec {
        TypeSpec::Text {
            min_len: 0,
            max_len,
            pattern: None,
        }
    }

    #[derive(Default)]
    struct Usage {
        assignments: Vec<CurrentAssignment>,
        heirs: Vec<Uuid>,
        missing: Vec<Uuid>,
    }

    struct NoCtx;
    impl ValueContext for NoCtx {
        fn enum_value(&self, _: &str) -> Option<Uuid> {
            None
        }
        fn object_kind(&self, _: Uuid) -> Option<&'static str> {
            None
        }
    }

    impl PropertyUsage for Usage {
        fn assignments(&self) -> Vec<CurrentAssignment> {
            self.assignments.clone()
        }
        fn inheriting_objects(&self) -> Vec<Uuid> {
            self.heirs.clone()
        }
        fn objects_without_value(&self, _: &ObligationScope) -> Vec<Uuid> {
            self.missing.clone()
        }
        fn revalidate(&self, ty: &dyn ValueType, value: &Value) -> Result<(), String> {
            let Value::One(Scalar::Text(t)) = value else {
                return Ok(());
            };
            ty.parse(&Raw::Text(t.clone()), &NoCtx).map(|_| ())
        }
    }

    fn text_assignment(t: &str) -> CurrentAssignment {
        CurrentAssignment {
            object: Uuid::now_v7(),
            object_type: "address".into(),
            value: Some(Value::One(Scalar::Text(t.into()))),
        }
    }

    #[test]
    fn keys() {
        // REQ-003-2 acceptance.
        for ok in [
            "policy.environment",
            "dns.fqdn",
            "custom.centre_cout",
            "core.owner",
        ] {
            assert!(PropertyKey::parse(ok).is_ok(), "{ok}");
        }
        for bad in [
            "environment",
            "Policy.env",
            "custom.",
            ".x",
            "custom.cost-centre",
            "a.b.c",
            "1x.y",
        ] {
            assert!(PropertyKey::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(
            key("policy.environment").write_permission(),
            "prop.policy.environment.write"
        );
    }

    #[test]
    fn namespaces_and_declarants() {
        // REQ-003-1, 2 acceptance.
        let admin = Declarant::Administrator;
        let module = Declarant::Module("policy".into());
        assert!(
            decl("custom.centre_cout", admin.clone(), text_spec(40))
                .validate()
                .is_ok()
        );
        assert!(
            decl("policy.environment", module.clone(), TypeSpec::Enum)
                .validate()
                .is_ok()
        );
        let id = |d: Declaration| d.validate().unwrap_err().rule.id;
        assert_eq!(
            id(decl("policy.x", admin.clone(), TypeSpec::Bool)),
            "prop.declaration.namespace"
        );
        assert_eq!(
            id(decl("custom.x", module.clone(), TypeSpec::Bool)),
            "prop.declaration.namespace"
        );
        assert_eq!(
            id(decl("dns.x", module, TypeSpec::Bool)),
            "prop.declaration.namespace"
        );
        assert_eq!(
            id(decl(
                "custom.x",
                admin.clone(),
                TypeSpec::Module("ip_list".into())
            )),
            "prop.declaration.namespace"
        );
        assert_eq!(
            id(decl(
                "custom.x",
                admin.clone(),
                TypeSpec::Int { min: 5, max: 1 }
            )),
            "prop.declaration.invalid"
        );
        let mut no_carrier = decl("custom.x", admin, TypeSpec::Bool);
        no_carrier.carriers.clear();
        assert_eq!(id(no_carrier), "prop.declaration.invalid");
    }

    #[test]
    fn stricter_parameters_are_checked_against_values() {
        // REQ-003-6 acceptance: reducing the maximum length below an existing value.
        let old = decl("custom.label", Declarant::Administrator, text_spec(40));
        let new = decl("custom.label", Declarant::Administrator, text_spec(5));
        let long = text_assignment("a long label");
        let usage = Usage {
            assignments: vec![long.clone(), text_assignment("ok")],
            ..Usage::default()
        };
        let err = check_evolution(&old, &new, &usage).unwrap_err();
        assert_eq!(err.rule.id, "prop.declaration.incompatible");
        assert_eq!(err.subjects, vec![long.object]);
        let loose = decl("custom.label", Declarant::Administrator, text_spec(80));
        assert!(check_evolution(&old, &loose, &usage).is_ok());
    }

    #[test]
    fn type_and_carrier_changes() {
        let old = decl("custom.label", Declarant::Administrator, text_spec(40));
        let usage = Usage {
            assignments: vec![text_assignment("x")],
            ..Usage::default()
        };
        let retyped = decl("custom.label", Declarant::Administrator, TypeSpec::Bool);
        assert!(check_evolution(&old, &retyped, &usage).is_err());
        assert!(
            check_evolution(&old, &retyped, &Usage::default()).is_ok(),
            "no assignment: free"
        );
        let mut fewer = old.clone();
        fewer.carriers.remove("address");
        assert!(check_evolution(&old, &fewer, &usage).is_err());
        fewer.carriers = BTreeSet::from(["address".to_owned()]);
        assert!(
            check_evolution(&old, &fewer, &usage).is_ok(),
            "only unused carrier removed"
        );
    }

    #[test]
    fn inheritance_and_obligation_changes() {
        let mut old = decl("custom.zone", Declarant::Administrator, TypeSpec::Enum);
        old.inherit = Some(InheritTree::Addressing);
        let mut flat = old.clone();
        flat.inherit = None;
        let heir = Uuid::now_v7();
        assert!(
            check_evolution(
                &old,
                &flat,
                &Usage {
                    heirs: vec![heir],
                    ..Usage::default()
                }
            )
            .is_err()
        );
        assert!(check_evolution(&old, &flat, &Usage::default()).is_ok());
        let mut obliged = old.clone();
        obliged
            .obligations
            .insert(ObligationScope::ObjectType("prefix".into()));
        let err = check_evolution(
            &old,
            &obliged,
            &Usage {
                missing: vec![heir],
                ..Usage::default()
            },
        )
        .unwrap_err();
        assert_eq!(err.subjects, vec![heir]);
    }

    #[test]
    fn closing_an_assigned_enum_value_is_refused() {
        let sandbox = Uuid::now_v7();
        let carrying = CurrentAssignment {
            object: Uuid::now_v7(),
            object_type: "prefix".into(),
            value: Some(Value::Set(BTreeSet::from([Scalar::Enum(sandbox)]))),
        };
        let usage = Usage {
            assignments: vec![carrying.clone()],
            ..Usage::default()
        };
        let k = key("policy.environment");
        assert_eq!(
            check_enum_value_closure(&k, sandbox, &usage)
                .unwrap_err()
                .subjects,
            vec![carrying.object]
        );
        assert!(check_enum_value_closure(&k, Uuid::now_v7(), &usage).is_ok());
    }
}
