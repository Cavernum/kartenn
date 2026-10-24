//! Inference of expected classifications on the addressing tree (REQ-010-40 to 49;
//! design 010 « Inférence »; decisions I1 to I15, I6 softened). A pure function of the
//! rules and of a node with its ancestors: it proposes and checks, it never decides a
//! verdict (D23).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use kartenn_core::addr::{Family, Prefix};
use kartenn_core::rules::Violation;
use regex::Regex;
use uuid::Uuid;

use crate::model::{AxisKey, AxisValue, Key};
use crate::rules::{INFERENCE_CRITERION_INVALID, INFERENCE_DEPTH, INFERENCE_REGEX_INVALID};

/// Deepest nesting of exceptions under a rule (REQ-010-44).
pub const MAX_DEPTH: usize = 4;

/// Longest regular expression of a criterion, in characters (REQ-010-42).
pub const REGEX_MAX_CHARS: usize = 256;

/// Most weaker rules listed with an expected value (REQ-010-49).
pub const DISCARDED_MAX: usize = 3;

/// Kind of a node of the addressing tree (REQ-010-41).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    /// A domain.
    Domain,
    /// A container prefix.
    Container,
    /// A network prefix.
    Network,
    /// An address.
    Address,
}

/// A node as the inference reads it, at the date of the evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Identifier.
    pub id: Uuid,
    /// Kind.
    pub kind: NodeKind,
    /// Its prefix (an address: its host prefix); none for a domain.
    pub prefix: Option<Prefix>,
    /// Its domain.
    pub domain: Uuid,
    /// Resolved site and its ancestors.
    pub sites: Vec<Uuid>,
    /// Resolved entity and its ancestors.
    pub entities: Vec<Uuid>,
    /// Name (slug, label or FQDN, as the storage gives it).
    pub name: Option<String>,
    /// Resolved values of other properties, by key.
    pub properties: BTreeMap<AxisKey, BTreeSet<AxisValue>>,
}

/// A checked regular expression: anchored, bounded, linear-time (REQ-010-42).
#[derive(Debug, Clone)]
pub struct NameRegex {
    source: String,
    regex: Regex,
}

impl NameRegex {
    /// Checks an expression (`policy.inference.regex_invalid`).
    ///
    /// # Errors
    /// A violation when it is not anchored (`^…$`), longer than 256 characters, or
    /// outside the syntax of the linear-time engine (no look-around, no back-reference).
    pub fn new(source: &str) -> Result<Self, Violation> {
        let invalid = |why: &str| {
            Violation::new(
                &INFERENCE_REGEX_INVALID,
                format!("expression {source:?} : {why}"),
            )
        };
        if source.chars().count() > REGEX_MAX_CHARS {
            return Err(invalid("plus de 256 caractères"));
        }
        if !(source.starts_with('^') && source.ends_with('$') && !source.ends_with("\\$")) {
            return Err(invalid("non ancrée (^…$)"));
        }
        let regex = Regex::new(source).map_err(|e| invalid(&e.to_string()))?;
        Ok(Self {
            source: source.to_owned(),
            regex,
        })
    }

    /// The expression as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.source
    }
}

impl PartialEq for NameRegex {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Eq for NameRegex {}

/// Test on another property (REQ-010-42).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyTest {
    /// The resolved set equals these values.
    Equals(BTreeSet<AxisValue>),
    /// The resolved set holds one of these values.
    In(BTreeSet<AxisValue>),
    /// The property has a value.
    Present,
}

