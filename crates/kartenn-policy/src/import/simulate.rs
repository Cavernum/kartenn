//! The simulation of a conversion (REQ-010-35, 53): where the converted policy answers
//! otherwise than the tool of origin, read with its own semantics.
//!
//! - The matrix: for each converted service and each pair of environments of the matrix,
//!   the answer of origin (catalogue, then cell: allowed, denied, « partiel » to review, or
//!   no cell) against the verdict of the converted policy (REQ-010-11). A missing cell is
//!   a difference: kartenn denies by default (S8).
//! - The inference: for each node given, the labels of origin — the later read rule that
//!   matches the node or an ancestor wins, then its exceptions, the later read winning —
//!   against what the converted rules expect (REQ-010-43 to 45).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use kartenn_core::addr::Prefix;
use kartenn_core::family::{IPV4, IPV6};
use uuid::Uuid;

use super::ImportError;
use super::convert::{Conversion, Inputs};
use super::source::{CellState, Constraint, NodeRow, SourceRule};
use crate::evaluate::{Endpoint, Outcome, Requested, Verdict, evaluate};
use crate::inference::{Expectation, Node, expected};
use crate::model::{AxisKey, AxisValue};
use crate::module::ENVIRONMENT;
use crate::snapshot::PolicySnapshot;

/// An answer, of origin or converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Answer {
    /// Allowed.
    Allowed,
    /// To review (« partiel », a conditional authorisation without pattern).
    Review,
    /// Denied.
    Denied,
    /// No cell for the pair (origin only).
    Undefined,
}

impl fmt::Display for Answer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Allowed => "autorisé",
            Self::Review => "à instruire",
            Self::Denied => "refusé",
            Self::Undefined => "non défini",
        })
    }
}

/// A pair of environments for a service answered otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixDifference {
    /// Environment of the source, as written.
    pub source: String,
    /// Environment of the destination, as written.
    pub destination: String,
    /// The service, as written.
    pub service: String,
    /// Its flow type, as written.
    pub flow_type: String,
    /// The answer of origin.
    pub origin: Answer,
    /// The answer of kartenn.
    pub kartenn: Answer,
}

impl fmt::Display for MatrixDifference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "matrice   {} -> {} pour {} ({}) : origine {}, kartenn {}",
            self.source, self.destination, self.service, self.flow_type, self.origin, self.kartenn
        )?;
        if self.origin == Answer::Undefined && self.kartenn == Answer::Denied {
            f.write_str(" (cellule absente : refus par défaut)")?;
        }
        Ok(())
    }
}

