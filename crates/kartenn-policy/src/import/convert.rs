//! Conversion of the policy of origin into the operations of one changeset (REQ-010-35,
//! 53), and into the same policy in memory for the simulation.
//!
//! - Every environment named by the matrix or by a label becomes a value of
//!   `policy.environment` (`environment.create`), unless declared existing.
//! - Every flow type and every service of the catalogue is created; a service without a
//!   flow type takes the default one, or is left out.
//! - « Autorisé = faux » becomes a `deny` rule on the service, without endpoint selector
//!   (S6); its description becomes the pedagogical text.
//! - Each cell becomes `policy.matrix.set`: `allow`, `deny` (with a text), or « partiel »
//!   as `conditional` under one pattern of manual review (Q11).
//! - Each rule of inference becomes `policy.inference_rule.create`; the reading order
//!   (files by name, tables in written order) becomes explicit priorities, the later one
//!   higher, which reproduces « the later read wins » (I6, Q12); an exception becomes a
//!   child rule; a search in a name becomes an anchored expression of the same meaning;
//!   a site expression becomes the list of the matching sites.
//!
//! What cannot be converted is left out and said in a notice.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::source::{Cell, CellState, Constraint, PortRow, ServiceRow, Site, SourceRule};
use crate::codec;
use crate::infer::criterion;
use crate::inference::{InferenceRule, MAX_DEPTH, NameRegex};
use crate::model::{
    Axis, AxisKey, AxisValue, Carrier, Effect, FlowSelector, FlowType, Key, Locale, Pattern, Rule,
    Selector, Service, Text, Texts,
};
use crate::module::ENVIRONMENT;
use crate::snapshot::Parts;

/// Key of the pattern « partiel » cells require.
pub const REVIEW_PATTERN: &str = "revue-import";

/// The inputs of a conversion.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Cells of the matrix.
    pub matrix: Vec<Cell>,
    /// First table of the catalogue.
    pub ports: Vec<PortRow>,
    /// Second table of the catalogue.
    pub services: Vec<ServiceRow>,
    /// Rules of inference, in reading order.
    pub rules: Vec<SourceRule>,
    /// Sites, for the site constraints.
    pub sites: Vec<Site>,
}

/// Options of a conversion.
#[derive(Debug, Clone)]
pub struct Options {
    /// The axis the rules of inference expect values on.
    pub axis: AxisKey,
    /// Flow type of a service the catalogue gives none.
    pub default_flow_type: Option<String>,
    /// Environments that already exist in the instance: not created.
    pub existing_environments: BTreeSet<String>,
    /// Language of the texts written (the default language of the instance).
    pub locale: Locale,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            axis: AxisKey::parse(ENVIRONMENT).expect("a property key"),
            default_flow_type: None,
            existing_environments: BTreeSet::new(),
            locale: kartenn_core::labels::default_locale().clone(),
        }
    }
}

/// A converted service: its key, its flow type, its name of origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedService {
    /// Key.
    pub key: Key,
    /// Flow type.
    pub flow_type: Key,
    /// Name in the catalogue of origin.
    pub name: String,
}

/// The result of a conversion.
#[derive(Debug, Clone)]
pub struct Conversion {
    /// The operations of the changeset, in order (`{op, …}` as `POST /v1/changesets`).
    pub operations: Vec<Value>,
    /// What was left out or changed, and why.
    pub notices: Vec<String>,
    /// The converted policy, as the instance will hold it once the changeset applied.
    pub parts: Parts,
    /// The converted rules of inference.
    pub inference: Vec<InferenceRule>,
    /// Environment key of each environment name.
    pub environments: BTreeMap<String, String>,
    /// Flow type key of each flow type name.
    pub flow_types: BTreeMap<String, Key>,
    /// The converted services.
    pub services: Vec<ConvertedService>,
}

/// A key `[a-z0-9-]` from a name of origin, or none when nothing is left.
#[must_use]
pub fn slug(name: &str) -> Option<String> {
    let mut out = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out = out.trim_end_matches('-').to_owned();
    out.truncate(63);
    let out = out.trim_end_matches('-').to_owned();
    (!out.is_empty()).then_some(out)
}