/// A criterion of an inference rule, from a closed list extended by code; each takes a
/// list of values (OR), criteria of a rule combine with AND (REQ-010-42, I3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Criterion {
    /// The node's prefix is inside one of these prefixes (or equal).
    Within(Vec<Prefix>),
    /// The node's prefix is one of these.
    Equals(Vec<Prefix>),
    /// The node's prefix is of one of these families.
    Family(Vec<Family>),
    /// The node is in one of these domains.
    Domain(Vec<Uuid>),
    /// The node is of one of these kinds.
    Kind(Vec<NodeKind>),
    /// The node's resolved site is one of these or below one of them.
    Site(Vec<Uuid>),
    /// The node's resolved entity is one of these or below one of them.
    Entity(Vec<Uuid>),
    /// A test on another property.
    Property(AxisKey, PropertyTest),
    /// The name matches one of these expressions.
    Name(Vec<NameRegex>),
    /// The prefix length is one of these.
    PrefixLength(Vec<u8>),
}

fn list<T: fmt::Display>(items: impl IntoIterator<Item = T>) -> String {
    items
        .into_iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

impl Criterion {
    fn is_empty(&self) -> bool {
        match self {
            Self::Within(v) | Self::Equals(v) => v.is_empty(),
            Self::Family(v) => v.is_empty(),
            Self::Domain(v) | Self::Site(v) | Self::Entity(v) => v.is_empty(),
            Self::Kind(v) => v.is_empty(),
            Self::Property(_, PropertyTest::Equals(v) | PropertyTest::In(v)) => v.is_empty(),
            Self::Property(_, PropertyTest::Present) => false,
            Self::Name(v) => v.is_empty(),
            Self::PrefixLength(v) => v.is_empty(),
        }
    }

    /// The observed value when `node` satisfies the criterion (REQ-010-49).
    #[must_use]
    pub fn observe(&self, node: &Node) -> Option<String> {
        let prefix = node.prefix;
        match self {
            Self::Within(ps) => {
                let p = prefix?;
                let outer = ps.iter().find(|outer| outer.contains(p))?;
                Some(format!("{p} dans {outer}"))
            }
            Self::Equals(ps) => {
                let p = prefix?;
                ps.contains(&p).then(|| format!("{p}"))
            }
            Self::Family(fs) => {
                let f = prefix?.family();
                fs.contains(&f).then(|| format!("famille {}", f.id()))
            }
            Self::Domain(ds) => ds
                .contains(&node.domain)
                .then(|| format!("domaine {}", node.domain)),
            Self::Kind(ks) => ks.contains(&node.kind).then(|| format!("{:?}", node.kind)),
            Self::Site(ss) => node
                .sites
                .iter()
                .find(|s| ss.contains(s))
                .map(|s| format!("site {s}")),
            Self::Entity(es) => node
                .entities
                .iter()
                .find(|e| es.contains(e))
                .map(|e| format!("entité {e}")),
            Self::Property(key, test) => {
                let values = node.properties.get(key).filter(|v| !v.is_empty());
                let ok = match (test, values) {
                    (PropertyTest::Present, Some(_)) => true,
                    (PropertyTest::Equals(want), Some(have)) => want == have,
                    (PropertyTest::In(want), Some(have)) => !want.is_disjoint(have),
                    (_, None) => false,
                };
                ok.then(|| format!("{key} = {{{}}}", list(values.into_iter().flatten())))
            }
            Self::Name(rs) => {
                let name = node.name.as_deref()?;
                rs.iter()
                    .find(|r| r.regex.is_match(name))
                    .map(|r| format!("nom {name:?} ~ {}", r.as_str()))
            }
            Self::PrefixLength(ls) => {
                let len = prefix?.prefix_len();
                ls.contains(&len).then(|| format!("longueur /{len}"))
            }
        }
    }
}

/// An inference rule: criteria, expected values, explicit priority and nested exceptions
/// that replace its expected set on the nodes they match (REQ-010-42 to 45).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceRule {
    /// Key.
    pub key: Key,
    /// The axis it produces expected values for.
    pub axis: AxisKey,
    /// Explicit priority: higher wins (I6).
    pub priority: i32,
    /// Criteria (AND); none = every node.
    pub criteria: Vec<Criterion>,
    /// Expected values, non-empty.
    pub values: BTreeSet<AxisValue>,
    /// Exceptions, evaluated only below where this rule matched.
    pub exceptions: Vec<InferenceRule>,
}

