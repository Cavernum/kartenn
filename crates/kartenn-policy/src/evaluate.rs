//! Verdict, outcome and score of a pair of endpoints for a service (REQ-010-10 to 15, 31,
//! 32, 36, 37; design 010 « Évaluation »). A pure function of the snapshot.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    AxisKey, AxisValue, ConditionKind, DossierField, Effect, Key, Rule, SignalKind,
};
use crate::snapshot::PolicySnapshot;

/// An endpoint as the evaluation sees it: per axis, its resolved set of values (REQ-010-12,
/// 31), and what conditions and signals read of its extent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Endpoint {
    /// Resolved values per axis; an absent or empty set is « not classified ».
    pub values: BTreeMap<AxisKey, BTreeSet<AxisValue>>,
    /// Number of addresses the requested endpoint stands for (an address: 1).
    pub width: u128,
    /// Whether the requested endpoint is a whole domain.
    pub whole_domain: bool,
}

impl Endpoint {
    fn on(&self, axis: &AxisKey) -> Option<&BTreeSet<AxisValue>> {
        self.values.get(axis).filter(|s| !s.is_empty())
    }
}

/// How the request named the service (REQ-010-14): read by the condition « named
/// service rather than raw port ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requested {
    /// By the service.
    Service,
    /// By a transport and a port.
    Port,
    /// By a flow type.
    FlowType,
}

/// The file joined to a request (REQ-010-38).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dossier {
    /// Fields present.
    pub fields: BTreeSet<DossierField>,
    /// Values given per axis (nature of the data on the sensitivity axis…).
    pub values: BTreeMap<AxisKey, BTreeSet<AxisValue>>,
}

/// A side of a flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// The source.
    Source,
    /// The destination.
    Destination,
}

/// The verdict of a pair for a service (REQ-010-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// Allowed.
    Allowed,
    /// Allowed under reserve: a conditional authorisation without a matching pattern
    /// (REQ-010-32).
    AllowedPending,
    /// Denied.
    Denied,
}

/// What decided the verdict (REQ-010-12).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Decisive {
    /// An endpoint has no value on an axis cited by a candidate rule.
    Unclassified {
        /// The side.
        side: Side,
        /// The axis.
        axis: AxisKey,
    },
    /// A rule: a deny, or an unconditional allow.
    Rule(Key),
    /// A conditional allow satisfied by a pattern.
    Pattern {
        /// The pattern that matched.
        pattern: Key,
        /// The conditional rule that required it.
        rule: Key,
    },
    /// A conditional allow without a matching pattern.
    Pending {
        /// The most specific such rule.
        rule: Key,
        /// Patterns expected by the applicable conditional rules.
        expected: Vec<Key>,
    },
    /// No allow reaches the pair (REQ-010-10).
    NoRule,
    /// No service covers the requested port (REQ-010-14).
    UnknownService,
}

/// The values of a combination that made a rule apply, per side and axis.
pub type Combination = BTreeMap<(Side, AxisKey), AxisValue>;

/// Why an applicable rule did not decide (REQ-010-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discard {
    /// A deny applies: authorisations lose (D28).
    DenyWins,
    /// A more specific rule of the same rank decided.
    LessSpecific,
    /// An unconditional authorisation decided before conditional ones.
    UnconditionalFirst,
}

/// An applicable rule set aside, with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discarded {
    /// The rule.
    pub rule: Key,
    /// Why.
    pub why: Discard,
}

/// Outcome of the triage (REQ-010-36), ordered from best to worst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outcome {
    /// Passes automatically.
    Automatic,
    /// To be reviewed.
    Review,
    /// Not compliant.
    Noncompliant,
}

/// State of a condition for one evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionState {
    /// Met.
    Met,
    /// Not met: the outcome is `Review`.
    Unmet,
    /// Not evaluated, for lack of a file or of ranks: the outcome is `Review`.
    NotEvaluated,
}

/// An applicable condition and its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionResult {
    /// The condition.
    pub condition: Key,
    /// Its state.
    pub state: ConditionState,
}

/// Most rules set aside listed with a verdict (REQ-010-12).
pub const DISCARDED_MAX: usize = 3;

/// Evaluation of a pair for a service (REQ-010-11, 12, 36, 37).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    /// The service.
    pub service: Option<Key>,
    /// The verdict.
    pub verdict: Verdict,
    /// What decided it.
    pub decisive: Decisive,
    /// Values that made the decisive rule apply.
    pub combination: Combination,
    /// Applicable rules set aside, three at most.
    pub discarded: Vec<Discarded>,
    /// Applicable conditions (when allowed) and their state.
    pub conditions: Vec<ConditionResult>,
    /// The outcome.
    pub outcome: Outcome,
    /// Signals raised (warnings).
    pub signals: Vec<Key>,
    /// Sum of their weights.
    pub score: i64,
}