/// The pairs answered otherwise (see the module documentation).
///
/// # Panics
/// Never: the axis key is a constant.
#[must_use]
pub fn matrix(inputs: &Inputs, conversion: &Conversion) -> Vec<MatrixDifference> {
    let snapshot = PolicySnapshot::new(conversion.parts.clone());
    let axis = AxisKey::parse(ENVIRONMENT).expect("a property key");
    let flow_names: BTreeMap<_, _> = conversion
        .flow_types
        .iter()
        .map(|(name, key)| (key.clone(), name.clone()))
        .collect();
    let cells: BTreeMap<(&str, &str, &str), CellState> = inputs
        .matrix
        .iter()
        .map(|c| {
            (
                (
                    c.source.as_str(),
                    c.flow_type.as_str(),
                    c.destination.as_str(),
                ),
                c.state,
            )
        })
        .collect();
    let forbidden: BTreeSet<&str> = inputs
        .services
        .iter()
        .filter(|s| !s.allowed)
        .map(|s| s.service.as_str())
        .collect();
    let names: BTreeSet<&String> = inputs
        .matrix
        .iter()
        .flat_map(|c| [&c.source, &c.destination])
        .filter(|n| conversion.environments.contains_key(*n))
        .collect();
    let endpoint = |name: &str| Endpoint {
        values: BTreeMap::from([(
            axis.clone(),
            BTreeSet::from([AxisValue::new(conversion.environments[name].as_str())]),
        )]),
        width: 1,
        whole_domain: false,
    };
    let mut out = Vec::new();
    for service in &conversion.services {
        let flow_name = flow_names
            .get(&service.flow_type)
            .cloned()
            .unwrap_or_default();
        for source in &names {
            for destination in &names {
                let origin = if forbidden.contains(service.name.as_str()) {
                    Answer::Denied
                } else {
                    match cells.get(&(source.as_str(), flow_name.as_str(), destination.as_str())) {
                        Some(CellState::Allow) => Answer::Allowed,
                        Some(CellState::Deny) => Answer::Denied,
                        Some(CellState::Partial) => Answer::Review,
                        None => Answer::Undefined,
                    }
                };
                let evaluation = evaluate(
                    &snapshot,
                    &endpoint(source),
                    &endpoint(destination),
                    &service.key,
                    Requested::Service,
                    None,
                );
                let kartenn = match (evaluation.verdict, evaluation.outcome) {
                    (Verdict::Allowed, Outcome::Automatic) => Answer::Allowed,
                    (Verdict::Denied, _) => Answer::Denied,
                    _ => Answer::Review,
                };
                if origin != kartenn {
                    out.push(MatrixDifference {
                        source: (*source).clone(),
                        destination: (*destination).clone(),
                        service: service.name.clone(),
                        flow_type: flow_name.clone(),
                        origin,
                        kartenn,
                    });
                }
            }
        }
    }
    out
}

/// What a side expects of a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    /// Nothing.
    Nothing,
    /// One set of values (keys).
    Values(BTreeSet<String>),
    /// Several candidates (kartenn only, REQ-010-45).
    Candidates(Vec<BTreeSet<String>>),
}

impl fmt::Display for Expect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let set = |s: &BTreeSet<String>| {
            format!("{{{}}}", s.iter().cloned().collect::<Vec<_>>().join(", "))
        };
        match self {
            Self::Nothing => f.write_str("rien"),
            Self::Values(v) => f.write_str(&set(v)),
            Self::Candidates(c) => write!(
                f,
                "candidats {}",
                c.iter().map(set).collect::<Vec<_>>().join(" | ")
            ),
        }
    }
}

/// A node expected otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDifference {
    /// The path of the node, from its root.
    pub path: String,
    /// What the rules of origin give it.
    pub origin: Expect,
    /// What the converted rules expect.
    pub kartenn: Expect,
}

impl fmt::Display for NodeDifference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "inférence {} : origine {}, kartenn {}",
            self.path, self.origin, self.kartenn
        )
    }
}

/// A node with what the rules of origin read of it.
struct Origin<'a> {
    row: &'a NodeRow,
    prefix: Option<Prefix>,
}

fn search(expression: &str, text: Option<&str>) -> bool {
    match (regex::Regex::new(expression), text) {
        (Ok(r), Some(t)) => r.is_match(t),
        _ => false,
    }
}

/// Whether a node satisfies every constraint of a rule of origin; an unknown constraint
/// never matches (the rule was left out of the conversion, with a notice).
fn matches(rule: &SourceRule, node: &Origin<'_>) -> bool {
    rule.constraints.iter().all(|c| match c {
        Constraint::Name(e) => search(e, node.row.name.as_deref()),
        Constraint::Site(e) => search(e, node.row.site.as_deref()),
        Constraint::Within(p) => match (Prefix::parse_any(&[IPV4, IPV6], p), node.prefix) {
            (Ok(outer), Some(inner)) => outer.contains(inner),
            _ => false,
        },
        Constraint::Equals(p) => {
            let p = Prefix::parse_any(&[IPV4, IPV6], p).ok();
            p.is_some() && p == node.prefix
        }
        Constraint::Unknown(_) => false,
    })
}

