//! The operations on the objects of the policy (feature `pg`; REQ-010-4 to 9, 33, 36, 37):
//! flow types, services, patterns, rules, admissibility conditions, signals and exempt
//! roles, each created by `<type>.create`, modified by `<type>.update` (a Merge Patch, the
//! texts replaced as a whole) and closed by the `Close` of the core. What one object can
//! check is checked here; what needs the state (designated axes, living references, texts
//! in the default language) is checked by the rules on the final state.

use kartenn_core::rules::Violation;
use kartenn_service::module::ModulePlan;
use kartenn_service::operations::{ModuleOperation, Ref};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::codec;
use crate::handler::{invalid, plan};
use crate::model::{Key, Label};
use crate::module::{CONDITION, EXEMPT_ROLE, FLOW_TYPE, PATTERN, RULE, SERVICE, SIGNAL};
use crate::rules::{RULE_CONDITIONAL_DENY, RULE_FLOW_SELECTOR_EMPTY, RULE_TERM_EMPTY};

/// The longest description of a flow type, in characters.
const DESCRIPTION_MAX_CHARS: usize = 4000;

/// The fields of an operation on an object, by name.
struct Fields<'a> {
    operation: &'a ModuleOperation,
    resolve: &'a dyn Fn(Ref) -> Option<Uuid>,
}

impl Fields<'_> {
    fn get(&self, name: &str) -> Option<&Value> {
        self.operation.body.get(name).filter(|v| !v.is_null())
    }

    fn required(&self, name: &str) -> Result<&Value, Violation> {
        self.get(name)
            .ok_or_else(|| invalid(self.operation, format!("{name} is required")))
    }

    fn key(&self) -> Result<Option<String>, Violation> {
        self.get("key")
            .map(|k| {
                let k = k
                    .as_str()
                    .ok_or_else(|| invalid(self.operation, "key is a string"))?;
                Ok(Key::new(k)?.as_str().to_owned())
            })
            .transpose()
    }

    fn label(&self) -> Result<Option<String>, Violation> {
        self.get("label")
            .map(|l| {
                let l = l
                    .as_str()
                    .ok_or_else(|| invalid(self.operation, "label is a string"))?;
                Ok(Label::new(l)?.as_str().to_owned())
            })
            .transpose()
    }

    /// The object of a reference.
    fn object(&self, value: &Value) -> Result<Uuid, Violation> {
        let reference: Ref = serde_json::from_value(value.clone())
            .map_err(|_| invalid(self.operation, format!("{value} is not a reference")))?;
        (self.resolve)(reference)
            .ok_or_else(|| invalid(self.operation, format!("{value}: unknown object")))
    }

    /// The objects of a list of references.
    fn objects(&self, name: &str) -> Result<Option<Vec<String>>, Violation> {
        self.get(name)
            .map(|list| {
                let list = list
                    .as_array()
                    .ok_or_else(|| invalid(self.operation, format!("{name} is a list")))?;
                let mut ids: Vec<String> = list
                    .iter()
                    .map(|r| self.object(r).map(|id| id.to_string()))
                    .collect::<Result<_, _>>()?;
                ids.sort_unstable();
                ids.dedup();
                Ok(ids)
            })
            .transpose()
    }
}

/// Checks the fields of `operation` against `known`; `id` is the object of an update.
fn known(operation: &ModuleOperation, known: &[&str]) -> Result<(), Violation> {
    let fields = ["local", "id", "completed"];
    for name in operation.body.keys() {
        if !known.contains(&name.as_str()) && !fields.contains(&name.as_str()) {
            return Err(invalid(operation, format!("unknown field {name}")));
        }
    }
    Ok(())
}