/// The evaluation of a port no service covers: denied, to be reviewed (REQ-010-14).
#[must_use]
pub fn unknown_service() -> Evaluation {
    Evaluation {
        service: None,
        verdict: Verdict::Denied,
        decisive: Decisive::UnknownService,
        combination: Combination::new(),
        discarded: Vec::new(),
        conditions: Vec::new(),
        outcome: Outcome::Review,
        signals: Vec::new(),
        score: 0,
    }
}

/// Whether `rule` applies to (`a`, `b`), and the combination that made it apply: one
/// value per term, the first of the intersection in key order; on a « same value » axis,
/// one value shared by both sides and admitted by both terms (REQ-010-8, 11).
#[must_use]
pub fn applies(rule: &Rule, a: &Endpoint, b: &Endpoint) -> Option<Combination> {
    let mut combination = Combination::new();
    for (side, selector, endpoint) in [
        (Side::Source, &rule.source, a),
        (Side::Destination, &rule.destination, b),
    ] {
        for (axis, admitted) in selector {
            let value = endpoint.on(axis)?.intersection(admitted).next()?;
            combination.insert((side, axis.clone()), value.clone());
        }
    }
    for axis in &rule.same {
        let shared = a.on(axis)?.intersection(b.on(axis)?).find(|v| {
            rule.source.get(axis).is_none_or(|s| s.contains(*v))
                && rule.destination.get(axis).is_none_or(|s| s.contains(*v))
        })?;
        combination.insert((Side::Source, axis.clone()), shared.clone());
        combination.insert((Side::Destination, axis.clone()), shared.clone());
    }
    Some(combination)
}

/// Specificity order: more criteria first, then fewer admitted values, then key (S23).
fn more_specific(x: &Rule, y: &Rule) -> Ordering {
    y.criteria()
        .cmp(&x.criteria())
        .then(x.admitted().cmp(&y.admitted()))
        .then(x.key.cmp(&y.key))
}

/// The verdict of a pair of endpoints for a service, without outcome nor score: the
/// « verdict de principe » of the policy differential (REQ-010-18).
#[must_use]
#[allow(clippy::too_many_lines)] // the six steps of REQ-010-11, read in order
pub fn verdict(
    snapshot: &PolicySnapshot,
    a: &Endpoint,
    b: &Endpoint,
    service: &Key,
) -> (Verdict, Decisive, Combination, Vec<Discarded>) {
    if snapshot.service(service).is_none() {
        return (
            Verdict::Denied,
            Decisive::UnknownService,
            Combination::new(),
            Vec::new(),
        );
    }
    let candidates: Vec<&Rule> = snapshot.candidates(service).collect();
    // 1. Not classified on an axis cited by a candidate rule.
    for axis in snapshot.cited(service) {
        for (side, endpoint) in [(Side::Source, a), (Side::Destination, b)] {
            if endpoint.on(axis).is_none() {
                let decisive = Decisive::Unclassified {
                    side,
                    axis: axis.clone(),
                };
                return (Verdict::Denied, decisive, Combination::new(), Vec::new());
            }
        }
    }
    let mut applicable: Vec<(&Rule, Combination)> = candidates
        .iter()
        .filter_map(|r| applies(r, a, b).map(|c| (*r, c)))
        .collect();
    applicable.sort_by(|(x, _), (y, _)| more_specific(x, y));
    let set_aside = |decided: &Key, why: &dyn Fn(&Rule) -> Discard| -> Vec<Discarded> {
        applicable
            .iter()
            .filter(|(r, _)| &r.key != decided)
            .take(DISCARDED_MAX)
            .map(|(r, _)| Discarded {
                rule: r.key.clone(),
                why: why(r),
            })
            .collect()
    };
    // 2. A deny wins, whatever authorisation also applies.
    if let Some((rule, combination)) = applicable.iter().find(|(r, _)| r.effect == Effect::Deny) {
        let discarded = set_aside(&rule.key, &|r| match r.effect {
            Effect::Deny => Discard::LessSpecific,
            Effect::Allow => Discard::DenyWins,
        });
        return (
            Verdict::Denied,
            Decisive::Rule(rule.key.clone()),
            combination.clone(),
            discarded,
        );
    }
    // 3. An unconditional authorisation (D25: union).
    if let Some((rule, combination)) = applicable
        .iter()
        .find(|(r, _)| r.required_patterns.is_empty())
    {
        let discarded = set_aside(&rule.key, &|r| {
            if r.required_patterns.is_empty() {
                Discard::LessSpecific
            } else {
                Discard::UnconditionalFirst
            }
        });
        return (
            Verdict::Allowed,
            Decisive::Rule(rule.key.clone()),
            combination.clone(),
            discarded,
        );
    }
    // 4. A conditional authorisation whose required pattern matches.
    for (rule, combination) in &applicable {
        if let Some(pattern) = rule
            .required_patterns
            .iter()
            .find(|p| pattern_matches(snapshot, p, a, b, service))
        {
            let decisive = Decisive::Pattern {
                pattern: pattern.clone(),
                rule: rule.key.clone(),
            };
            let discarded = set_aside(&rule.key, &|_| Discard::LessSpecific);
            return (Verdict::Allowed, decisive, combination.clone(), discarded);
        }
    }
    // 5. Allowed under reserve.
    if let Some((rule, combination)) = applicable.first() {
        let expected: BTreeSet<Key> = applicable
            .iter()
            .flat_map(|(r, _)| r.required_patterns.iter().cloned())
            .collect();
        let decisive = Decisive::Pending {
            rule: rule.key.clone(),
            expected: expected.into_iter().collect(),
        };
        let discarded = set_aside(&rule.key, &|_| Discard::LessSpecific);
        return (
            Verdict::AllowedPending,
            decisive,
            combination.clone(),
            discarded,
        );
    }
    // 6. Denied by default.
    (
        Verdict::Denied,
        Decisive::NoRule,
        Combination::new(),
        Vec::new(),
    )
}

