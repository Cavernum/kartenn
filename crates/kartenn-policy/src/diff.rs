//! Policy differential (REQ-010-18; design 010 « Différentiel de politique »): the
//! abstract combinations (values of the source axes, values of the destination axes,
//! service) whose verdict of principle flips between two snapshots.
//!
//! A verdict only depends on which values an endpoint carries among those the candidate
//! rules cite, and, for « same value », on whether both sides share one. An endpoint's
//! set on an axis is therefore described by classes: cited values, « another value » (no
//! rule cites it, different on each side) and, where a rule asks for the same value,
//! « one same uncited value » on both sides. The differential evaluates one
//! representative of each combination of classes on both snapshots, for endpoints that
//! carry one or two values per axis (D25: a node may carry several environments); that
//! bound catches a deny masking an authorisation (REQ-010-18) and keeps the count
//! polynomial in the cited values.

use std::collections::{BTreeMap, BTreeSet};

use crate::evaluate::{Decisive, Endpoint, Side, Verdict, verdict};
use crate::model::{AxisKey, AxisValue, Key, Rule};
use crate::snapshot::PolicySnapshot;

/// Above this many candidates for one differential, only the count is computed
/// (REQ-003-26, design 010 « Borne »).
pub const MAX_CANDIDATES: usize = 100_000;

/// Most values an endpoint carries on an axis in a candidate.
pub const VALUES_PER_AXIS: usize = 2;

/// Class of a value an endpoint carries on an axis.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// A value some candidate rule cites.
    Value(AxisValue),
    /// A value no candidate rule cites, different from the other side's.
    Other,
    /// One value no candidate rule cites, the same on both sides.
    Shared,
}

/// An abstract combination for a service.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Candidate {
    /// The service.
    pub service: Key,
    /// Classes of the source's values, per axis cited by the candidate rules.
    pub source: BTreeMap<AxisKey, BTreeSet<Slot>>,
    /// Classes of the destination's values, per axis.
    pub destination: BTreeMap<AxisKey, BTreeSet<Slot>>,
}

/// A verdict of principle and what decided it.
pub type Principle = (Verdict, Decisive);

/// A combination whose verdict flips.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flip {
    /// The combination.
    pub candidate: Candidate,
    /// Before the change.
    pub before: Principle,
    /// After the change.
    pub after: Principle,
}

impl Flip {
    /// Whether the change opens the combination (`denied` before, allowed after).
    #[must_use]
    pub fn opens(&self) -> bool {
        self.before.0 == Verdict::Denied && self.after.0 != Verdict::Denied
    }

    /// Whether the change closes it.
    #[must_use]
    pub fn closes(&self) -> bool {
        self.before.0 != Verdict::Denied && self.after.0 == Verdict::Denied
    }
}

/// The differential of two snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyDiff {
    /// Flips, by service then combination.
    pub flips: Vec<Flip>,
    /// Candidates evaluated (or counted, when truncated).
    pub candidates: usize,
    /// Whether the candidates exceeded [`MAX_CANDIDATES`]: flips are then not computed.
    pub truncated: bool,
}

impl PolicyDiff {
    /// Combinations opened.
    #[must_use]
    pub fn opens(&self) -> usize {
        self.flips.iter().filter(|f| f.opens()).count()
    }

    /// Combinations closed.
    #[must_use]
    pub fn closes(&self) -> usize {
        self.flips.iter().filter(|f| f.closes()).count()
    }

    /// Combinations whose verdict moves to or from « allowed under reserve » only.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.flips.len() - self.opens() - self.closes()
    }
}

/// Representatives of the uncited classes: no key of a value can start with a NUL.
fn representative(slot: &Slot, side: Side) -> AxisValue {
    match slot {
        Slot::Value(v) => v.clone(),
        Slot::Other => AxisValue::new(match side {
            Side::Source => "\0other-source",
            Side::Destination => "\0other-destination",
        }),
        Slot::Shared => AxisValue::new("\0shared"),
    }
}

/// A pair of class sets (source, destination) for one axis.
type Classes = (BTreeSet<Slot>, BTreeSet<Slot>);