impl InferenceRule {
    /// Checks the rule and its exceptions (REQ-010-42, 44).
    ///
    /// # Errors
    /// Violations of `policy.inference.depth` and `policy.inference.criterion_invalid`.
    pub fn check(&self) -> Result<(), Vec<Violation>> {
        let mut errors = Vec::new();
        self.check_at(1, &mut errors);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn check_at(&self, depth: usize, errors: &mut Vec<Violation>) {
        if depth > MAX_DEPTH + 1 {
            errors.push(Violation::new(
                &INFERENCE_DEPTH,
                format!(
                    "l'exception {} est imbriquée sur {} niveaux, au plus {MAX_DEPTH}",
                    self.key,
                    depth - 1
                ),
            ));
        }
        if self.values.is_empty() {
            errors.push(Violation::new(
                &INFERENCE_CRITERION_INVALID,
                format!("la règle {} n'attend aucune valeur", self.key),
            ));
        }
        if self.criteria.iter().any(Criterion::is_empty) {
            errors.push(Violation::new(
                &INFERENCE_CRITERION_INVALID,
                format!("la règle {} a un critère sans valeur", self.key),
            ));
        }
        for exception in &self.exceptions {
            exception.check_at(depth + 1, errors);
        }
    }

    /// The observations of every criterion when `node` satisfies them all.
    fn observe(&self, node: &Node) -> Option<Vec<String>> {
        self.criteria.iter().map(|c| c.observe(node)).collect()
    }

    /// Whether the rule matches a node of `chain`, the node or an ancestor: the node is
    /// in its scope (REQ-010-43, transmitted to the sub-branch).
    #[must_use]
    pub fn reaches(&self, chain: &[Node]) -> bool {
        self.deepest(chain, 0).is_some()
    }

    /// Whether the rule or one of its exceptions is `key`.
    #[must_use]
    pub fn holds(&self, key: &Key) -> bool {
        &self.key == key || self.exceptions.iter().any(|e| e.holds(key))
    }

    /// The deepest node of `chain[from..]` the rule matches, with its observations.
    fn deepest(&self, chain: &[Node], from: usize) -> Option<(usize, Vec<String>)> {
        (from..chain.len())
            .rev()
            .find_map(|i| self.observe(&chain[i]).map(|o| (i, o)))
    }
}

/// An expected set and why (REQ-010-49).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    /// The expected values.
    pub values: BTreeSet<AxisValue>,
    /// The top rule.
    pub rule: Key,
    /// Its priority.
    pub priority: i32,
    /// The chain of exceptions applied below it, outermost first.
    pub exceptions: Vec<Key>,
    /// The node the top rule matched (transmitted to its sub-branch, REQ-010-43).
    pub matched: Uuid,
    /// Depth of that node in the chain (0 = root).
    pub depth: usize,
    /// Observations of the criteria of the rule (or exception) that gave the values.
    pub observations: Vec<String>,
    /// Weaker matching rules set aside, three at most.
    pub discarded: Vec<Key>,
}

/// What the inference expects for a node on an axis (REQ-010-45, 47).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expectation {
    /// No rule matches.
    None,
    /// One expected set.
    Single(Expected),
    /// Rules of the same priority at the same depth expect different sets: every
    /// candidate, no default value, no mismatch (I6 softened).
    Multiple(Vec<Expected>),
}

impl Expectation {
    /// The rules in conflict, for the anomaly `policy.inference.conflict` on the rule set.
    #[must_use]
    pub fn conflict(&self) -> Vec<Key> {
        match self {
            Self::Multiple(candidates) => candidates.iter().map(|c| c.rule.clone()).collect(),
            _ => Vec::new(),
        }
    }

    /// The single expected set, if any (for the default value, REQ-010-52).
    #[must_use]
    pub fn single(&self) -> Option<&BTreeSet<AxisValue>> {
        match self {
            Self::Single(e) => Some(&e.values),
            _ => None,
        }
    }
}

