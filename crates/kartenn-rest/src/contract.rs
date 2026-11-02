//! The comparator of contracts (spec 006, REQ-006-34; task 006-22; design « Versions et
//! contrat »): what the contract a build produces breaks of the reference contract of the
//! repository. `/v1` only grows (H12): a path, an operation, a parameter, a member or a
//! schema removed, a type changed, a member made required in a request, or no longer
//! guaranteed in a response, is a breach. What is added is not: it only asks for the
//! reference to be written again (`cargo run -p kartenn-server --all-features -- openapi`).
//!
//! No mature Rust crate compares two OpenAPI documents (design « Crates et
//! dépendances »): this one reads them as JSON.

use std::collections::BTreeSet;
use std::fmt;

use serde_json::Value;

use crate::openapi::QUERY_EXTENSION;

/// The methods of a path item, `QUERY` in its extension.
const METHODS: [&str; 9] = [
    "get",
    "put",
    "post",
    "delete",
    "patch",
    "head",
    "options",
    "trace",
    QUERY_EXTENSION,
];

/// What breaks the reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breach {
    /// Where, as a path into the reference.
    pub at: String,
    /// What.
    pub what: String,
}

impl fmt::Display for Breach {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.at, self.what)
    }
}

/// Which way a schema travels: what a client sends may not narrow, what it receives may
/// not widen; a component may travel both ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    Request,
    Response,
    Both,
}

impl Way {
    const fn sent(self) -> bool {
        matches!(self, Self::Request | Self::Both)
    }

    const fn received(self) -> bool {
        matches!(self, Self::Response | Self::Both)
    }
}