/// A pattern matches when one of its allows applies and none of its denies does
/// (REQ-010-9).
fn pattern_matches(
    snapshot: &PolicySnapshot,
    pattern: &Key,
    a: &Endpoint,
    b: &Endpoint,
    service: &Key,
) -> bool {
    snapshot
        .pattern_rules(pattern, service, Effect::Allow)
        .any(|r| applies(r, a, b).is_some())
        && !snapshot
            .pattern_rules(pattern, service, Effect::Deny)
            .any(|r| applies(r, a, b).is_some())
}

fn max_rank(
    snapshot: &PolicySnapshot,
    axis: &AxisKey,
    values: Option<&BTreeSet<AxisValue>>,
) -> Option<i32> {
    values?
        .iter()
        .filter_map(|v| snapshot.attributes(axis, v).rank)
        .max()
}

/// Evaluates a pair for a service: verdict, outcome, score (REQ-010-11, 36, 37). Neither
/// the order of the rules nor the signals change the verdict or the outcome.
#[must_use]
pub fn evaluate(
    snapshot: &PolicySnapshot,
    a: &Endpoint,
    b: &Endpoint,
    service: &Key,
    requested: Requested,
    dossier: Option<&Dossier>,
) -> Evaluation {
    let (verdict, decisive, combination, discarded) = verdict(snapshot, a, b, service);
    let Some(svc) = snapshot.service(service) else {
        return unknown_service();
    };
    let mut conditions = Vec::new();
    let outcome = match verdict {
        Verdict::Denied => Outcome::Noncompliant,
        Verdict::AllowedPending => Outcome::Review,
        Verdict::Allowed => {
            let deciding_rule = match &decisive {
                Decisive::Rule(k) | Decisive::Pattern { rule: k, .. } => Some(k),
                _ => None,
            };
            for condition in snapshot.conditions.values() {
                let applicable = (condition.rules.is_empty() && condition.flow_types.is_empty())
                    || deciding_rule.is_some_and(|k| condition.rules.contains(k))
                    || condition.flow_types.contains(&svc.flow_type);
                if !applicable {
                    continue;
                }
                let state = condition_state(snapshot, &condition.kind, a, b, requested, dossier);
                conditions.push(ConditionResult {
                    condition: condition.key.clone(),
                    state,
                });
            }
            if conditions.iter().all(|c| c.state == ConditionState::Met) {
                Outcome::Automatic
            } else {
                Outcome::Review
            }
        }
    };
    let mut signals = Vec::new();
    let mut score = 0i64;
    for signal in snapshot.signals.values() {
        let raised = match &signal.kind {
            SignalKind::WideEndpoint(n) => a.width >= *n || b.width >= *n,
            SignalKind::FlowType(types) => types.contains(&svc.flow_type),
            SignalKind::Service(services) => services.contains(&svc.key),
            SignalKind::AxisDiffers(axis) => a.on(axis) != b.on(axis),
            SignalKind::DestinationHigherRank(axis) => {
                match (
                    max_rank(snapshot, axis, a.on(axis)),
                    max_rank(snapshot, axis, b.on(axis)),
                ) {
                    (Some(ra), Some(rb)) => rb > ra,
                    _ => false,
                }
            }
            SignalKind::EndpointValue(axis, values) => [a, b]
                .iter()
                .any(|e| e.on(axis).is_some_and(|s| !s.is_disjoint(values))),
            SignalKind::TransverseOnly => {
                verdict != Verdict::Denied
                    && combination
                        .iter()
                        .any(|((_, axis), value)| snapshot.attributes(axis, value).transverse)
            }
            SignalKind::RankAtLeast(axis, rank) => [a, b]
                .iter()
                .any(|e| max_rank(snapshot, axis, e.on(axis)).is_some_and(|r| r >= *rank)),
        };
        if raised {
            signals.push(signal.key.clone());
            score += i64::from(signal.weight);
        }
    }
    Evaluation {
        service: Some(service.clone()),
        verdict,
        decisive,
        combination,
        discarded,
        conditions,
        outcome,
        signals,
        score,
    }
}