/// Refines what `rule`, matched at `depth`, expects, through its exceptions; a tie
/// between exceptions expecting different sets gives several outcomes.
fn refine(
    rule: &InferenceRule,
    depth: usize,
    observations: Vec<String>,
    chain: &[Node],
) -> Vec<(BTreeSet<AxisValue>, Vec<Key>, Vec<String>)> {
    let matched: Vec<(&InferenceRule, usize, Vec<String>)> = rule
        .exceptions
        .iter()
        .filter_map(|e| e.deepest(chain, depth).map(|(d, o)| (e, d, o)))
        .collect();
    let Some(best) = matched.iter().map(|(e, d, _)| (e.priority, *d)).max() else {
        return vec![(rule.values.clone(), Vec::new(), observations)];
    };
    let mut out: Vec<(BTreeSet<AxisValue>, Vec<Key>, Vec<String>)> = Vec::new();
    for (exception, d, obs) in matched {
        if (exception.priority, d) != best {
            continue;
        }
        for (values, mut keys, inner) in refine(exception, d, obs, chain) {
            keys.insert(0, exception.key.clone());
            if !out.iter().any(|(v, ..)| v == &values) {
                out.push((values, keys, inner));
            }
        }
    }
    out
}

/// What the rules expect for the last node of `chain` (its ancestors first) on `axis`
/// (REQ-010-43 to 45): the highest explicit priority, then the deepest node matched;
/// a remaining tie between different sets gives every candidate.
#[must_use]
pub fn expected(rules: &[InferenceRule], axis: &AxisKey, chain: &[Node]) -> Expectation {
    among(rules.iter().filter(|r| &r.axis == axis), chain)
}

/// [`expected`] among `rules`, already those of the axis.
fn among<'r>(rules: impl Iterator<Item = &'r InferenceRule>, chain: &[Node]) -> Expectation {
    let mut matched: Vec<(&InferenceRule, usize, Vec<String>)> = rules
        .filter_map(|r| r.deepest(chain, 0).map(|(d, o)| (r, d, o)))
        .collect();
    // Strongest first; the key orders equals for a stable explanation.
    matched.sort_by(|(x, dx, _), (y, dy, _)| {
        (y.priority, *dy)
            .cmp(&(x.priority, *dx))
            .then(x.key.cmp(&y.key))
    });
    let Some(&(_, top_depth, _)) = matched.first() else {
        return Expectation::None;
    };
    let top = (matched[0].0.priority, top_depth);
    let weaker: Vec<Key> = matched
        .iter()
        .filter(|(r, d, _)| (r.priority, *d) != top)
        .take(DISCARDED_MAX)
        .map(|(r, ..)| r.key.clone())
        .collect();
    let mut candidates: Vec<Expected> = Vec::new();
    for (rule, depth, observations) in matched.iter().filter(|(r, d, _)| (r.priority, *d) == top) {
        for (values, exceptions, observations) in refine(rule, *depth, observations.clone(), chain)
        {
            if candidates.iter().any(|c| c.values == values) {
                continue;
            }
            candidates.push(Expected {
                values,
                rule: rule.key.clone(),
                priority: rule.priority,
                exceptions,
                matched: chain[*depth].id,
                depth: *depth,
                observations,
                discarded: weaker.clone(),
            });
        }
    }
    if candidates.len() == 1 {
        Expectation::Single(candidates.remove(0))
    } else {
        Expectation::Multiple(candidates)
    }
}

/// The rules of an axis indexed by the prefixes of their criteria `within` or `equals`,
/// for [`expected`] over many chains (REQ-010-54, S36). Such a rule matches a node only
/// when one of its prefixes is the supernet of the node's prefix at its own length: a
/// chain tests the rules found under the supernets of its nodes, and the rules without
/// such a criterion, instead of every rule of the axis. Same answer as [`expected`].
#[derive(Debug)]
pub struct IndexedRules<'a> {
    /// The rules of the axis, in their order.
    rules: Vec<&'a InferenceRule>,
    /// The rules without a criterion on the prefix: tested on every chain.
    free: Vec<usize>,
    /// The rules under each prefix their first criterion on the prefix names.
    by_prefix: HashMap<Prefix, Vec<usize>>,
    /// The family and length of every prefix of `by_prefix`.
    lengths: Vec<(Family, u8)>,
}