/// The breaches of `produced` against `reference`, both OpenAPI documents as JSON.
#[must_use]
pub fn breaches(reference: &Value, produced: &Value) -> Vec<Breach> {
    let mut found = Breaches {
        found: Vec::new(),
        reference,
        produced,
    };
    let members = |v: &Value, k: &str| v.get(k).and_then(Value::as_object).cloned();
    let paths = members(reference, "paths").unwrap_or_default();
    let now = members(produced, "paths").unwrap_or_default();
    for (path, item) in &paths {
        let at = format!("paths.{path}");
        let Some(current) = now.get(path) else {
            found.add(&at, "path removed");
            continue;
        };
        for method in METHODS {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let at = format!("{at}.{method}");
            match current.get(method) {
                None => found.add(&at, "operation removed"),
                Some(now) => found.operation(&at, operation, now),
            }
        }
    }
    let components = |v: &Value, section: &str| {
        v.pointer(&format!("/components/{section}"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    };
    let now = components(produced, "parameters");
    for (name, parameter) in &components(reference, "parameters") {
        let at = format!("components.parameters.{name}");
        match (resolve(reference, parameter), now.get(name)) {
            (_, None) => found.add(&at, "parameter removed"),
            (None, Some(_)) => {}
            (Some(was), Some(now)) => match resolve(produced, now) {
                None => found.add(&at, "reference not resolved"),
                Some(is) => found.parameter(&at, was, is),
            },
        }
    }
    let now = components(produced, "schemas");
    for (name, schema) in &components(reference, "schemas") {
        let at = format!("components.schemas.{name}");
        match now.get(name) {
            None => found.add(&at, "schema removed"),
            Some(now) => found.schema(&at, schema, now, Way::Both),
        }
    }
    found.found
}

/// The breaches found, and the two documents their local references are resolved in.
struct Breaches<'a> {
    found: Vec<Breach>,
    reference: &'a Value,
    produced: &'a Value,
}

/// How many local references [`resolve`] follows before it gives up on a loop.
const HOPS: usize = 8;

/// What `value` stands for in `document`: the target of its local reference `$ref`
/// (`#/…`, a JSON pointer, RFC 6901 §6), followed through the references the target holds
/// in turn, or `value` itself when it is no reference. `None` when the reference is not
/// local, names nothing or loops.
fn resolve<'a>(document: &'a Value, value: &'a Value) -> Option<&'a Value> {
    let mut current = value;
    for _ in 0..HOPS {
        let Some(target) = current.get("$ref") else {
            return Some(current);
        };
        current = document.pointer(target.as_str()?.strip_prefix('#')?)?;
    }
    None
}

/// The identity of a parameter of an operation of `document`: its name and its place,
/// once its reference resolved; the reference itself, at the place `$ref`, when it names
/// nothing, so that two references never stand for one another.
fn parameter_key(document: &Value, parameter: &Value) -> (String, String) {
    let text = |p: &Value, member: &str| {
        p.get(member)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    match resolve(document, parameter) {
        Some(p) => (text(p, "name"), text(p, "in")),
        None => (text(parameter, "$ref"), "$ref".to_owned()),
    }
}

/// Whether `parameter` is a reference that one of `others` makes too: the component
/// it names is compared once, under `components.parameters`.
fn shared_reference(parameter: &Value, others: &[Value]) -> bool {
    parameter
        .get("$ref")
        .is_some_and(|r| others.iter().any(|p| p.get("$ref") == Some(r)))
}

/// The types a schema allows: `type` as a string or an array.
fn types(schema: &Value) -> BTreeSet<String> {
    match schema.get("type") {
        Some(Value::String(t)) => BTreeSet::from([t.clone()]),
        Some(Value::Array(ts)) => ts
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => BTreeSet::new(),
    }
}

fn strings(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect()
}

fn required(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool).unwrap_or(false)
}

impl Breaches<'_> {
    fn add(&mut self, at: &str, what: impl Into<String>) {
        self.found.push(Breach {
            at: at.to_owned(),
            what: what.into(),
        });
    }

    /// Compares two parameters, their references resolved.
    fn parameter(&mut self, at: &str, reference: &Value, produced: &Value) {
        for member in ["name", "in"] {
            if let Some(was) = reference.get(member)
                && produced.get(member) != Some(was)
            {
                self.add(at, format!("{member} changed from {was}"));
            }
        }
        if required(produced.get("required")) && !required(reference.get("required")) {
            self.add(at, "parameter now required");
        }
        if let (Some(was), Some(is)) = (reference.get("schema"), produced.get("schema")) {
            self.schema(&format!("{at}.schema"), was, is, Way::Request);
        }
    }

    fn operation(&mut self, at: &str, reference: &Value, produced: &Value) {
        let (then, now) = (self.reference, self.produced);
        let list = |v: &Value| {
            v.get("parameters")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let (before, after) = (list(reference), list(produced));
        for parameter in &before {
            if shared_reference(parameter, &after) {
                continue;
            }
            let key = parameter_key(then, parameter);
            let at = format!("{at}.parameters.{}.{}", key.1, key.0);
            match after.iter().find(|p| parameter_key(now, p) == key) {
                None => self.add(&at, "parameter removed"),
                Some(is) => {
                    if let (Some(was), Some(is)) = (resolve(then, parameter), resolve(now, is)) {
                        self.parameter(&at, was, is);
                    }
                }
            }
        }
        for parameter in &after {
            if shared_reference(parameter, &before) {
                continue;
            }
            let key = parameter_key(now, parameter);
            let known = before.iter().any(|p| parameter_key(then, p) == key);
            let is_required = resolve(now, parameter).is_some_and(|p| required(p.get("required")));
            if !known && is_required {
                self.add(
                    &format!("{at}.parameters.{}.{}", key.1, key.0),
                    "required parameter added",
                );
            }
        }
        let body = |v: &Value| v.get("requestBody").cloned();
        match (body(reference), body(produced)) {
            (Some(was), Some(is)) => {
                let at = format!("{at}.requestBody");
                if required(is.get("required")) && !required(was.get("required")) {
                    self.add(&at, "body now required");
                }
                self.contents(&at, &was, &is, Way::Request);
            }
            (Some(_), None) => self.add(&format!("{at}.requestBody"), "body removed"),
            (None, Some(is)) if required(is.get("required")) => {
                self.add(&format!("{at}.requestBody"), "required body added");
            }
            _ => {}
        }
        let responses = |v: &Value| {
            v.get("responses")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default()
        };
        let now = responses(produced);
        for (status, response) in responses(reference) {
            let at = format!("{at}.responses.{status}");
            match now.get(&status) {
                None => self.add(&at, "response removed"),
                Some(is) => self.contents(&at, &response, is, Way::Response),
            }
        }
    }

    fn contents(&mut self, at: &str, reference: &Value, produced: &Value, way: Way) {
        let Some(before) = reference.get("content").and_then(Value::as_object) else {
            return;
        };
        for (media, content) in before {
            let at = format!("{at}.content.{media}");
            match produced.pointer(&format!("/content/{}", media.replace('/', "~1"))) {
                None => self.add(&at, "media type removed"),
                Some(now) => {
                    if let (Some(was), Some(is)) = (content.get("schema"), now.get("schema")) {
                        self.schema(&format!("{at}.schema"), was, is, way);
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn schema(&mut self, at: &str, reference: &Value, produced: &Value, way: Way) {
        if let Some(name) = reference.get("$ref") {
            if produced.get("$ref") != Some(name) {
                self.add(at, format!("type changed from {name}"));
            }
            return;
        }
        if produced.get("$ref").is_some() {
            self.add(at, "type changed to a reference");
            return;
        }
        let (before, after) = (types(reference), types(produced));
        if !before.is_empty() || !after.is_empty() {
            let narrowed = way.sent() && !before.is_subset(&after);
            let widened = way.received() && !after.is_subset(&before);
            if narrowed || widened {
                self.add(at, format!("type changed from {before:?} to {after:?}"));
            }
        }
        if let Some(format) = reference.get("format")
            && produced.get("format") != Some(format)
        {
            self.add(at, format!("format changed from {format}"));
        }
        let was = strings(reference.get("required"));
        let is = strings(produced.get("required"));
        if way.sent() {
            for name in is.difference(&was) {
                self.add(&format!("{at}.{name}"), "member now required");
            }
        }
        if way.received() {
            for name in was.difference(&is) {
                self.add(&format!("{at}.{name}"), "member no longer guaranteed");
            }
        }
        if way.sent() && reference.get("enum").is_some() {
            let now = strings(produced.get("enum"));
            for value in strings(reference.get("enum")).difference(&now) {
                self.add(at, format!("value {value} removed"));
            }
        }
        if let Some(members) = reference.get("properties").and_then(Value::as_object) {
            for (name, member) in members {
                let at = format!("{at}.{name}");
                match produced.pointer(&format!(
                    "/properties/{}",
                    name.replace('~', "~0").replace('/', "~1")
                )) {
                    None => self.add(&at, "member removed"),
                    Some(now) => self.schema(&at, member, now, way),
                }
            }
        }
        for nested in ["items", "additionalProperties"] {
            if let (Some(was), Some(is)) = (reference.get(nested), produced.get(nested))
                && was.is_object()
                && is.is_object()
            {
                self.schema(&format!("{at}.{nested}"), was, is, way);
            }
        }
        for alternatives in ["oneOf", "anyOf", "allOf"] {
            let list = |v: &Value| {
                v.get(alternatives)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            };
            let (before, after) = (list(reference), list(produced));
            if before.is_empty() && after.is_empty() {
                continue;
            }
            let at = format!("{at}.{alternatives}");
            if after.len() < before.len() && (way.sent() || alternatives == "allOf") {
                self.add(&at, "alternative removed");
            }
            if after.len() > before.len() && (way.received() || alternatives == "allOf") {
                self.add(&at, "alternative added");
            }
            for (n, (was, is)) in before.iter().zip(&after).enumerate() {
                self.schema(&format!("{at}.{n}"), was, is, way);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn document(schema: &Value, parameter_required: bool) -> Value {
        json!({
            "paths": {
                "/v1/things": {
                    "get": {
                        "parameters": [{"name": "q", "in": "query", "required": parameter_required,
                                        "schema": {"type": "string"}}],
                        "responses": {"200": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/Thing"}}}}}
                    }
                }
            },
            "components": {"schemas": {"Thing": schema.clone()}}
        })
    }

    fn thing() -> Value {
        json!({"type": "object", "required": ["id"], "properties": {
            "id": {"type": "string", "format": "uuid"},
            "name": {"type": ["string", "null"]},
            "usage": {"type": "string", "enum": ["a", "b"]},
        }})
    }

    #[test]
    fn the_same_document_breaks_nothing() {
        let d = document(&thing(), false);
        assert_eq!(breaches(&d, &d), []);
    }

    #[test]
    fn a_member_removed_breaks() {
        let mut changed = thing();
        changed["properties"]
            .as_object_mut()
            .unwrap()
            .remove("name");
        let found = breaches(&document(&thing(), false), &document(&changed, false));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].at, "components.schemas.Thing.name");
    }

    #[test]
    fn a_member_added_breaks_nothing() {
        let mut changed = thing();
        changed["properties"]["more"] = json!({"type": "integer"});
        assert_eq!(
            breaches(&document(&thing(), false), &document(&changed, false)),
            []
        );
    }

    #[test]
    fn a_type_changed_breaks() {
        let mut changed = thing();
        changed["properties"]["id"] = json!({"type": "integer"});
        assert!(!breaches(&document(&thing(), false), &document(&changed, false)).is_empty());
    }

    #[test]
    fn a_parameter_made_required_breaks() {
        let found = breaches(&document(&thing(), false), &document(&thing(), true));
        assert_eq!(found[0].what, "parameter now required");
    }

    #[test]
    fn an_operation_or_a_query_removed_breaks() {
        let mut reference = document(&thing(), false);
        reference["paths"]["/v1/things"][QUERY_EXTENSION] = json!({"responses": {}});
        let mut produced = reference.clone();
        produced["paths"]["/v1/things"]
            .as_object_mut()
            .unwrap()
            .remove(QUERY_EXTENSION);
        let found = breaches(&reference, &produced);
        assert_eq!(found[0].what, "operation removed");
        produced["paths"]
            .as_object_mut()
            .unwrap()
            .remove("/v1/things");
        assert_eq!(breaches(&reference, &produced)[0].what, "path removed");
    }

    #[test]
    fn an_enumeration_narrowed_breaks_what_is_sent() {
        let mut changed = thing();
        changed["properties"]["usage"]["enum"] = json!(["a"]);
        let found = breaches(&document(&thing(), false), &document(&changed, false));
        assert_eq!(found[0].what, "value b removed");
    }

    /// A document whose operation takes the parameters `refs` by reference, among the
    /// components `Background` and `Other`; `Background` required or not.
    fn referring(refs: &[&str], background_required: bool) -> Value {
        let mut d = document(&thing(), false);
        d["paths"]["/v1/things"]["get"]["parameters"] = refs
            .iter()
            .map(|name| json!({"$ref": format!("#/components/parameters/{name}")}))
            .collect();
        d["components"]["parameters"] = json!({
            "Background": {"name": "Kartenn-Background", "in": "header",
                           "required": background_required, "schema": {"type": "string"}},
            "Other": {"name": "Kartenn-Other", "in": "header", "schema": {"type": "string"}},
        });
        d
    }

    #[test]
    fn a_referred_parameter_is_compared_through_its_reference() {
        let d = referring(&["Background"], false);
        assert_eq!(breaches(&d, &d), []);
        let found = breaches(&d, &referring(&["Background"], true));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].at, "components.parameters.Background");
        assert_eq!(found[0].what, "parameter now required");
    }

    #[test]
    fn a_component_parameter_changed_or_removed_breaks() {
        let reference = referring(&["Background"], false);
        let mut produced = reference.clone();
        produced["components"]["parameters"]["Background"]["schema"] = json!({"type": "integer"});
        let found = breaches(&reference, &produced);
        assert_eq!(found[0].at, "components.parameters.Background.schema");
        produced["components"]["parameters"]
            .as_object_mut()
            .unwrap()
            .remove("Background");
        assert_eq!(breaches(&reference, &produced)[0].what, "parameter removed");
    }

    #[test]
    fn two_references_are_two_parameters() {
        let found = breaches(
            &referring(&["Background"], false),
            &referring(&["Other"], false),
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].at,
            "paths./v1/things.get.parameters.header.Kartenn-Background"
        );
        assert_eq!(found[0].what, "parameter removed");
        let found = breaches(
            &referring(&["Other"], false),
            &referring(&["Other", "Background"], true),
        );
        assert_eq!(found[0].what, "required parameter added");
    }

    #[test]
    fn a_parameter_written_out_matches_its_reference() {
        let reference = referring(&["Background"], false);
        let mut produced = reference.clone();
        produced["paths"]["/v1/things"]["get"]["parameters"] = json!([
            {"name": "Kartenn-Background", "in": "header", "required": true,
             "schema": {"type": "string"}}
        ]);
        let found = breaches(&reference, &produced);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].what, "parameter now required");
    }
}