/// The columns of a flow type (REQ-010-4).
fn flow_type(f: &Fields<'_>, columns: &mut Map<String, Value>) -> Result<(), Violation> {
    if let Some(description) = f.get("description") {
        let text = description
            .as_str()
            .ok_or_else(|| invalid(f.operation, "description is a string"))?;
        if text.chars().count() > DESCRIPTION_MAX_CHARS
            || text.chars().any(|c| c.is_control() && c != '\n')
        {
            return Err(invalid(
                f.operation,
                "description too long or with control characters",
            ));
        }
        columns.insert("description".into(), text.into());
    }
    Ok(())
}

/// The columns of a service (REQ-010-5).
fn service(f: &Fields<'_>, columns: &mut Map<String, Value>) -> Result<(), Violation> {
    if let Some(flow_type) = f.get("flow_type") {
        columns.insert(
            "flow_type_id".into(),
            f.object(flow_type)?.to_string().into(),
        );
    }
    if let Some(entries) = f.get("entries") {
        let (checked, row) = codec::entries(entries)?;
        if checked.is_empty() {
            return Err(Violation::new(
                &crate::rules::SERVICE_ENTRIES_EMPTY,
                "a service has at least one entry",
            ));
        }
        columns.insert("entries".into(), row);
    }
    Ok(())
}

/// The texts of a pattern, a rule or a condition (S29).
fn texts(f: &Fields<'_>, columns: &mut Map<String, Value>) -> Result<(), Violation> {
    if let Some(texts) = f.get("texts") {
        columns.insert("texts".into(), codec::texts(texts)?.1);
    }
    Ok(())
}

/// The columns of a rule (REQ-010-8, S23 to S25).
fn rule(f: &Fields<'_>, columns: &mut Map<String, Value>) -> Result<(), Violation> {
    if let Some(effect) = f.get("effect") {
        let effect = match effect.as_str() {
            Some("allow") => "allow",
            Some("deny") => "deny",
            _ => return Err(invalid(f.operation, "effect is allow or deny")),
        };
        columns.insert("effect".into(), effect.into());
    }
    for side in ["source", "destination"] {
        if let Some(selector) = f.get(side) {
            let (terms, row) = codec::selector(selector)?;
            if let Some((axis, _)) = terms.iter().find(|(_, values)| values.is_empty()) {
                return Err(Violation::new(
                    &RULE_TERM_EMPTY,
                    format!("{axis} ({side}) admits no value"),
                ));
            }
            columns.insert(side.into(), row);
        }
    }
    if let Some(same) = f.get("same") {
        let axes: Vec<Value> = codec::axes(same)?
            .iter()
            .map(|a| a.to_string().into())
            .collect();
        columns.insert("same".into(), axes.into());
    }
    for (field, column) in [
        ("flow_types", "flow_type_ids"),
        ("services", "service_ids"),
        ("required_patterns", "required_pattern_ids"),
    ] {
        if let Some(ids) = f.objects(field)? {
            columns.insert(column.into(), ids.into());
        }
    }
    if let Some(pattern) = f.get("pattern") {
        columns.insert("pattern_id".into(), f.object(pattern)?.to_string().into());
    }
    texts(f, columns)
}

/// The columns of an admissibility condition (REQ-010-36).
fn condition(f: &Fields<'_>, columns: &mut Map<String, Value>) -> Result<(), Violation> {
    if let Some(kind) = f.get("kind") {
        let kind = kind
            .as_str()
            .ok_or_else(|| invalid(f.operation, "kind is a string"))?;
        let params = f.get("params").cloned().unwrap_or_else(|| json!({}));
        codec::condition(kind, &params)?;
        columns.insert("kind".into(), kind.into());
        columns.insert("params".into(), params);
    }
    for (field, column) in [("rules", "rule_ids"), ("flow_types", "flow_type_ids")] {
        if let Some(ids) = f.objects(field)? {
            columns.insert(column.into(), ids.into());
        }
    }
    texts(f, columns)
}