/// Per axis, the cited values and whether a rule asks for the same value.
fn cited(rules: &[&Rule]) -> BTreeMap<AxisKey, (BTreeSet<AxisValue>, bool)> {
    let mut out: BTreeMap<AxisKey, (BTreeSet<AxisValue>, bool)> = BTreeMap::new();
    for rule in rules {
        for selector in [&rule.source, &rule.destination] {
            for (axis, values) in selector {
                out.entry(axis.clone())
                    .or_default()
                    .0
                    .extend(values.iter().cloned());
            }
        }
        for axis in &rule.same {
            out.entry(axis.clone()).or_default().1 = true;
        }
    }
    out
}

/// The number of combinations [`classes`] gives for `rules`, without building them: per
/// axis, n cited values and « another value » make n + 1 slots, taken one or two at a
/// time on each side, plus the shared value when a rule asks for the same value.
fn class_count(rules: &[&Rule]) -> usize {
    cited(rules)
        .values()
        .try_fold(1usize, |n, (values, same)| {
            let slots = values.len().checked_add(1)?;
            let sets = slots.checked_add(slots.checked_mul(slots - 1)? / 2)?;
            let pairs = sets.checked_mul(sets)?.checked_add(usize::from(*same))?;
            n.checked_mul(pairs)
        })
        .unwrap_or(usize::MAX)
}

/// Per axis, the pairs of class sets (source, destination) to try.
fn classes(rules: &[&Rule]) -> Vec<(AxisKey, Vec<Classes>)> {
    cited(rules)
        .into_iter()
        .map(|(axis, (values, same))| {
            let slots: Vec<Slot> = values
                .into_iter()
                .map(Slot::Value)
                .chain([Slot::Other])
                .collect();
            let mut sets: Vec<BTreeSet<Slot>> =
                slots.iter().map(|s| BTreeSet::from([s.clone()])).collect();
            for (i, x) in slots.iter().enumerate() {
                for y in &slots[i + 1..] {
                    sets.push(BTreeSet::from([x.clone(), y.clone()]));
                }
            }
            let mut pairs: Vec<Classes> = sets
                .iter()
                .flat_map(|s| sets.iter().map(move |d| (s.clone(), d.clone())))
                .collect();
            if same {
                let shared = BTreeSet::from([Slot::Shared]);
                pairs.push((shared.clone(), shared));
            }
            (axis, pairs)
        })
        .collect()
}

fn same_candidates(before: &PolicySnapshot, after: &PolicySnapshot, service: &Key) -> bool {
    let flow_type = |s: &PolicySnapshot| s.service(service).map(|svc| svc.flow_type.clone());
    let rules = |s: &PolicySnapshot| s.candidates(service).cloned().collect::<Vec<_>>();
    flow_type(before) == flow_type(after) && rules(before) == rules(after)
}