/// An anchored expression of the same meaning as a search of `source` in a name: the tool
/// of origin searched, the inference matches the whole name (REQ-010-42).
#[must_use]
pub fn anchored(source: &str) -> String {
    let (start, body) = source
        .strip_prefix('^')
        .map_or((false, source), |rest| (true, rest));
    let unescaped_end = body.ends_with('$') && !body.ends_with("\\$");
    let body = if unescaped_end {
        &body[..body.len() - 1]
    } else {
        body
    };
    format!(
        "^{}(?:{body}){}$",
        if start { "" } else { ".*" },
        if unescaped_end { "" } else { ".*" }
    )
}

struct Builder<'a> {
    options: &'a Options,
    operations: Vec<Value>,
    notices: Vec<String>,
    next_local: u32,
    keys: BTreeSet<String>,
}

impl Builder<'_> {
    fn local(&mut self) -> u32 {
        self.next_local += 1;
        self.next_local
    }

    fn push(&mut self, name: &str, fields: Value) {
        let mut object = fields;
        object["op"] = format!("policy.{name}").into();
        self.operations.push(object);
    }

    /// A key not yet used by this conversion, from `base`.
    fn unique(&mut self, base: &str) -> String {
        let mut key = base.to_owned();
        let mut n = 1;
        while !self.keys.insert(key.clone()) {
            n += 1;
            let suffix = format!("-{n}");
            let mut stem = base.to_owned();
            stem.truncate(63 - suffix.len());
            key = format!("{}{suffix}", stem.trim_end_matches('-'));
        }
        key
    }

    fn text(&self, body: &str) -> (Texts, Value) {
        let text = Text::new(body, None).expect("a short text");
        (
            Texts(BTreeMap::from([(self.options.locale.clone(), text)])),
            json!({ self.options.locale.as_str(): { "body": body } }),
        )
    }
}

/// The labels of a rule and of its exceptions.
fn collect(rule: &SourceRule, out: &mut Vec<String>) {
    out.extend(rule.labels.iter().cloned());
    for e in &rule.exceptions {
        collect(e, out);
    }
}

/// The labels of an object named `name` at origin, in the default language (spec 018).
fn labels_of(name: &str) -> serde_json::Value {
    let text = kartenn_core::labels::label_text(name).unwrap_or_else(|_| "(sans libellé)".into());
    json!({ kartenn_core::labels::default_locale().as_str(): text })
}