/// The columns of a signal (REQ-010-37).
fn signal(f: &Fields<'_>, columns: &mut Map<String, Value>) -> Result<(), Violation> {
    if let Some(kind) = f.get("kind") {
        let kind = kind
            .as_str()
            .ok_or_else(|| invalid(f.operation, "kind is a string"))?;
        let params = f.get("params").cloned().unwrap_or_else(|| json!({}));
        codec::signal(kind, &params)?;
        columns.insert("kind".into(), kind.into());
        columns.insert("params".into(), params);
    }
    if let Some(weight) = f.get("weight") {
        let weight = weight
            .as_i64()
            .and_then(|w| i32::try_from(w).ok())
            .ok_or_else(|| invalid(f.operation, "weight is an integer"))?;
        columns.insert("weight".into(), weight.into());
    }
    Ok(())
}

/// The columns of an exempt role (S10): a key of `machines.interface_role`.
fn exempt_role(f: &Fields<'_>, columns: &mut Map<String, Value>) -> Result<(), Violation> {
    if let Some(role) = f.get("role") {
        let role = role
            .as_str()
            .filter(|r| {
                !r.is_empty()
                    && r.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    && !r.starts_with('-')
                    && !r.ends_with('-')
            })
            .ok_or_else(|| invalid(f.operation, "role is the key of a role of interface"))?;
        columns.insert("role_key".into(), role.into());
    }
    Ok(())
}

/// What a type of object takes: its fields beyond key and label, its columns, the
/// columns its creation requires, and the defaults of the columns a creation may omit.
struct Spec {
    kind: &'static str,
    keyed: bool,
    fields: &'static [&'static str],
    columns: fn(&Fields<'_>, &mut Map<String, Value>) -> Result<(), Violation>,
    required: &'static [&'static str],
    defaults: fn() -> Value,
}

const SPECS: &[Spec] = &[
    Spec {
        kind: FLOW_TYPE,
        keyed: true,
        fields: &["description"],
        columns: flow_type,
        required: &[],
        defaults: || json!({ "description": null }),
    },
    Spec {
        kind: SERVICE,
        keyed: true,
        fields: &["flow_type", "entries"],
        columns: service,
        required: &["flow_type_id", "entries"],
        defaults: || json!({}),
    },
    Spec {
        kind: PATTERN,
        keyed: true,
        fields: &["texts"],
        columns: texts,
        required: &[],
        defaults: || json!({ "texts": {} }),
    },
    Spec {
        kind: RULE,
        keyed: true,
        fields: &[
            "effect",
            "source",
            "destination",
            "same",
            "flow_types",
            "services",
            "pattern",
            "required_patterns",
            "texts",
        ],
        columns: rule,
        required: &["effect"],
        defaults: || {
            json!({
                "source": {}, "destination": {}, "same": [], "flow_type_ids": [],
                "service_ids": [], "pattern_id": null, "required_pattern_ids": [], "texts": {},
            })
        },
    },
    Spec {
        kind: CONDITION,
        keyed: true,
        fields: &["kind", "params", "rules", "flow_types", "texts"],
        columns: condition,
        required: &["kind"],
        defaults: || json!({ "params": {}, "rule_ids": [], "flow_type_ids": [], "texts": {} }),
    },
    Spec {
        kind: SIGNAL,
        keyed: true,
        fields: &["kind", "params", "weight"],
        columns: signal,
        required: &["kind", "weight"],
        defaults: || json!({ "params": {} }),
    },
    Spec {
        kind: EXEMPT_ROLE,
        keyed: false,
        fields: &["role"],
        columns: exempt_role,
        required: &["role_key"],
        defaults: || json!({}),
    },
];

/// The local checks of a rule once written whole: a flow selector, no pattern required
/// by a deny (REQ-010-8, S25).
fn whole_rule(operation: &ModuleOperation, row: &Map<String, Value>) -> Result<(), Violation> {
    let empty = |name: &str| {
        row.get(name)
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    };
    if empty("flow_type_ids") && empty("service_ids") {
        return Err(Violation::new(
            &RULE_FLOW_SELECTOR_EMPTY,
            format!(
                "{}: the rule cites no flow type nor service",
                operation.tag()
            ),
        ));
    }
    let deny = row.get("effect").and_then(Value::as_str) == Some("deny");
    if deny && !empty("required_pattern_ids") {
        return Err(Violation::new(
            &RULE_CONDITIONAL_DENY,
            format!("{}: a deny requires no pattern", operation.tag()),
        ));
    }
    Ok(())
}