/// The flips between `before` and `after` (REQ-010-18). Services whose candidate rules
/// did not change are skipped: their verdicts cannot flip.
#[must_use]
pub fn diff(before: &PolicySnapshot, after: &PolicySnapshot) -> PolicyDiff {
    let services: BTreeSet<Key> = before
        .services()
        .chain(after.services())
        .map(|s| s.key.clone())
        .collect();
    let mut out = PolicyDiff::default();
    let mut plans = Vec::new();
    for service in services {
        if same_candidates(before, after, &service) {
            continue;
        }
        let rules: Vec<&Rule> = before
            .candidates(&service)
            .chain(after.candidates(&service))
            .collect();
        // Counted before being built: the combinations of a large policy would not fit
        // in memory (task 011-10).
        out.candidates = out.candidates.saturating_add(class_count(&rules));
        plans.push((service, rules));
    }
    if out.candidates > MAX_CANDIDATES {
        out.truncated = true;
        return out;
    }
    for (service, rules) in plans {
        let axes = classes(&rules);
        let mut index = vec![0usize; axes.len()];
        loop {
            let mut candidate = Candidate {
                service: service.clone(),
                source: BTreeMap::new(),
                destination: BTreeMap::new(),
            };
            let (mut a, mut b) = (Endpoint::default(), Endpoint::default());
            for ((axis, pairs), i) in axes.iter().zip(&index) {
                let (s, d) = &pairs[*i];
                let values = |slots: &BTreeSet<Slot>, side| {
                    slots
                        .iter()
                        .map(|slot| representative(slot, side))
                        .collect()
                };
                a.values.insert(axis.clone(), values(s, Side::Source));
                b.values.insert(axis.clone(), values(d, Side::Destination));
                candidate.source.insert(axis.clone(), s.clone());
                candidate.destination.insert(axis.clone(), d.clone());
            }
            let (vb, db, ..) = verdict(before, &a, &b, &service);
            let (va, da, ..) = verdict(after, &a, &b, &service);
            if vb != va {
                out.flips.push(Flip {
                    candidate,
                    before: (vb, db),
                    after: (va, da),
                });
            }
            // Next combination (odometer); done when every digit wrapped.
            let mut digit = 0;
            while digit < axes.len() {
                index[digit] += 1;
                if index[digit] < axes[digit].1.len() {
                    break;
                }
                index[digit] = 0;
                digit += 1;
            }
            if digit == axes.len() {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::many_single_char_names)]
mod tests {
    use super::*;
    use crate::evaluate::tests::{ENV, arb_rules, endpoint, parts, rule};
    use crate::model::Effect;
    use crate::model::tests::{axis, key};
    use proptest::prelude::*;

    fn one(value: &str) -> BTreeSet<Slot> {
        BTreeSet::from([Slot::Value(value.into())])
    }

    #[test]
    fn the_combinations_are_counted_as_built() {
        let rules = [
            rule(
                "a",
                Effect::Allow,
                &[(ENV, &["prod", "dev"])],
                &[(ENV, &["dev"])],
                &["admin"],
            ),
            rule(
                "b",
                Effect::Deny,
                &[(ENV, &["test"])],
                &[(ENV, &["prod"])],
                &["admin"],
            ),
        ];
        let rules: Vec<&Rule> = rules.iter().collect();
        let built: usize = classes(&rules).iter().map(|(_, p)| p.len()).product();
        // Three cited values and « another »: 4 + 6 = 10 sets a side.
        assert_eq!(built, 100);
        assert_eq!(class_count(&rules), built);
    }

    #[test]
    fn acceptance_of_req_010_18() {
        let before = PolicySnapshot::new(parts(vec![]));
        let allow = rule(
            "global-dev",
            Effect::Allow,
            &[(ENV, &["global"])],
            &[(ENV, &["dev"])],
            &["admin"],
        );
        let after = PolicySnapshot::new(parts(vec![allow.clone()]));
        let opened = diff(&before, &after);
        // Every combination with `global` at the source and `dev` at the destination
        // opens, for each of the two admin services (ssh, rdp); nothing closes.
        assert_eq!(opened.closes(), 0);
        assert!(
            opened
                .flips
                .iter()
                .all(|f| f.after.1 == Decisive::Rule(key("global-dev")))
        );
        for service in ["ssh", "rdp"] {
            assert!(
                opened
                    .flips
                    .iter()
                    .any(|f| f.candidate.service == key(service)
                        && f.candidate.source[&axis(ENV)] == one("global")
                        && f.candidate.destination[&axis(ENV)] == one("dev"))
            );
        }
        assert!(
            opened
                .flips
                .iter()
                .all(|f| f.candidate.source[&axis(ENV)].contains(&Slot::Value("global".into())))
        );

        let deny = rule(
            "no-prod-dev",
            Effect::Deny,
            &[(ENV, &["prod"])],
            &[(ENV, &["dev"])],
            &["admin"],
        );
        let masked = PolicySnapshot::new(parts(vec![allow, deny]));
        let closed = diff(&after, &masked);
        // A source {global, prod} towards dev: allowed, then masked by the deny.
        assert_eq!(closed.opens(), 0);
        assert!(closed.closes() >= 2);
        assert!(
            closed
                .flips
                .iter()
                .all(|f| f.after.1 == Decisive::Rule(key("no-prod-dev")))
        );
        let both = BTreeSet::from([Slot::Value("global".into()), Slot::Value("prod".into())]);
        assert!(
            closed
                .flips
                .iter()
                .any(|f| f.candidate.source[&axis(ENV)] == both)
        );
        assert!(!closed.truncated);

        // Nothing changed: nothing is evaluated.
        assert_eq!(diff(&masked, &masked), PolicyDiff::default());
    }

    // Oracle: the cartesian product of endpoints carrying one or two values per axis, over
    // the cited values and two uncited ones, mapped to classes, flips exactly as the
    // differential says (on the pairs the classes describe).
    const UNIVERSE: [&str; 5] = ["v0", "v1", "v2", "u0", "u1"];

    fn subsets() -> Vec<Vec<&'static str>> {
        let mut out: Vec<Vec<&str>> = UNIVERSE.iter().map(|v| vec![*v]).collect();
        for (i, x) in UNIVERSE.iter().enumerate() {
            for y in &UNIVERSE[i + 1..] {
                out.push(vec![*x, *y]);
            }
        }
        out
    }

    type Row = (Candidate, Principle, Principle);

    fn oracle(before: &PolicySnapshot, after: &PolicySnapshot) -> BTreeSet<Row> {
        let subsets = subsets();
        let mut out = BTreeSet::new();
        for service in before.services().map(|s| s.key.clone()) {
            if same_candidates(before, after, &service) {
                continue;
            }
            let rules: Vec<&Rule> = before
                .candidates(&service)
                .chain(after.candidates(&service))
                .collect();
            let cited = cited(&rules);
            let axes: Vec<&AxisKey> = cited.keys().collect();
            let n = subsets.len();
            let combos = n.pow(2 * u32::try_from(axes.len()).unwrap());
            'combo: for code in 0..combos {
                let pick = |i: usize| &subsets[code / n.pow(u32::try_from(i).unwrap()) % n];
                let (mut a, mut b) = (endpoint(&[]), endpoint(&[]));
                let mut candidate = Candidate {
                    service: service.clone(),
                    source: BTreeMap::new(),
                    destination: BTreeMap::new(),
                };
                for (k, axis) in axes.iter().enumerate() {
                    let (values, same) = &cited[*axis];
                    let sv: BTreeSet<AxisValue> =
                        pick(2 * k).iter().map(|v| AxisValue::from(*v)).collect();
                    let dv: BTreeSet<AxisValue> = pick(2 * k + 1)
                        .iter()
                        .map(|v| AxisValue::from(*v))
                        .collect();
                    let shared_uncited = sv.intersection(&dv).any(|v| !values.contains(v));
                    let (s, d) = if *same && shared_uncited {
                        if sv.len() > 1 || dv.len() > 1 {
                            continue 'combo;
                        }
                        (
                            BTreeSet::from([Slot::Shared]),
                            BTreeSet::from([Slot::Shared]),
                        )
                    } else {
                        let class = |set: &BTreeSet<AxisValue>| -> BTreeSet<Slot> {
                            set.iter()
                                .map(|v| {
                                    if values.contains(v) {
                                        Slot::Value(v.clone())
                                    } else {
                                        Slot::Other
                                    }
                                })
                                .collect()
                        };
                        (class(&sv), class(&dv))
                    };
                    a.values.insert((*axis).clone(), sv);
                    b.values.insert((*axis).clone(), dv);
                    candidate.source.insert((*axis).clone(), s);
                    candidate.destination.insert((*axis).clone(), d);
                }
                let (vb, db, ..) = verdict(before, &a, &b, &service);
                let (va, da, ..) = verdict(after, &a, &b, &service);
                if vb != va {
                    out.insert((candidate, (vb, db), (va, da)));
                }
            }
        }
        out
    }

    proptest! {
        // Each case walks about 10^5 pairs of endpoints per affected service: few cases.
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn equals_the_flips_of_the_cartesian_product(before in arb_rules(), after in arb_rules()) {
            let before = PolicySnapshot::new(parts(before));
            let after = PolicySnapshot::new(parts(after));
            let computed: BTreeSet<Row> = diff(&before, &after)
                .flips
                .into_iter()
                .map(|f| (f.candidate, f.before, f.after))
                .collect();
            prop_assert_eq!(computed, oracle(&before, &after));
        }
    }
}