/// Converts the policy of origin (see the module documentation).
///
/// # Panics
/// Never: the keys and texts it builds are checked by construction.
#[must_use]
#[allow(clippy::too_many_lines)] // one pass per part of the input, in the changeset order
pub fn convert(inputs: &Inputs, options: &Options) -> Conversion {
    let mut b = Builder {
        options,
        operations: Vec::new(),
        notices: Vec::new(),
        next_local: 0,
        keys: BTreeSet::new(),
    };

    // Environments: those of the matrix, then those of the labels.
    let mut environments = BTreeMap::new();
    let mut labels = Vec::new();
    for rule in &inputs.rules {
        collect(rule, &mut labels);
    }
    let names = inputs
        .matrix
        .iter()
        .flat_map(|c| [c.source.clone(), c.destination.clone()])
        .chain(labels);
    for name in names {
        if environments.contains_key(&name) {
            continue;
        }
        let Some(key) = slug(&name) else {
            b.notices.push(format!(
                "environnement {name:?} : aucune clé possible, écarté"
            ));
            continue;
        };
        if key != name {
            b.notices
                .push(format!("environnement {name:?} : clé {key}"));
        }
        environments.insert(name, key);
    }
    let created: BTreeSet<&String> = environments.values().collect();
    for key in created {
        if !options.existing_environments.contains(key) {
            b.push("environment.create", json!({ "key": key }));
        }
    }

    // Flow types: those of the matrix, of the catalogue, and the default one.
    let mut flow_types: BTreeMap<String, (Key, u32)> = BTreeMap::new();
    let mut parts_flow_types = Vec::new();
    let names = inputs
        .matrix
        .iter()
        .map(|c| c.flow_type.clone())
        .chain(inputs.services.iter().filter_map(|s| s.flow_type.clone()))
        .chain(options.default_flow_type.clone());
    for name in names {
        if flow_types.contains_key(&name) {
            continue;
        }
        let Some(key) = slug(&name).and_then(|k| Key::new(&k).ok()) else {
            b.notices.push(format!(
                "type de flux {name:?} : aucune clé possible, écarté"
            ));
            continue;
        };
        let key = Key::new(&b.unique(key.as_str())).expect("a key");
        let local = b.local();
        b.push(
            "flow_type.create",
            json!({ "local": local, "key": key.as_str(), "labels": labels_of(&name) }),
        );
        parts_flow_types.push(FlowType { key: key.clone() });
        flow_types.insert(name, (key, local));
    }

    // Services: their entries from the first table, their status from the second.
    let mut entries: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    for row in &inputs.ports {
        let mut entry = json!({ "transport": row.transport });
        if let Some((first, last)) = row.ports {
            entry["ports"] = json!([first, last]);
        }
        entries.entry(row.service.as_str()).or_default().push(entry);
    }
    let status: BTreeMap<&str, &ServiceRow> = inputs
        .services
        .iter()
        .map(|s| (s.service.as_str(), s))
        .collect();
    for name in status.keys().filter(|n| !entries.contains_key(*n)) {
        b.notices.push(format!(
            "service {name:?} : aucun port dans la première table, écarté"
        ));
    }
    let mut services = Vec::new();
    let mut parts_services = Vec::new();
    let mut parts_rules = Vec::new();
    for (name, list) in &entries {
        let row = status.get(name);
        if row.is_none() {
            b.notices.push(format!(
                "service {name:?} : absent de la seconde table, tenu pour autorisé"
            ));
        }
        let Some(flow_name) = row
            .and_then(|r| r.flow_type.clone())
            .or_else(|| options.default_flow_type.clone())
        else {
            b.notices.push(format!(
                "service {name:?} : aucun type de flux (colonne flow_type ou --default-flow-type), écarté"
            ));
            continue;
        };
        let Some((flow_type, flow_local)) = flow_types.get(&flow_name).cloned() else {
            continue;
        };
        let Some(key) = slug(name).and_then(|k| Key::new(&k).ok()) else {
            b.notices
                .push(format!("service {name:?} : aucune clé possible, écarté"));
            continue;
        };
        let entries_json = Value::Array(list.clone());
        let checked = match codec::entries(&entries_json) {
            Ok((checked, _)) => checked,
            Err(v) => {
                b.notices
                    .push(format!("service {name:?} : {}, écarté", v.message));
                continue;
            }
        };
        let key = Key::new(&b.unique(key.as_str())).expect("a key");
        let Ok(service) = Service::new(key.clone(), std::slice::from_ref(&flow_type), checked)
        else {
            continue;
        };
        let local = b.local();
        b.push(
            "service.create",
            json!({ "local": local, "key": key.as_str(), "labels": labels_of(name),
                    "flow_type": { "local": flow_local }, "entries": entries_json }),
        );
        parts_services.push(service);
        services.push(ConvertedService {
            key: key.clone(),
            flow_type: flow_type.clone(),
            name: (*name).to_owned(),
        });
        if let Some(row) = row.filter(|r| !r.allowed) {
            let body = row
                .description
                .clone()
                .unwrap_or_else(|| format!("Le service {name} est interdit au catalogue."));
            let (texts, texts_json) = b.text(&body);
            let rule_key = Key::new(&b.unique(&format!("catalogue-deny-{}", key.as_str())))
                .unwrap_or_else(|_| Key::new(&b.unique("catalogue-deny")).expect("a key"));
            let rule_labels = labels_of(&format!("Interdit au catalogue : {name}"));
            let rule_local = b.local();
            b.push(
                "rule.create",
                json!({ "local": rule_local, "key": rule_key.as_str(),
                        "labels": rule_labels, "effect": "deny",
                        "services": [{ "local": local }], "texts": texts_json }),
            );
            parts_rules.push(Rule {
                key: rule_key,
                source: Selector::new(),
                destination: Selector::new(),
                same: BTreeSet::new(),
                flows: FlowSelector {
                    flow_types: BTreeSet::new(),
                    services: BTreeSet::from([key]),
                },
                effect: Effect::Deny,
                required_patterns: BTreeSet::new(),
                pattern: None,
                texts,
            });
        }
    }

    // The pattern of manual review, when a cell is « partiel ».
    let mut parts_patterns = Vec::new();
    let review = inputs
        .matrix
        .iter()
        .any(|c| c.state == CellState::Partial)
        .then(|| {
            let body = "Flux à instruire : la matrice d'origine le soumettait à une revue \
                        manuelle du respect des motifs.";
            let (texts, texts_json) = b.text(body);
            let local = b.local();
            let key = b.unique(REVIEW_PATTERN);
            b.push(
                "pattern.create",
                json!({ "local": local, "key": key,
                        "labels": labels_of("Revue manuelle (import)"), "texts": texts_json }),
            );
            let key = Key::new(&key).expect("a key");
            parts_patterns.push(Pattern {
                key: key.clone(),
                texts,
            });
            (key, local)
        });

    // The cells; a cell written twice keeps the last state.
    let axis = AxisKey::parse(ENVIRONMENT).expect("a property key");
    let mut cells: BTreeMap<(String, String, String), CellState> = BTreeMap::new();
    for cell in &inputs.matrix {
        let id = (
            cell.source.clone(),
            cell.flow_type.clone(),
            cell.destination.clone(),
        );
        if cells.insert(id, cell.state).is_some() {
            b.notices.push(format!(
                "cellule {} -{}-> {} écrite deux fois : la dernière l'emporte",
                cell.source, cell.flow_type, cell.destination
            ));
        }
    }
    for ((source, flow_name, destination), state) in &cells {
        let (Some(src), Some(dst), Some((flow_type, flow_local))) = (
            environments.get(source),
            environments.get(destination),
            flow_types.get(flow_name),
        ) else {
            continue;
        };
        let mut body = json!({ "source": src, "destination": dst,
                               "flow_type": { "local": flow_local } });
        let mut required = BTreeSet::new();
        let mut texts = Texts::default();
        let effect = match state {
            CellState::Allow => {
                body["state"] = "allow".into();
                Effect::Allow
            }
            CellState::Deny => {
                let (t, json) = b.text(&format!(
                    "Interdit par la matrice d'origine : {source} vers {destination} ({flow_name})."
                ));
                texts = t;
                body["state"] = "deny".into();
                body["texts"] = json;
                Effect::Deny
            }
            CellState::Partial => {
                let (pattern, local) = review.clone().expect("a pattern for partial cells");
                required.insert(pattern);
                body["state"] = "conditional".into();
                body["patterns"] = json!([{ "local": local }]);
                Effect::Allow
            }
        };
        b.push("matrix.set", body);
        let key = Key::new(&b.unique(&format!("matrix-{}", parts_rules.len() + 1))).expect("a key");
        parts_rules.push(Rule {
            key,
            source: Selector::from([(axis.clone(), BTreeSet::from([AxisValue::new(src)]))]),
            destination: Selector::from([(axis.clone(), BTreeSet::from([AxisValue::new(dst)]))]),
            same: BTreeSet::new(),
            flows: FlowSelector {
                flow_types: BTreeSet::from([flow_type.clone()]),
                services: BTreeSet::new(),
            },
            effect,
            required_patterns: required,
            pattern: None,
            texts,
        });
    }
    for (name, (key, _)) in &flow_types {
        if !services.iter().any(|s| &s.flow_type == key) {
            b.notices.push(format!(
                "type de flux {name:?} : aucun service, ses cellules ne s'appliquent à rien"
            ));
        }
    }

    // The rules of inference: reading order → priority, the later one higher.
    let mut inference = Vec::new();
    let mut rank = 0;
    for rule in &inputs.rules {
        rank += 1;
        if let Some(converted) = inference_rule(
            &mut b,
            rule,
            rank * 10,
            None,
            0,
            &environments,
            &inputs.sites,
        ) {
            inference.push(converted);
        }
    }

    let parts = Parts {
        revision: 0,
        default_locale: options.locale.clone(),
        axes: vec![Axis {
            key: axis,
            carrier: Carrier::Tree,
            inference: true,
        }],
        values: Vec::new(),
        flow_types: parts_flow_types,
        services: parts_services,
        rules: parts_rules,
        patterns: parts_patterns,
        conditions: Vec::new(),
        signals: Vec::new(),
    };
    Conversion {
        operations: b.operations,
        notices: b.notices,
        parts,
        inference,
        environments,
        flow_types: flow_types.into_iter().map(|(n, (k, _))| (n, k)).collect(),
        services,
    }
}

