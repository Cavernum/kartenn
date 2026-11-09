//! Label selectors (réf. `docs/reference/kubernetes-calcul.md`, § Namespace, labels, noms
//! et UID; « Labels and Selectors », « Label selectors »).
//!
//! Documented by Kubernetes, and kept: requirements combine by AND; a pair of
//! `matchLabels` is an `In` requirement of one value; `NotIn` (like `!=`) also keeps the
//! objects without the key; `In` and `NotIn` need a non-empty set of values; a Service
//! selector is a map, equality only.
//!
//! Chosen by kartenn: the empty selector matches every object, as the AND of no
//! requirement; the meaning of an empty selector depends on its context in Kubernetes, so
//! a caller for which it selects nothing (a Service without selector) asks
//! [`Selector::is_empty`] first. The values of `Exists` and `DoesNotExist`, which carry
//! none, are ignored rather than refused: the mirror does not refuse what the cluster
//! admitted.

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::rules::Violation;

use crate::names::{LabelKey, LabelValue, Labels};
use crate::rules::LABEL_INVALID;

/// One requirement of a selector.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Requirement {
    /// The key is present with this value (`matchLabels`, a Service selector, `=`).
    Equals(LabelKey, LabelValue),
    /// The key is present with one of these values.
    In(LabelKey, BTreeSet<LabelValue>),
    /// The key is absent, or present with none of these values.
    NotIn(LabelKey, BTreeSet<LabelValue>),
    /// The key is present, whatever its value.
    Exists(LabelKey),
    /// The key is absent.
    DoesNotExist(LabelKey),
}

impl Requirement {
    /// Parses an element of `matchExpressions`: a key, an operator as the API spells it
    /// (`In`, `NotIn`, `Exists`, `DoesNotExist`) and its values.
    ///
    /// # Errors
    /// `kube.label.invalid`: key or value out of syntax, unknown operator, `In` or `NotIn`
    /// without values.
    pub fn parse(key: &str, operator: &str, values: &[&str]) -> Result<Self, Violation> {
        let key = LabelKey::parse(key)?;
        let set = || -> Result<BTreeSet<LabelValue>, Violation> {
            if values.is_empty() {
                return Err(Violation::new(
                    &LABEL_INVALID,
                    format!("sélecteur « {key} {operator} » : il faut au moins une valeur"),
                ));
            }
            values.iter().map(|v| LabelValue::parse(v)).collect()
        };
        match operator {
            "In" => Ok(Self::In(key.clone(), set()?)),
            "NotIn" => Ok(Self::NotIn(key.clone(), set()?)),
            "Exists" => Ok(Self::Exists(key)),
            "DoesNotExist" => Ok(Self::DoesNotExist(key)),
            other => Err(Violation::new(
                &LABEL_INVALID,
                format!("sélecteur « {key} » : opérateur « {other} » inconnu"),
            )),
        }
    }

    /// Whether labels satisfy the requirement.
    #[must_use]
    pub fn matches(&self, labels: &Labels) -> bool {
        self.matches_with(|key| labels.get(key).map(LabelValue::as_str))
    }

    /// Whether the labels that `get` reads satisfy the requirement.
    fn matches_with<'a>(&self, get: impl Fn(&str) -> Option<&'a str>) -> bool {
        match self {
            Self::Equals(key, value) => get(key.as_str()) == Some(value.as_str()),
            Self::In(key, values) => get(key.as_str()).is_some_and(|v| values.contains(v)),
            Self::NotIn(key, values) => get(key.as_str()).is_none_or(|v| !values.contains(v)),
            Self::Exists(key) => get(key.as_str()).is_some(),
            Self::DoesNotExist(key) => get(key.as_str()).is_none(),
        }
    }
}

/// A label selector: requirements combined by AND.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Selector(BTreeSet<Requirement>);

impl Selector {
    /// A selector of these requirements.
    #[must_use]
    pub fn new(requirements: impl IntoIterator<Item = Requirement>) -> Self {
        Self(requirements.into_iter().collect())
    }