fn condition_state(
    snapshot: &PolicySnapshot,
    kind: &ConditionKind,
    a: &Endpoint,
    b: &Endpoint,
    requested: Requested,
    dossier: Option<&Dossier>,
) -> ConditionState {
    let met = |ok: bool| {
        if ok {
            ConditionState::Met
        } else {
            ConditionState::Unmet
        }
    };
    match kind {
        ConditionKind::DossierField(field) => dossier.map_or(ConditionState::NotEvaluated, |d| {
            met(d.fields.contains(field))
        }),
        ConditionKind::MaxWidth(max) => met(a.width <= *max && b.width <= *max),
        ConditionKind::NotWholeDomain => met(!a.whole_domain && !b.whole_domain),
        ConditionKind::NamedService => met(requested == Requested::Service),
        ConditionKind::DossierRankAtMost(axis) => {
            let Some(dossier) = dossier else {
                return ConditionState::NotEvaluated;
            };
            match (
                max_rank(snapshot, axis, dossier.values.get(axis)),
                max_rank(snapshot, axis, b.on(axis)),
            ) {
                (Some(data), Some(destination)) => met(data <= destination),
                _ => ConditionState::NotEvaluated,
            }
        }
    }
}

/// Synthesis of several evaluations (REQ-010-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Synthesis {
    /// `Allowed` if every verdict allows (under reserve included), `Denied` if none does.
    pub verdict: SynthesisVerdict,
    /// Evaluations that allow.
    pub allowed: usize,
    /// Evaluations that deny.
    pub denied: usize,
    /// The worst outcome.
    pub outcome: Outcome,
}

/// Verdict of a synthesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthesisVerdict {
    /// Every verdict allows.
    Allowed,
    /// Some allow, some deny.
    Partial,
    /// None allows.
    Denied,
}

/// Synthesis of evaluations; `None` when there are none.
#[must_use]
pub fn synthesis(evaluations: &[Evaluation]) -> Option<Synthesis> {
    let outcome = evaluations.iter().map(|e| e.outcome).max()?;
    let denied = evaluations
        .iter()
        .filter(|e| e.verdict == Verdict::Denied)
        .count();
    let allowed = evaluations.len() - denied;
    let verdict = match (allowed, denied) {
        (_, 0) => SynthesisVerdict::Allowed,
        (0, _) => SynthesisVerdict::Denied,
        _ => SynthesisVerdict::Partial,
    };
    Some(Synthesis {
        verdict,
        allowed,
        denied,
        outcome,
    })
}

#[cfg(test)]
#[allow(clippy::many_single_char_names)]
pub(crate) mod tests {
    use super::*;
    use crate::model::tests::{axis, fr, key};
    use crate::model::{
        Axis, Carrier, Condition, FlowSelector, FlowType, Pattern, RawEntry, Service, ServiceEntry,
        Signal, Text, Texts, ValueAttributes,
    };
    use crate::snapshot::Parts;
    use proptest::prelude::*;

    pub(crate) const ENV: &str = "policy.environment";

    pub(crate) fn texts() -> Texts {
        let mut t = Texts::default();
        t.0.insert(fr(), Text::new("Bonne pratique.", None).unwrap());
        t
    }

    pub(crate) fn service(name: &str, flow_type: &str, port: i64) -> Service {
        let entry = ServiceEntry::new(&RawEntry {
            transport: "tcp",
            ports: Some((port, port)),
            ..RawEntry::default()
        })
        .unwrap();
        Service::new(key(name), &[key(flow_type)], vec![entry]).unwrap()
    }

    pub(crate) fn rule(
        name: &str,
        effect: Effect,
        source: &[(&str, &[&str])],
        destination: &[(&str, &[&str])],
        flow_types: &[&str],
    ) -> Rule {
        let selector = |terms: &[(&str, &[&str])]| {
            terms
                .iter()
                .map(|(a, vs)| (axis(a), vs.iter().map(|v| AxisValue::from(*v)).collect()))
                .collect()
        };
        Rule {
            key: key(name),
            source: selector(source),
            destination: selector(destination),
            same: BTreeSet::new(),
            flows: FlowSelector {
                flow_types: flow_types.iter().map(|t| key(t)).collect(),
                services: BTreeSet::new(),
            },
            effect,
            required_patterns: BTreeSet::new(),
            pattern: None,
            texts: texts(),
        }
    }