impl<'a> IndexedRules<'a> {
    /// Indexes the rules of `axis` among `rules`.
    #[must_use]
    pub fn new(rules: &'a [InferenceRule], axis: &AxisKey) -> Self {
        let rules: Vec<&InferenceRule> = rules.iter().filter(|r| &r.axis == axis).collect();
        let mut free = Vec::new();
        let mut by_prefix: HashMap<Prefix, Vec<usize>> = HashMap::new();
        let mut lengths: Vec<(Family, u8)> = Vec::new();
        for (n, rule) in rules.iter().enumerate() {
            let bound = rule.criteria.iter().find_map(|c| match c {
                Criterion::Within(ps) | Criterion::Equals(ps) => Some(ps),
                _ => None,
            });
            let Some(prefixes) = bound else {
                free.push(n);
                continue;
            };
            for p in prefixes {
                by_prefix.entry(*p).or_default().push(n);
                let length = (p.family(), p.prefix_len());
                if !lengths.contains(&length) {
                    lengths.push(length);
                }
            }
        }
        Self {
            rules,
            free,
            by_prefix,
            lengths,
        }
    }

    /// What the rules expect for the last node of `chain`, as [`expected`].
    #[must_use]
    pub fn expected(&self, chain: &[Node]) -> Expectation {
        let mut picked = self.free.clone();
        for prefix in chain.iter().filter_map(|node| node.prefix) {
            for (family, length) in &self.lengths {
                if *family != prefix.family() {
                    continue;
                }
                if let Some(rules) = prefix
                    .supernet(*length)
                    .and_then(|s| self.by_prefix.get(&s))
                {
                    picked.extend(rules);
                }
            }
        }
        picked.sort_unstable();
        picked.dedup();
        among(picked.into_iter().map(|n| self.rules[n]), chain)
    }
}

/// Whether a declared set differs from what the inference expects (REQ-010-47): only a
/// single expectation can differ; none or several candidates never do.
#[must_use]
pub fn mismatch(declared: &BTreeSet<AxisValue>, expectation: &Expectation) -> bool {
    expectation.single().is_some_and(|e| e != declared)
}

#[cfg(test)]
#[allow(clippy::many_single_char_names)]
mod tests {
    use super::*;
    use crate::model::tests::{axis, key};
    use kartenn_core::family::IPV4;
    use proptest::prelude::*;

    const ENV: &str = "policy.environment";

    fn prefix(text: &str) -> Prefix {
        Prefix::parse(IPV4, text).unwrap()
    }

    fn values(vs: &[&str]) -> BTreeSet<AxisValue> {
        vs.iter().map(|v| AxisValue::from(*v)).collect()
    }

    fn node(kind: NodeKind, p: Option<&str>, name: Option<&str>, site: Uuid) -> Node {
        Node {
            id: Uuid::now_v7(),
            kind,
            prefix: p.map(prefix),
            domain: Uuid::nil(),
            sites: vec![site],
            entities: Vec::new(),
            name: name.map(str::to_owned),
            properties: BTreeMap::new(),
        }
    }

    fn rule(name: &str, priority: i32, criteria: Vec<Criterion>, vs: &[&str]) -> InferenceRule {
        InferenceRule {
            key: key(name),
            axis: axis(ENV),
            priority,
            criteria,
            values: values(vs),
            exceptions: Vec::new(),
        }
    }