/// The criteria of a rule of origin, or why it cannot be converted.
fn criteria(rule: &SourceRule, sites: &[Site]) -> Result<Vec<Value>, String> {
    rule.constraints
        .iter()
        .map(|c| match c {
            Constraint::Name(source) => {
                let expression = anchored(source);
                NameRegex::new(&expression).map_err(|v| v.message.clone())?;
                Ok(json!({ "name": [expression] }))
            }
            Constraint::Within(prefix) => Ok(json!({ "within": [prefix] })),
            Constraint::Equals(prefix) => Ok(json!({ "equals": [prefix] })),
            Constraint::Site(source) => {
                let regex =
                    regex::Regex::new(source).map_err(|e| format!("site {source:?} : {e}"))?;
                let ids: Vec<String> = sites
                    .iter()
                    .filter(|s| regex.is_match(&s.name))
                    .map(|s| s.id.to_string())
                    .collect();
                if ids.is_empty() {
                    return Err(format!("aucun site ne correspond à {source:?}"));
                }
                Ok(json!({ "site": ids }))
            }
            Constraint::Unknown(what) => Err(format!("contrainte inconnue {what:?}")),
        })
        .collect()
}

/// Converts one rule of origin and its exceptions; none when it cannot be.
#[allow(clippy::too_many_arguments)] // the state of the conversion, passed down the tree
fn inference_rule(
    b: &mut Builder<'_>,
    rule: &SourceRule,
    priority: i32,
    parent: Option<u32>,
    depth: usize,
    environments: &BTreeMap<String, String>,
    sites: &[Site],
) -> Option<InferenceRule> {
    let skip = |b: &mut Builder<'_>, why: String| {
        b.notices.push(format!(
            "règle d'inférence {:?} ({}) non convertie : {why}",
            rule.name, rule.file
        ));
        None
    };
    if depth > MAX_DEPTH {
        return skip(
            b,
            format!("exception imbriquée au-delà de {MAX_DEPTH} niveaux"),
        );
    }
    let values: BTreeSet<String> = rule
        .labels
        .iter()
        .filter_map(|l| environments.get(l).cloned())
        .collect();
    if values.is_empty() {
        return skip(b, "aucune étiquette".into());
    }
    let criteria_json = match criteria(rule, sites) {
        Ok(c) => c,
        Err(why) => return skip(b, why),
    };
    let parsed: Result<Vec<_>, _> = criteria_json.iter().map(criterion).collect();
    let parsed = match parsed {
        Ok(p) => p,
        Err(v) => return skip(b, v.message.clone()),
    };
    let Some(base) = slug(&rule.name) else {
        return skip(b, "aucune clé possible".into());
    };
    let key = Key::new(&b.unique(&base)).expect("a key");
    let local = b.local();
    let mut body = json!({ "local": local, "key": key.as_str(),
                           "axis": b.options.axis.to_string(), "priority": priority,
                           "criteria": criteria_json, "values": values });
    if let Some(parent) = parent {
        body["parent"] = json!({ "local": parent });
    }
    b.push("inference_rule.create", body);
    let mut converted = InferenceRule {
        key,
        axis: b.options.axis.clone(),
        priority,
        criteria: parsed,
        values: values.into_iter().map(AxisValue::new).collect(),
        exceptions: Vec::new(),
    };
    let mut order = 0;
    for exception in &rule.exceptions {
        order += 1;
        if let Some(child) = inference_rule(
            b,
            exception,
            order * 10,
            Some(local),
            depth + 1,
            environments,
            sites,
        ) {
            converted.exceptions.push(child);
        }
    }
    Some(converted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_a_key() {
        assert_eq!(
            slug("  Recette Équipe 2 ").as_deref(),
            Some("recette-quipe-2")
        );
        assert_eq!(slug("--"), None);
    }

    #[test]
    fn a_search_becomes_an_anchored_expression_of_the_same_meaning() {
        for (source, matching, other) in [
            ("^prd", "prd-web-01", "web-prd"),
            ("prd", "web-prd-01", "web-01"),
            ("-db$", "prd-db", "prd-db-1"),
            ("^lab$", "lab", "lab1"),
        ] {
            let expression = anchored(source);
            let regex = regex::Regex::new(&expression).unwrap();
            assert!(NameRegex::new(&expression).is_ok(), "{expression}");
            assert!(regex.is_match(matching), "{expression} {matching}");
            assert!(!regex.is_match(other), "{expression} {other}");
        }
    }
}