    pub(crate) fn parts(rules: Vec<Rule>) -> Parts {
        Parts {
            revision: 1,
            default_locale: fr(),
            axes: [
                ENV,
                "policy.sensitivity",
                "policy.application",
                "policy.conduit_role",
            ]
            .iter()
            .map(|k| Axis {
                key: axis(k),
                carrier: Carrier::Tree,
                inference: *k == ENV,
            })
            .collect(),
            values: vec![
                (
                    axis(ENV),
                    "global".into(),
                    ValueAttributes {
                        transverse: true,
                        rank: None,
                    },
                ),
                (
                    axis("policy.sensitivity"),
                    "interne".into(),
                    ValueAttributes {
                        transverse: false,
                        rank: Some(2),
                    },
                ),
                (
                    axis("policy.sensitivity"),
                    "confidentiel".into(),
                    ValueAttributes {
                        transverse: false,
                        rank: Some(3),
                    },
                ),
            ],
            flow_types: ["admin", "application", "file", "database"]
                .iter()
                .map(|t| FlowType { key: key(t) })
                .collect(),
            services: vec![
                service("ssh", "admin", 22),
                service("rdp", "admin", 3389),
                service("https", "application", 443),
                service("sftp", "file", 2222),
                service("pg", "database", 5432),
            ],
            rules,
            patterns: Vec::new(),
            conditions: Vec::new(),
            signals: Vec::new(),
        }
    }

    pub(crate) fn endpoint(values: &[(&str, &[&str])]) -> Endpoint {
        Endpoint {
            values: values
                .iter()
                .map(|(a, vs)| (axis(a), vs.iter().map(|v| AxisValue::from(*v)).collect()))
                .collect(),
            width: 1,
            whole_domain: false,
        }
    }

    fn ssh(snapshot: &PolicySnapshot, a: &Endpoint, b: &Endpoint) -> Evaluation {
        evaluate(snapshot, a, b, &key("ssh"), Requested::Service, None)
    }

    #[test]
    fn acceptance_of_req_010_11() {
        let allow = rule(
            "global-dev",
            Effect::Allow,
            &[(ENV, &["global"])],
            &[(ENV, &["dev"])],
            &["admin"],
        );
        let a = endpoint(&[(ENV, &["prod", "global"])]);
        let b = endpoint(&[(ENV, &["dev"])]);

        let only_allow = PolicySnapshot::new(parts(vec![allow.clone()]));
        let e = ssh(&only_allow, &a, &b);
        assert_eq!(e.verdict, Verdict::Allowed);
        assert_eq!(e.decisive, Decisive::Rule(key("global-dev")));
        assert_eq!(
            e.combination.get(&(Side::Source, axis(ENV))),
            Some(&"global".into())
        );
        assert_eq!(
            e.combination.get(&(Side::Destination, axis(ENV))),
            Some(&"dev".into())
        );
        assert_eq!(e.outcome, Outcome::Automatic);

        let deny = rule(
            "no-prod-dev",
            Effect::Deny,
            &[(ENV, &["prod"])],
            &[(ENV, &["dev"])],
            &["admin"],
        );
        let with_deny = PolicySnapshot::new(parts(vec![allow, deny]));
        let e = ssh(&with_deny, &a, &b);
        assert_eq!(e.verdict, Verdict::Denied);
        assert_eq!(e.decisive, Decisive::Rule(key("no-prod-dev")));
        assert_eq!(
            e.discarded,
            [Discarded {
                rule: key("global-dev"),
                why: Discard::DenyWins
            }]
        );
        assert_eq!(e.outcome, Outcome::Noncompliant);

        let unclassified = ssh(&with_deny, &a, &endpoint(&[]));
        assert_eq!(
            unclassified.decisive,
            Decisive::Unclassified {
                side: Side::Destination,
                axis: axis(ENV)
            }
        );

        // No candidate for ssh cites the sensitivity: its absence is not a refusal.
        let b_no_sensitivity = endpoint(&[(ENV, &["dev"])]);
        assert!(!matches!(
            ssh(&with_deny, &a, &b_no_sensitivity).decisive,
            Decisive::Unclassified { .. }
        ));
        // No rule at all: denied by default (REQ-010-10).
        let empty = PolicySnapshot::new(parts(vec![]));
        assert_eq!(ssh(&empty, &a, &b).decisive, Decisive::NoRule);
    }

    #[test]
    fn same_value_needs_a_shared_value() {
        let mut same_app = rule(
            "same-app",
            Effect::Allow,
            &[(ENV, &["prod"])],
            &[(ENV, &["prod"])],
            &["database"],
        );
        same_app.same.insert(axis("policy.application"));
        let s = PolicySnapshot::new(parts(vec![same_app]));
        let pg = |a: &Endpoint, b: &Endpoint| {
            evaluate(&s, a, b, &key("pg"), Requested::Service, None).verdict
        };
        let a = endpoint(&[(ENV, &["prod"]), ("policy.application", &["crm", "erp"])]);
        let b = endpoint(&[(ENV, &["prod"]), ("policy.application", &["erp"])]);
        let c = endpoint(&[(ENV, &["prod"]), ("policy.application", &["wiki"])]);
        assert_eq!(pg(&a, &b), Verdict::Allowed);
        assert_eq!(pg(&a, &c), Verdict::Denied);
    }

