//! Encoding of property declarations and values for the property tables (spec 003,
//! migration 0011).

use std::collections::BTreeSet;

use kartenn_core::properties::{
    CurrentAssignment, Declarant, Declaration, InheritTree, PropertyKey, REF_TARGETS, TypeSpec,
};
use kartenn_core::values::{Cardinality, Date, Scalar, Value};
use kartenn_service::port::StoreError;
use serde_json::{Value as Json, json};
use sqlx::types::Uuid;

fn decode_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(format!("stored property data: {e}"))
}

/// Identifier and JSON parameters of a type, as stored.
pub(crate) fn encode_type(spec: &TypeSpec) -> (String, String) {
    let params = match spec {
        TypeSpec::Text {
            min_len,
            max_len,
            pattern,
        } => json!({ "min_len": min_len, "max_len": max_len, "pattern": pattern }),
        TypeSpec::Int { min, max } => json!({ "min": min, "max": max }),
        TypeSpec::Ref { target } => json!({ "target": target }),
        TypeSpec::Enum | TypeSpec::Bool | TypeSpec::Date | TypeSpec::Fqdn | TypeSpec::Module(_) => {
            json!({})
        }
    };
    (spec.id().to_owned(), params.to_string())
}

fn decode_type(id: &str, params: &str) -> Result<TypeSpec, StoreError> {
    let p: Json = serde_json::from_str(params).map_err(decode_err)?;
    let int = |k: &str| p[k].as_i64().ok_or_else(|| decode_err(format!("{id}.{k}")));
    let size = |k: &str| {
        p[k].as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| decode_err(format!("{id}.{k}")))
    };
    Ok(match id {
        "enum" => TypeSpec::Enum,
        "text" => TypeSpec::Text {
            min_len: size("min_len")?,
            max_len: size("max_len")?,
            pattern: p["pattern"].as_str().map(str::to_owned),
        },
        "int" => TypeSpec::Int {
            min: int("min")?,
            max: int("max")?,
        },
        "bool" => TypeSpec::Bool,
        "date" => TypeSpec::Date,
        "fqdn" => TypeSpec::Fqdn,
        "ref" => {
            let target = p["target"].as_str().unwrap_or_default();
            TypeSpec::Ref {
                target: REF_TARGETS
                    .iter()
                    .find(|t| **t == target)
                    .ok_or_else(|| decode_err(format!("reference target {target}")))?,
            }
        }
        other => TypeSpec::Module(other.to_owned()),
    })
}

/// Row of `property_declaration`: key, type, parameters, cardinality, carriers, tree.
pub(crate) type DeclarationRow = (String, String, String, String, Vec<String>, Option<String>);

/// Columns of [`DeclarationRow`].
pub(crate) const DECLARATION_COLUMNS: &str =
    "key, value_type, type_params::text, cardinality, carriers, inherit_tree";

/// Decodes a declaration; its obligations are not read (003-10).
pub(crate) fn decode_declaration(
    (key, value_type, params, cardinality, carriers, inherit): DeclarationRow,
) -> Result<Declaration, StoreError> {
    let key = PropertyKey::parse(&key).map_err(|v| decode_err(v.message))?;
    Ok(Declaration {
        declarant: Declarant::of(&key),
        key,
        spec: decode_type(&value_type, &params)?,
        cardinality: match cardinality.as_str() {
            "single" => Cardinality::Single,
            "set" => Cardinality::Set,
            other => return Err(decode_err(format!("cardinality {other}"))),
        },
        carriers: carriers.into_iter().collect(),
        inherit: inherit.as_deref().map(InheritTree::from_name),
        obligations: BTreeSet::new(),
    })
}

/// Row of `property_assignment` for the compatibility checker.
pub(crate) type AssignmentRow = (
    Uuid,
    String,
    String,
    Option<Uuid>,
    Option<Vec<Uuid>>,
    Option<String>,
    Option<Vec<String>>,
    Option<i64>,
    Option<Vec<i64>>,
    Option<bool>,
    Option<String>,
    Option<Vec<String>>,
);

/// Columns of [`AssignmentRow`].
pub(crate) const ASSIGNMENT_COLUMNS: &str = "object_id, object_type, kind, v_uuid, v_uuids, \
     v_text, v_texts, v_int, v_ints, v_bool, v_date::text, v_dates::text[]";

/// Decodes an assignment; identifiers are closed-list values or references depending on
/// the type of the property.
pub(crate) fn decode_assignment(
    type_id: &str,
    row: AssignmentRow,
) -> Result<CurrentAssignment, StoreError> {
    let (object, object_type, kind, uuid, uuids, text, texts, int, ints, boolean, date, dates) =
        row;
    let id = |v: Uuid| {
        if type_id == "enum" {
            Scalar::Enum(v)
        } else {
            Scalar::Ref(v)
        }
    };
    let day = |t: &str| Date::parse(t).map(Scalar::Date).map_err(decode_err);
    let value = if kind == "cleared" {
        None
    } else if let Some(v) = uuid {
        Some(Value::One(id(v)))
    } else if let Some(v) = uuids {
        Some(Value::Set(v.into_iter().map(id).collect()))
    } else if let Some(v) = text {
        Some(Value::One(Scalar::Text(v)))
    } else if let Some(v) = texts {
        Some(Value::Set(v.into_iter().map(Scalar::Text).collect()))
    } else if let Some(v) = int {
        Some(Value::One(Scalar::Int(v)))
    } else if let Some(v) = ints {
        Some(Value::Set(v.into_iter().map(Scalar::Int).collect()))
    } else if let Some(v) = boolean {
        Some(Value::One(Scalar::Bool(v)))
    } else if let Some(v) = date {
        Some(Value::One(day(&v)?))
    } else if let Some(v) = dates {
        Some(Value::Set(
            v.iter().map(|t| day(t)).collect::<Result<_, _>>()?,
        ))
    } else {
        return Err(decode_err(format!("assignment of {object} has no value")));
    };
    Ok(CurrentAssignment {
        object,
        object_type,
        value,
    })
}

/// Kind name of a stored object kind, as a reference target names it.
pub(crate) fn ref_kind(kind: &str) -> Option<&'static str> {
    REF_TARGETS.iter().find(|t| **t == kind).copied()
}

/// Objects referenced by assignments.
pub(crate) fn referenced(assignments: &[CurrentAssignment]) -> Vec<Uuid> {
    let mut ids = BTreeSet::new();
    for a in assignments {
        match &a.value {
            Some(Value::One(Scalar::Ref(id))) => {
                ids.insert(*id);
            }
            Some(Value::Set(set)) => ids.extend(set.iter().filter_map(|s| match s {
                Scalar::Ref(id) => Some(*id),
                _ => None,
            })),
            _ => {}
        }
    }
    ids.into_iter().collect()
}