    #[test]
    fn acceptance_of_req_010_42() {
        let (a, b, c) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let prod = rule(
            "prod-by-name",
            1,
            vec![
                Criterion::Name(vec![NameRegex::new("^prd-.*$").unwrap()]),
                Criterion::Site(vec![a, b]),
            ],
            &["prod"],
        );
        let at = |name: &str, site| {
            vec![node(
                NodeKind::Network,
                Some("10.1.0.0/24"),
                Some(name),
                site,
            )]
        };
        assert!(
            expected(std::slice::from_ref(&prod), &axis(ENV), &at("prd-web", b))
                .single()
                .is_some()
        );
        assert_eq!(
            expected(std::slice::from_ref(&prod), &axis(ENV), &at("prd-web", c)),
            Expectation::None
        );
        assert_eq!(
            expected(std::slice::from_ref(&prod), &axis(ENV), &at("rec-web", a)),
            Expectation::None
        );

        let refused = |s: &str| NameRegex::new(s).unwrap_err().rule.id;
        assert_eq!(refused("prd-.*"), "policy.inference.regex_invalid");
        assert_eq!(refused("^prd"), "policy.inference.regex_invalid");
        assert_eq!(
            refused(&format!("^{}$", "a".repeat(260))),
            "policy.inference.regex_invalid"
        );
        assert_eq!(refused("^(?=prd).*$"), "policy.inference.regex_invalid");
        assert_eq!(refused("^(a)\\1$"), "policy.inference.regex_invalid");
    }

    #[test]
    fn a_match_is_transmitted_to_the_sub_branch() {
        let site = Uuid::now_v7();
        let r = rule(
            "prod-container",
            1,
            vec![Criterion::Name(vec![NameRegex::new("^prd$").unwrap()])],
            &["prod"],
        );
        let chain = vec![
            node(NodeKind::Container, Some("10.0.0.0/8"), Some("prd"), site),
            node(NodeKind::Network, Some("10.1.0.0/24"), Some("lan"), site),
            node(NodeKind::Address, Some("10.1.0.5/32"), None, site),
        ];
        let Expectation::Single(e) = expected(&[r], &axis(ENV), &chain) else {
            panic!()
        };
        assert_eq!(
            (e.values, e.matched, e.depth),
            (values(&["prod"]), chain[0].id, 0)
        );
        assert_eq!(e.observations, ["nom \"prd\" ~ ^prd$"]);
    }

    #[test]
    fn acceptance_of_req_010_44() {
        let site = Uuid::now_v7();
        let mut prod = rule(
            "prod",
            1,
            vec![Criterion::Name(vec![NameRegex::new("^prd-.*$").unwrap()])],
            &["prod"],
        );
        prod.exceptions.push(rule(
            "x-is-rec",
            0,
            vec![Criterion::Within(vec![prefix("10.9.0.0/16")])],
            &["rec"],
        ));
        let chain = |net: &str| {
            vec![
                node(NodeKind::Network, Some(net), Some("prd-lan"), site),
                node(
                    NodeKind::Address,
                    Some(&format!("{}/32", net.split('/').next().unwrap())),
                    None,
                    site,
                ),
            ]
        };
        let in_x = expected(
            std::slice::from_ref(&prod),
            &axis(ENV),
            &chain("10.9.1.0/24"),
        );
        let Expectation::Single(e) = in_x else {
            panic!()
        };
        assert_eq!(
            (e.values, e.exceptions),
            (values(&["rec"]), vec![key("x-is-rec")])
        );
        assert_eq!(
            expected(
                std::slice::from_ref(&prod),
                &axis(ENV),
                &chain("10.1.1.0/24")
            )
            .single(),
            Some(&values(&["prod"]))
        );
        // An exception only applies where its parent matched.
        let alone = vec![node(
            NodeKind::Network,
            Some("10.9.1.0/24"),
            Some("lan"),
            site,
        )];
        assert_eq!(expected(&[prod], &axis(ENV), &alone), Expectation::None);

        // A fifth nesting is refused.
        let mut deepest = rule("e5", 0, vec![], &["x"]);
        for i in (0..5).rev() {
            let mut parent = rule(&format!("e{i}"), 0, vec![], &["x"]);
            parent.exceptions.push(deepest);
            deepest = parent;
        }
        let errors = deepest.check().unwrap_err();
        assert!(errors.iter().any(|v| v.rule.id == "policy.inference.depth"));
        let mut four = rule("f0", 0, vec![], &["x"]);
        let mut cursor = &mut four;
        for i in 1..=4 {
            cursor
                .exceptions
                .push(rule(&format!("f{i}"), 0, vec![], &["x"]));
            cursor = &mut cursor.exceptions[0];
        }
        assert!(four.check().is_ok());
    }