    fn relay_parts() -> Parts {
        let mut conditional = rule(
            "prod-file-global",
            Effect::Allow,
            &[(ENV, &["prod"])],
            &[(ENV, &["global"])],
            &["file"],
        );
        conditional
            .required_patterns
            .insert(key("object-storage-relay"));
        let mut to_store = rule(
            "relay-in",
            Effect::Allow,
            &[],
            &[("policy.conduit_role", &["object_storage"])],
            &["file"],
        );
        to_store.pattern = Some(key("object-storage-relay"));
        let mut from_store = rule(
            "relay-out",
            Effect::Allow,
            &[("policy.conduit_role", &["object_storage"])],
            &[],
            &["file"],
        );
        from_store.pattern = Some(key("object-storage-relay"));
        let mut p = parts(vec![conditional, to_store, from_store]);
        p.patterns.push(Pattern {
            key: key("object-storage-relay"),
            texts: texts(),
        });
        p
    }

    #[test]
    fn acceptance_of_req_010_32() {
        let s = PolicySnapshot::new(relay_parts());
        let sftp = |b: &Endpoint| {
            let a = endpoint(&[(ENV, &["prod"]), ("policy.conduit_role", &["none"])]);
            evaluate(&s, &a, b, &key("sftp"), Requested::Service, None)
        };
        let store = sftp(&endpoint(&[
            (ENV, &["global"]),
            ("policy.conduit_role", &["object_storage"]),
        ]));
        assert_eq!(store.verdict, Verdict::Allowed);
        // The pattern's own rule is an unconditional allow too: it decides first.
        assert!(matches!(
            store.decisive,
            Decisive::Rule(_) | Decisive::Pattern { .. }
        ));
        let files = sftp(&endpoint(&[
            (ENV, &["global"]),
            ("policy.conduit_role", &["file_server"]),
        ]));
        assert_eq!(files.verdict, Verdict::AllowedPending);
        assert_eq!(
            files.decisive,
            Decisive::Pending {
                rule: key("prod-file-global"),
                expected: vec![key("object-storage-relay")]
            }
        );
        assert_eq!(files.outcome, Outcome::Review);

        // An unconditional authorisation applicable at the same time wins.
        let mut p = relay_parts();
        p.rules.push(rule(
            "prod-file-any",
            Effect::Allow,
            &[(ENV, &["prod"])],
            &[],
            &["file"],
        ));
        let s = PolicySnapshot::new(p);
        let a = endpoint(&[(ENV, &["prod"]), ("policy.conduit_role", &["none"])]);
        let b = endpoint(&[
            (ENV, &["global"]),
            ("policy.conduit_role", &["file_server"]),
        ]);
        let e = evaluate(&s, &a, &b, &key("sftp"), Requested::Service, None);
        assert_eq!(
            (e.verdict, e.decisive),
            (Verdict::Allowed, Decisive::Rule(key("prod-file-any")))
        );
    }

    #[test]
    fn a_pattern_decides_when_only_conditional_rules_apply() {
        let mut conditional = rule(
            "prod-file-global",
            Effect::Allow,
            &[(ENV, &["prod"])],
            &[(ENV, &["global"])],
            &["file"],
        );
        conditional.required_patterns.insert(key("relay"));
        // The pattern's allow only covers a service the conditional rule does not.
        let mut member = rule(
            "relay-https",
            Effect::Allow,
            &[],
            &[("policy.conduit_role", &["object_storage"])],
            &["file"],
        );
        member.pattern = Some(key("relay"));
        member
            .source
            .insert(axis(ENV), BTreeSet::from(["prod".into()]));
        member
            .destination
            .insert(axis(ENV), BTreeSet::from(["global".into()]));
        let mut p = parts(vec![conditional, member]);
        p.patterns.push(Pattern {
            key: key("relay"),
            texts: texts(),
        });
        let s = PolicySnapshot::new(p);
        let a = endpoint(&[(ENV, &["prod"]), ("policy.conduit_role", &["none"])]);
        let b = endpoint(&[
            (ENV, &["global"]),
            ("policy.conduit_role", &["object_storage"]),
        ]);
        let e = evaluate(&s, &a, &b, &key("sftp"), Requested::Service, None);
        assert_eq!(e.verdict, Verdict::Allowed);
    }

    fn condition(name: &str, kind: ConditionKind) -> Condition {
        Condition {
            key: key(name),
            kind,
            rules: BTreeSet::new(),
            flow_types: BTreeSet::new(),
            texts: texts(),
        }
    }