/// The plan of `operation` (`<type>.create` or `<type>.update`), or `None` when it is not
/// an operation on an object of the catalogue.
pub(crate) fn plan_object(
    operation: &ModuleOperation,
    object: Uuid,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Option<Result<ModulePlan, Violation>> {
    let (kind, action) = operation.name.rsplit_once('.')?;
    let spec = SPECS
        .iter()
        .find(|s| s.kind.strip_prefix("policy.") == Some(kind))?;
    Some(plan_with(spec, action, operation, object, resolve))
}

fn plan_with(
    spec: &Spec,
    action: &str,
    operation: &ModuleOperation,
    object: Uuid,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    // A key for every object but an exempt role; a label but on a condition and a signal
    // (design « Données »).
    let named: &[&str] = match spec.kind {
        _ if !spec.keyed => &[],
        CONDITION | SIGNAL => &["key"],
        _ => &["key", "label"],
    };
    known(operation, &[named, spec.fields].concat())?;
    let f = Fields { operation, resolve };
    let mut columns = Map::new();
    if named.contains(&"key")
        && let Some(key) = f.key()?
    {
        columns.insert("key".into(), key.into());
    }
    if named.contains(&"label")
        && let Some(label) = f.label()?
    {
        // Until the writes take `labels` (task 018-7), the label is that of the default
        // language.
        columns.insert(
            "labels".into(),
            serde_json::json!({ kartenn_core::labels::default_locale().as_str(): &label }),
        );
        columns.insert("label".into(), label.into());
    }
    (spec.columns)(&f, &mut columns)?;
    match action {
        "create" => {
            let mut row = match (spec.defaults)() {
                Value::Object(defaults) => defaults,
                _ => Map::new(),
            };
            row.extend(columns);
            for column in named.iter().chain(spec.required) {
                if row.get(*column).is_none_or(Value::is_null) {
                    return Err(match *column {
                        // REQ-010-5: a service has exactly one flow type, at least one entry.
                        "flow_type_id" => Violation::new(
                            &crate::rules::SERVICE_FLOW_TYPE,
                            format!("{}: a service has exactly one flow type", operation.tag()),
                        ),
                        "entries" => Violation::new(
                            &crate::rules::SERVICE_ENTRIES_EMPTY,
                            format!("{}: a service has at least one entry", operation.tag()),
                        ),
                        _ => invalid(operation, format!("{column} is required")),
                    });
                }
            }
            if spec.kind == RULE {
                whole_rule(operation, &row)?;
            }
            row.insert("id".into(), object.to_string().into());
            Ok(plan(spec.kind, object, true, json!({ "create": row })))
        }
        "update" => {
            let id = f.required("id")?.clone();
            let id = f.object(&id)?;
            if spec.kind == RULE
                && let Some(Value::String(effect)) = columns.get("effect")
                && effect == "deny"
                && columns
                    .get("required_pattern_ids")
                    .and_then(Value::as_array)
                    .is_some_and(|p| !p.is_empty())
            {
                return Err(Violation::new(
                    &RULE_CONDITIONAL_DENY,
                    format!("{}: a deny requires no pattern", operation.tag()),
                ));
            }
            Ok(plan(spec.kind, id, false, json!({ "update": columns })))
        }
        _ => Err(invalid(operation, "unknown operation")),
    }
}

/// The columns whose value is unique over the period for objects of `kind`.
pub(crate) fn unique_columns(kind: &str) -> &'static [&'static str] {
    match kind {
        crate::module::AXIS => &["property_key"],
        crate::module::VALUE => &["property_key", "value_key"],
        EXEMPT_ROLE => &["role_key"],
        _ => &["key"],
    }
}