    #[test]
    fn acceptance_of_req_010_45() {
        let site = Uuid::now_v7();
        let container = Criterion::Kind(vec![NodeKind::Container]);
        let p = rule("as-prod", 10, vec![container.clone()], &["prod"]);
        let r = rule("as-rec", 10, vec![container], &["rec"]);
        let chain = vec![
            node(NodeKind::Container, Some("10.0.0.0/8"), None, site),
            node(NodeKind::Address, Some("10.0.0.1/32"), None, site),
        ];
        let e = expected(&[p, r], &axis(ENV), &chain);
        assert_eq!(e.conflict(), [key("as-prod"), key("as-rec")]);
        assert_eq!(e.single(), None, "no default value");
        assert!(
            !mismatch(&values(&["dev"]), &e),
            "no mismatch on several candidates"
        );
        assert!(!mismatch(&values(&["dev"]), &Expectation::None));
    }

    #[test]
    fn criteria_reject_empty_lists_and_rules_without_values() {
        let empty = rule("e", 0, vec![Criterion::Domain(vec![])], &[]);
        let ids: Vec<_> = empty
            .check()
            .unwrap_err()
            .iter()
            .map(|v| v.rule.id)
            .collect();
        assert_eq!(ids, ["policy.inference.criterion_invalid"; 2]);
    }

    #[test]
    fn every_criterion_observes_what_it_saw() {
        let site = Uuid::now_v7();
        let mut n = node(NodeKind::Network, Some("10.1.0.0/24"), Some("lan"), site);
        n.properties
            .insert(axis("policy.exposure"), values(&["internet"]));
        let seen = |c: Criterion| c.observe(&n);
        assert!(seen(Criterion::Within(vec![prefix("10.0.0.0/8")])).is_some());
        assert!(seen(Criterion::Equals(vec![prefix("10.1.0.0/24")])).is_some());
        assert!(seen(Criterion::Family(vec![IPV4])).is_some());
        assert!(seen(Criterion::Domain(vec![Uuid::nil()])).is_some());
        assert!(seen(Criterion::Site(vec![site])).is_some());
        assert!(seen(Criterion::Entity(vec![site])).is_none());
        assert!(seen(Criterion::PrefixLength(vec![24])).is_some());
        assert!(
            seen(Criterion::Property(
                axis("policy.exposure"),
                PropertyTest::In(values(&["internet"]))
            ))
            .is_some()
        );
        assert!(
            seen(Criterion::Property(
                axis("policy.exposure"),
                PropertyTest::Equals(values(&["internet", "x"]))
            ))
            .is_none()
        );
        assert!(
            seen(Criterion::Property(
                axis("policy.sensitivity"),
                PropertyTest::Present
            ))
            .is_none()
        );
    }