    #[test]
    fn acceptance_of_req_010_36() {
        let allow = rule(
            "prod-app",
            Effect::Allow,
            &[(ENV, &["prod"])],
            &[(ENV, &["prod"])],
            &["application"],
        );
        let mut p = parts(vec![allow]);
        p.conditions
            .push(condition("max-width", ConditionKind::MaxWidth(256)));
        p.conditions.push(condition(
            "data-sensitivity",
            ConditionKind::DossierRankAtMost(axis("policy.sensitivity")),
        ));
        let s = PolicySnapshot::new(p);
        let narrow = endpoint(&[(ENV, &["prod"]), ("policy.sensitivity", &["interne"])]);
        let mut wide = narrow.clone();
        wide.width = 65_536;
        let file = |sensitivity: &str| Dossier {
            fields: BTreeSet::from([DossierField::DataNature]),
            values: BTreeMap::from([(
                axis("policy.sensitivity"),
                BTreeSet::from([sensitivity.into()]),
            )]),
        };
        let https = |a: &Endpoint, d: Option<&Dossier>| {
            evaluate(&s, a, &narrow, &key("https"), Requested::Service, d)
        };
        let ok = https(&narrow, Some(&file("interne")));
        assert_eq!(
            (ok.verdict, ok.outcome),
            (Verdict::Allowed, Outcome::Automatic)
        );
        let from_16 = https(&wide, Some(&file("interne")));
        assert_eq!(from_16.outcome, Outcome::Review);
        assert!(from_16.conditions.contains(&ConditionResult {
            condition: key("max-width"),
            state: ConditionState::Unmet
        }));
        let confidential = https(&narrow, Some(&file("confidentiel")));
        assert_eq!(confidential.outcome, Outcome::Review);
        // Without a file, a condition reading it is not evaluated, and the outcome is not
        // automatic (REQ-010-38).
        let no_file = https(&narrow, None);
        assert_eq!(no_file.outcome, Outcome::Review);
        assert!(
            no_file
                .conditions
                .iter()
                .any(|c| c.state == ConditionState::NotEvaluated)
        );
    }

    #[test]
    fn signals_add_up_without_deciding() {
        let allow = rule(
            "prod-admin",
            Effect::Allow,
            &[(ENV, &["global"])],
            &[(ENV, &["prod"])],
            &["admin"],
        );
        let mut p = parts(vec![allow]);
        p.signals = vec![
            Signal {
                key: key("admin"),
                kind: SignalKind::FlowType(BTreeSet::from([key("admin")])),
                weight: 3,
            },
            Signal {
                key: key("transverse"),
                kind: SignalKind::TransverseOnly,
                weight: 2,
            },
            Signal {
                key: key("envs-differ"),
                kind: SignalKind::AxisDiffers(axis(ENV)),
                weight: 1,
            },
            Signal {
                key: key("wide"),
                kind: SignalKind::WideEndpoint(256),
                weight: 5,
            },
        ];
        let s = PolicySnapshot::new(p);
        let e = ssh(
            &s,
            &endpoint(&[(ENV, &["global"])]),
            &endpoint(&[(ENV, &["prod"])]),
        );
        assert_eq!(
            e.signals,
            [key("admin"), key("envs-differ"), key("transverse")]
        );
        assert_eq!(e.score, 6);
        assert_eq!(
            (e.verdict, e.outcome),
            (Verdict::Allowed, Outcome::Automatic)
        );
    }

    #[test]
    fn synthesis_of_req_010_15() {
        assert_eq!(synthesis(&[]), None);
        let mut allowed = unknown_service();
        allowed.verdict = Verdict::Allowed;
        allowed.outcome = Outcome::Automatic;
        let denied = unknown_service();
        let s = synthesis(&[allowed.clone(), denied]).unwrap();
        assert_eq!(
            (s.verdict, s.allowed, s.denied, s.outcome),
            (SynthesisVerdict::Partial, 1, 1, Outcome::Review)
        );
        assert_eq!(
            synthesis(&[allowed]).unwrap().verdict,
            SynthesisVerdict::Allowed
        );
    }

    #[test]
    fn a_port_stands_for_each_service_that_covers_it() {
        let mut p = parts(vec![]);
        p.services.push(service("https-admin", "admin", 443));
        let s = PolicySnapshot::new(p);
        let keys: Vec<_> = s
            .services_for_port(crate::model::Transport::Tcp, 443)
            .iter()
            .map(|s| s.key.clone())
            .collect();
        assert_eq!(keys, [key("https"), key("https-admin")]);
        assert!(
            s.services_for_port(crate::model::Transport::Tcp, 9999)
                .is_empty()
        );
        assert_eq!(unknown_service().outcome, Outcome::Review);
    }

    // Oracle (REQ-010-11): random policies over two axes of three values, endpoints
    // classified on every axis.
    pub(crate) const AXES: [&str; 2] = [ENV, "policy.sensitivity"];
    pub(crate) const VALUES: [&str; 3] = ["v0", "v1", "v2"];