    /// An equality selector, as `matchLabels` or a Service selector.
    #[must_use]
    pub fn of_labels(labels: Labels) -> Self {
        Self::new(labels.into_iter().map(|(k, v)| Requirement::Equals(k, v)))
    }

    /// Whether the selector has no requirement.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The requirements, in a stable order.
    pub fn requirements(&self) -> impl Iterator<Item = &Requirement> {
        self.0.iter()
    }

    /// Whether labels satisfy every requirement; the empty selector matches everything.
    #[must_use]
    pub fn matches(&self, labels: &Labels) -> bool {
        self.0.iter().all(|r| r.matches(labels))
    }

    /// Whether labels as read from the cluster, all of them and not only the tracked
    /// ones, satisfy every requirement: a selection is computed on what the cluster holds.
    #[must_use]
    pub fn matches_read(&self, labels: &BTreeMap<String, String>) -> bool {
        self.0
            .iter()
            .all(|r| r.matches_with(|key| labels.get(key).map(String::as_str)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::parse_labels;

    fn labels(pairs: &[(&str, &str)]) -> Labels {
        parse_labels(pairs.iter().copied()).unwrap()
    }

    fn req(key: &str, op: &str, values: &[&str]) -> Requirement {
        Requirement::parse(key, op, values).unwrap()
    }

    #[test]
    fn selector_and_semantics() {
        let selector = Selector::new([
            req("app", "In", &["pay", "billing"]),
            req("tier", "NotIn", &["frontend"]),
            req("partition", "Exists", &[]),
            req("canary", "DoesNotExist", &[]),
        ]);
        let base = [("app", "pay"), ("tier", "db"), ("partition", "a")];
        assert!(selector.matches(&labels(&base)));
        // NotIn keeps an object without the key.
        assert!(selector.matches(&labels(&[("app", "billing"), ("partition", "")])));
        // Each requirement can fail the whole selector on its own.
        for failing in [
            labels(&[("app", "web"), ("tier", "db"), ("partition", "a")]),
            labels(&[("tier", "db"), ("partition", "a")]),
            labels(&[("app", "pay"), ("tier", "frontend"), ("partition", "a")]),
            labels(&[("app", "pay"), ("tier", "db")]),
            labels(&[
                ("app", "pay"),
                ("tier", "db"),
                ("partition", "a"),
                ("canary", "1"),
            ]),
        ] {
            assert!(!selector.matches(&failing), "{failing:?}");
        }
        // The empty selector is the AND of nothing; a caller that means « nothing » asks.
        assert!(Selector::default().matches(&labels(&[])));
        assert!(Selector::default().is_empty());
    }

    #[test]
    fn match_labels_are_equalities() {
        let selector = Selector::of_labels(labels(&[("app", "pay"), ("tier", "db")]));
        assert!(selector.matches(&labels(&[("app", "pay"), ("tier", "db"), ("v", "2")])));
        assert!(!selector.matches(&labels(&[("app", "pay")])));
        assert!(!selector.matches(&labels(&[("app", "pay"), ("tier", "")])));
        // An empty value is a value: `tier=` does not match an object without `tier`.
        let empty = Selector::of_labels(labels(&[("tier", "")]));
        assert!(empty.matches(&labels(&[("tier", "")])));
        assert!(!empty.matches(&labels(&[])));
    }

    #[test]
    fn requirement_refusals() {
        let refused =
            |r: Result<Requirement, Violation>| r.is_err_and(|v| v.rule.id == "kube.label.invalid");
        assert!(refused(Requirement::parse("app", "In", &[])));
        assert!(refused(Requirement::parse("app", "NotIn", &[])));
        assert!(refused(Requirement::parse("app", "Gt", &["1"])));
        assert!(refused(Requirement::parse("app", "in", &["pay"])));
        assert!(refused(Requirement::parse("bad key", "Exists", &[])));
        assert!(refused(Requirement::parse("app", "In", &["-x"])));
        assert_eq!(
            Requirement::parse("app", "Exists", &["ignored"]).unwrap(),
            Requirement::Exists(LabelKey::parse("app").unwrap())
        );
    }
}