    // Oracle of the tie-break (REQ-010-45) on random chains and rules without exceptions:
    // among the rules that match some node of the chain, keep the highest priority, then
    // the deepest matched node; one set left = single, several = every candidate.
    proptest! {
        #[test]
        fn departage_matches_the_oracle(
            depth in 1usize..6,
            raw in prop::collection::vec((0i32..3, 0usize..6, 0usize..3), 0..6),
        ) {
            let site = Uuid::now_v7();
            let chain: Vec<Node> = (0..depth)
                .map(|i| node(NodeKind::Network, Some("10.0.0.0/8"), Some(&format!("n{i}")), site))
                .collect();
            let rules: Vec<InferenceRule> = raw
                .iter()
                .enumerate()
                .map(|(i, (priority, at, v))| {
                    let name = NameRegex::new(&format!("^n{at}$")).unwrap();
                    rule(&format!("r{i}"), *priority, vec![Criterion::Name(vec![name])], &[["a", "b", "c"][*v]])
                })
                .collect();
            let matching: Vec<(i32, usize, &str)> = raw
                .iter()
                .filter(|(_, at, _)| *at < depth)
                .map(|(p, at, v)| (*p, *at, ["a", "b", "c"][*v]))
                .collect();
            let got = expected(&rules, &axis(ENV), &chain);
            match matching.iter().map(|(p, d, _)| (*p, *d)).max() {
                None => prop_assert_eq!(got, Expectation::None),
                Some(best) => {
                    let sets: BTreeSet<&str> =
                        matching.iter().filter(|(p, d, _)| (*p, *d) == best).map(|(.., v)| *v).collect();
                    match got {
                        Expectation::Single(e) => {
                            prop_assert_eq!(sets.len(), 1);
                            prop_assert_eq!(e.values, values(&[sets.first().unwrap()]));
                        }
                        Expectation::Multiple(cs) => {
                            let got: BTreeSet<_> = cs.iter().map(|c| c.values.first().unwrap().as_str().to_owned()).collect();
                            prop_assert_eq!(got, sets.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>());
                        }
                        Expectation::None => prop_assert!(false, "a rule matched"),
                    }
                }
            }
        }

        // The index of the rules by their prefixes answers as the scan of every rule
        // (S36, bench of the preview): rules within, equal to, or without a prefix, some
        // with an exception, on a chain of nested prefixes.
        #[test]
        fn the_indexed_rules_answer_as_every_rule(
            depth in 1usize..5,
            raw in prop::collection::vec(
                (0i32..3, 0u8..4, 8u8..33, 0usize..3, any::<u32>(), any::<bool>()), 0..12),
            leaf in any::<u32>(),
        ) {
            let site = Uuid::now_v7();
            let host = |len: u8, bits: u32| {
                let mask = if len == 0 { 0 } else { u32::MAX << (32 - u32::from(len)) };
                prefix(&format!("{}/{len}", std::net::Ipv4Addr::from(bits & mask)))
            };
            let mut chain = vec![node(NodeKind::Domain, None, None, site)];
            chain.extend((0..depth).map(|i| {
                let len = [8u8, 16, 24, 32][i.min(3)];
                let mut n = node(NodeKind::Network, None, Some(&format!("n{i}")), site);
                n.prefix = Some(host(len, leaf));
                n
            }));
            let rules: Vec<InferenceRule> = raw
                .iter()
                .enumerate()
                .map(|(i, (priority, how, len, v, bits, exception))| {
                    // Near the leaf half of the time, for rules that match.
                    let bits = if i % 2 == 0 { leaf } else { *bits };
                    let p = host(*len, bits);
                    let criteria = match how {
                        0 => vec![Criterion::Within(vec![p])],
                        1 => vec![Criterion::Equals(vec![p, host(8, leaf)])],
                        2 => vec![Criterion::Kind(vec![NodeKind::Network]), Criterion::Within(vec![p])],
                        _ => vec![Criterion::Name(vec![NameRegex::new(&format!("^n{}$", len % 4)).unwrap()])],
                    };
                    let mut r = rule(&format!("r{i}"), *priority, criteria, &[["a", "b", "c"][*v]]);
                    if *exception {
                        r.exceptions.push(rule(
                            &format!("x{i}"), 0, vec![Criterion::Within(vec![host(24, leaf)])], &["z"]));
                    }
                    r
                })
                .collect();
            prop_assert_eq!(
                IndexedRules::new(&rules, &axis(ENV)).expected(&chain),
                expected(&rules, &axis(ENV), &chain)
            );
        }
    }
}