/// The labels of `rule`, matched at `depth`, through its exceptions: the later read
/// exception matching at or below that node wins, recursively.
fn refine<'a>(rule: &'a SourceRule, depth: usize, chain: &[Origin<'_>]) -> &'a [String] {
    let mut labels: &[String] = &rule.labels;
    for exception in &rule.exceptions {
        if let Some(d) = (depth..chain.len())
            .rev()
            .find(|&i| matches(exception, &chain[i]))
        {
            labels = refine(exception, d, chain);
        }
    }
    labels
}

/// What the tool of origin gives the last node of `chain`: the later read rule that
/// matches it or an ancestor.
fn origin(rules: &[SourceRule], chain: &[Origin<'_>]) -> Option<Vec<String>> {
    let mut out = None;
    for rule in rules {
        if let Some(d) = (0..chain.len()).rev().find(|&i| matches(rule, &chain[i])) {
            out = Some(refine(rule, d, chain).to_vec());
        }
    }
    out
}

/// The nodes expected otherwise (see the module documentation).
///
/// # Errors
/// A parent unknown, a cycle, or a prefix unreadable.
pub fn inference(
    inputs: &Inputs,
    nodes: &[NodeRow],
    conversion: &Conversion,
    axis: &AxisKey,
) -> Result<Vec<NodeDifference>, ImportError> {
    let by_id: BTreeMap<&str, &NodeRow> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let sites: BTreeMap<&str, Uuid> = inputs
        .sites
        .iter()
        .map(|s| (s.name.as_str(), s.id))
        .collect();
    let ids: BTreeMap<&str, Uuid> = nodes
        .iter()
        .map(|n| (n.id.as_str(), Uuid::now_v7()))
        .collect();
    let mut out = Vec::new();
    for node in nodes {
        // The chain, root first.
        let mut chain_rows = vec![node];
        let mut current = node;
        while let Some(parent) = &current.parent {
            let row = by_id.get(parent.as_str()).ok_or_else(|| {
                ImportError::new(format!("nœud {} : parent {parent} inconnu", current.id))
            })?;
            if chain_rows.len() > 64 || chain_rows.iter().any(|r| r.id == row.id) {
                return Err(ImportError::new(format!(
                    "nœud {} : cycle de parents",
                    node.id
                )));
            }
            chain_rows.push(row);
            current = row;
        }
        chain_rows.reverse();
        let mut origins = Vec::new();
        let mut chain = Vec::new();
        for row in &chain_rows {
            let prefix = row
                .prefix
                .as_deref()
                .map(|p| {
                    Prefix::parse_any(&[IPV4, IPV6], p).map_err(|e| {
                        ImportError::new(format!("nœud {} : préfixe {p} : {e}", row.id))
                    })
                })
                .transpose()?;
            origins.push(Origin { row, prefix });
            chain.push(Node {
                id: ids[row.id.as_str()],
                kind: row.kind,
                prefix,
                domain: ids[chain_rows[0].id.as_str()],
                sites: row
                    .site
                    .as_deref()
                    .and_then(|s| sites.get(s))
                    .copied()
                    .into_iter()
                    .collect(),
                entities: Vec::new(),
                name: row.name.clone(),
                properties: BTreeMap::new(),
            });
        }
        let origin = origin(&inputs.rules, &origins).map_or(Expect::Nothing, |labels| {
            Expect::Values(
                labels
                    .iter()
                    .filter_map(|l| conversion.environments.get(l).cloned())
                    .collect(),
            )
        });
        let keys = |s: &BTreeSet<AxisValue>| s.iter().map(|v| v.as_str().to_owned()).collect();
        let kartenn = match expected(&conversion.inference, axis, &chain) {
            Expectation::None => Expect::Nothing,
            Expectation::Single(e) => Expect::Values(keys(&e.values)),
            Expectation::Multiple(c) => {
                Expect::Candidates(c.iter().map(|e| keys(&e.values)).collect())
            }
        };
        if origin != kartenn {
            out.push(NodeDifference {
                path: chain_rows
                    .iter()
                    .map(|r| r.id.as_str())
                    .collect::<Vec<_>>()
                    .join(" / "),
                origin,
                kartenn,
            });
        }
    }
    Ok(out)
}
