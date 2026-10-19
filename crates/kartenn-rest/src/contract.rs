//! The comparator of contracts (spec 006, REQ-006-34; task 006-22; design « Versions et
//! contrat »): what the contract a build produces breaks of the reference contract of the
//! repository. `/v1` only grows (H12): a path, an operation, a parameter, a member or a
//! schema removed, a type changed, a member made required in a request, or no longer
//! guaranteed in a response, is a breach. What is added is not: it only asks for the
//! reference to be written again (`cargo run -p kartenn-rest --bin openapi`).
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
    let mut found = Breaches(Vec::new());
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
    let schemas = |v: &Value| {
        v.pointer("/components/schemas")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    };
    let (reference, produced) = (schemas(reference), schemas(produced));
    for (name, schema) in &reference {
        let at = format!("components.schemas.{name}");
        match produced.get(name) {
            None => found.add(&at, "schema removed"),
            Some(now) => found.schema(&at, schema, now, Way::Both),
        }
    }
    found.0
}

struct Breaches(Vec<Breach>);

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

impl Breaches {
    fn add(&mut self, at: &str, what: impl Into<String>) {
        self.0.push(Breach {
            at: at.to_owned(),
            what: what.into(),
        });
    }

    fn operation(&mut self, at: &str, reference: &Value, produced: &Value) {
        let key = |p: &Value| {
            (
                p.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                p.get("in")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )
        };
        let list = |v: &Value| {
            v.get("parameters")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let (before, after) = (list(reference), list(produced));
        for parameter in &before {
            let (name, place) = key(parameter);
            let at = format!("{at}.parameters.{place}.{name}");
            match after
                .iter()
                .find(|p| key(p) == (name.clone(), place.clone()))
            {
                None => self.add(&at, "parameter removed"),
                Some(now) => {
                    if required(now.get("required")) && !required(parameter.get("required")) {
                        self.add(&at, "parameter now required");
                    }
                    if let (Some(was), Some(is)) = (parameter.get("schema"), now.get("schema")) {
                        self.schema(&format!("{at}.schema"), was, is, Way::Request);
                    }
                }
            }
        }
        for now in &after {
            let (name, place) = key(now);
            let known = before
                .iter()
                .any(|p| key(p) == (name.clone(), place.clone()));
            if !known && required(now.get("required")) {
                self.add(
                    &format!("{at}.parameters.{place}.{name}"),
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
}