    pub(crate) fn arb_selector() -> impl Strategy<Value = Vec<(usize, Vec<usize>)>> {
        prop::collection::vec(
            (0..AXES.len(), prop::collection::vec(0..VALUES.len(), 1..3)),
            0..3,
        )
    }

    pub(crate) fn arb_rule(i: usize) -> impl Strategy<Value = Rule> {
        (
            arb_selector(),
            arb_selector(),
            prop::bool::ANY,
            prop::sample::select(vec!["admin", "application"]),
            prop::option::of(0..AXES.len()),
        )
            .prop_map(move |(src, dst, deny, flow, same)| {
                let sel = |terms: Vec<(usize, Vec<usize>)>| -> crate::model::Selector {
                    let mut out = crate::model::Selector::new();
                    for (a, vs) in terms {
                        out.entry(axis(AXES[a]))
                            .or_default()
                            .extend(vs.into_iter().map(|v| AxisValue::from(VALUES[v])));
                    }
                    out
                };
                let mut r = rule(
                    &format!("r{i}"),
                    if deny { Effect::Deny } else { Effect::Allow },
                    &[],
                    &[],
                    &[flow],
                );
                r.source = sel(src);
                r.destination = sel(dst);
                if let Some(a) = same {
                    r.same.insert(axis(AXES[a]));
                }
                r
            })
    }

    pub(crate) fn arb_rules() -> impl Strategy<Value = Vec<Rule>> {
        (0..6usize).prop_flat_map(|n| (0..n).map(arb_rule).collect::<Vec<_>>())
    }

    pub(crate) fn arb_endpoint() -> impl Strategy<Value = Endpoint> {
        prop::collection::vec(
            prop::collection::btree_set(0..VALUES.len(), 1..3),
            AXES.len(),
        )
        .prop_map(|sets| Endpoint {
            values: sets
                .into_iter()
                .enumerate()
                .map(|(a, vs)| {
                    (
                        axis(AXES[a]),
                        vs.into_iter().map(|v| AxisValue::from(VALUES[v])).collect(),
                    )
                })
                .collect(),
            width: 1,
            whole_domain: false,
        })
    }

    proptest! {
        #[test]
        fn a_deny_wins(rules in arb_rules(), a in arb_endpoint(), b in arb_endpoint()) {
            let s = PolicySnapshot::new(parts(rules.clone()));
            let e = ssh(&s, &a, &b);
            let deny_applies = rules.iter().any(|r| r.effect == Effect::Deny
                && r.flows.flow_types.contains(&key("admin")) && applies(r, &a, &b).is_some());
            prop_assert_eq!(deny_applies, matches!(e.decisive, Decisive::Rule(ref k) if s.rule(k).unwrap().effect == Effect::Deny));
            if deny_applies { prop_assert_eq!(e.verdict, Verdict::Denied); }
        }

        #[test]
        fn monotonic(rules in arb_rules(), extra in arb_rule(99), a in arb_endpoint(), b in arb_endpoint()) {
            let before = ssh(&PolicySnapshot::new(parts(rules.clone())), &a, &b).verdict;
            let mut more = rules;
            more.push(extra.clone());
            let after = ssh(&PolicySnapshot::new(parts(more)), &a, &b).verdict;
            match extra.effect {
                // An authorisation never closes; a deny never opens.
                Effect::Allow => prop_assert!(!(before != Verdict::Denied && after == Verdict::Denied)),
                Effect::Deny => prop_assert!(!(before == Verdict::Denied && after != Verdict::Denied)),
            }
        }

        #[test]
        fn the_order_of_the_rules_has_no_effect(rules in arb_rules(), a in arb_endpoint(), b in arb_endpoint()) {
            let forward = ssh(&PolicySnapshot::new(parts(rules.clone())), &a, &b);
            let mut reversed = rules;
            reversed.reverse();
            prop_assert_eq!(forward, ssh(&PolicySnapshot::new(parts(reversed)), &a, &b));
        }

        #[test]
        fn signals_change_neither_verdict_nor_outcome(rules in arb_rules(), a in arb_endpoint(), b in arb_endpoint(), w in -5i32..5) {
            let plain = ssh(&PolicySnapshot::new(parts(rules.clone())), &a, &b);
            let mut p = parts(rules);
            p.signals = vec![
                Signal { key: key("differs"), kind: SignalKind::AxisDiffers(axis(ENV)), weight: w },
                Signal { key: key("v0"), kind: SignalKind::EndpointValue(axis(ENV), BTreeSet::from(["v0".into()])), weight: 1 },
            ];
            let signalled = ssh(&PolicySnapshot::new(p), &a, &b);
            prop_assert_eq!((plain.verdict, plain.outcome), (signalled.verdict, signalled.outcome));
        }
    }
}
