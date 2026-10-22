//! The policy at one date, in memory (REQ-010-11, 16, 28; design 010 « Instantané de
//! politique »).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::model::{
    Axis, AxisKey, AxisValue, Condition, Effect, FlowType, Key, Locale, Pattern, Rule, Service,
    Signal, Transport, ValueAttributes,
};

fn index<T>(items: Vec<T>, key: impl Fn(&T) -> Key) -> BTreeMap<Key, T> {
    items.into_iter().map(|i| (key(&i), i)).collect()
}

/// The objects of the policy valid at (`valid_at`, `as_of`), as the storage reads them.
#[derive(Debug, Clone)]
pub struct Parts {
    /// Revision of the policy (cache key and provenance).
    pub revision: u64,
    /// Default language of the instance (REQ-010-33).
    pub default_locale: Locale,
    /// Designated axes.
    pub axes: Vec<Axis>,
    /// Attributes of the described values.
    pub values: Vec<(AxisKey, AxisValue, ValueAttributes)>,
    /// Flow types.
    pub flow_types: Vec<FlowType>,
    /// Services.
    pub services: Vec<Service>,
    /// Rules, in no particular order.
    pub rules: Vec<Rule>,
    /// Patterns.
    pub patterns: Vec<Pattern>,
    /// Admissibility conditions.
    pub conditions: Vec<Condition>,
    /// Signals.
    pub signals: Vec<Signal>,
}

/// The policy at one date, indexed for evaluation.
#[derive(Debug, Clone)]
pub struct PolicySnapshot {
    pub(crate) revision: u64,
    pub(crate) default_locale: Locale,
    pub(crate) axes: BTreeMap<AxisKey, Axis>,
    pub(crate) values: BTreeMap<(AxisKey, AxisValue), ValueAttributes>,
    pub(crate) flow_types: BTreeMap<Key, FlowType>,
    pub(crate) services: BTreeMap<Key, Service>,
    pub(crate) rules: BTreeMap<Key, Arc<Rule>>,
    pub(crate) patterns: BTreeMap<Key, Pattern>,
    pub(crate) conditions: BTreeMap<Key, Condition>,
    pub(crate) signals: BTreeMap<Key, Signal>,
    /// Rules whose flow selector covers each service, types expanded to services.
    by_service: BTreeMap<Key, Arc<Candidates>>,
    /// Rules of each pattern.
    by_pattern: BTreeMap<Key, Vec<Key>>,
}

/// The candidate rules of a service, by key, and the axes they cite (task 011-10: read
/// once per snapshot, not once per evaluation).
#[derive(Debug, Clone, Default)]
struct Candidates {
    rules: Vec<Arc<Rule>>,
    cited: BTreeSet<AxisKey>,
}

impl PolicySnapshot {
    /// Indexes the objects. The order of `parts.rules` has no effect (S23).
    #[must_use]
    pub fn new(parts: Parts) -> Self {
        let services: BTreeMap<Key, Service> = index(parts.services, |s: &Service| s.key.clone());
        let rules: BTreeMap<Key, Arc<Rule>> = parts
            .rules
            .into_iter()
            .map(|r| (r.key.clone(), Arc::new(r)))
            .collect();
        // The rules citing each flow type and each service, by key.
        let mut of_type: BTreeMap<&Key, Vec<&Arc<Rule>>> = BTreeMap::new();
        let mut of_service: BTreeMap<&Key, Vec<&Arc<Rule>>> = BTreeMap::new();
        for rule in rules.values() {
            for flow_type in &rule.flows.flow_types {
                of_type.entry(flow_type).or_default().push(rule);
            }
            for service in &rule.flows.services {
                of_service.entry(service).or_default().push(rule);
            }
        }
        let candidates = |of: &[&[&Arc<Rule>]]| -> Arc<Candidates> {
            let covering: BTreeMap<&Key, &Arc<Rule>> = of
                .iter()
                .flat_map(|rules| rules.iter())
                .map(|r| (&r.key, *r))
                .collect();
            let rules: Vec<Arc<Rule>> = covering.into_values().cloned().collect();
            let cited = rules.iter().flat_map(|r| r.axes()).cloned().collect();
            Arc::new(Candidates { rules, cited })
        };
        // The services of a flow type no rule names share its candidates.
        let mut of_types: BTreeMap<&Key, Arc<Candidates>> = BTreeMap::new();
        let by_service: BTreeMap<Key, Arc<Candidates>> = services
            .values()
            .map(|service| {
                let typed: &[&Arc<Rule>] =
                    of_type.get(&service.flow_type).map_or(&[], Vec::as_slice);
                let found = match of_service.get(&service.key) {
                    Some(named) => candidates(&[typed, named]),
                    None => of_types
                        .entry(&service.flow_type)
                        .or_insert_with(|| candidates(&[typed]))
                        .clone(),
                };
                (service.key.clone(), found)
            })
            .collect();
        let mut by_pattern: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
        for rule in rules.values() {
            if let Some(pattern) = &rule.pattern {
                by_pattern
                    .entry(pattern.clone())
                    .or_default()
                    .push(rule.key.clone());
            }
        }
        Self {
            revision: parts.revision,
            default_locale: parts.default_locale,
            axes: parts.axes.into_iter().map(|a| (a.key.clone(), a)).collect(),
            values: parts
                .values
                .into_iter()
                .map(|(axis, value, attrs)| ((axis, value), attrs))
                .collect(),
            flow_types: index(parts.flow_types, |t: &FlowType| t.key.clone()),
            services,
            rules,
            patterns: index(parts.patterns, |p: &Pattern| p.key.clone()),
            conditions: index(parts.conditions, |c: &Condition| c.key.clone()),
            signals: index(parts.signals, |s: &Signal| s.key.clone()),
            by_service,
            by_pattern,
        }
    }

    /// Revision of the policy.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Default language of the instance.
    #[must_use]
    pub fn default_locale(&self) -> &Locale {
        &self.default_locale
    }

    /// A flow type by key.
    #[must_use]
    pub fn flow_type(&self, key: &Key) -> Option<&FlowType> {
        self.flow_types.get(key)
    }

    /// A service by key.
    #[must_use]
    pub fn service(&self, key: &Key) -> Option<&Service> {
        self.services.get(key)
    }

    /// Every service, by key.
    pub fn services(&self) -> impl Iterator<Item = &Service> {
        self.services.values()
    }

    /// A rule by key.
    #[must_use]
    pub fn rule(&self, key: &Key) -> Option<&Rule> {
        self.rules.get(key).map(AsRef::as_ref)
    }

    /// Every rule, by key.
    pub fn rules(&self) -> impl Iterator<Item = &Rule> {
        self.rules.values().map(AsRef::as_ref)
    }

    /// A pattern by key.
    #[must_use]
    pub fn pattern(&self, key: &Key) -> Option<&Pattern> {
        self.patterns.get(key)
    }

    /// Designated axes.
    #[must_use]
    pub fn axis_keys(&self) -> BTreeSet<AxisKey> {
        self.axes.keys().cloned().collect()
    }

    /// Attributes of a value; default when not described.
    #[must_use]
    pub fn attributes(&self, axis: &AxisKey, value: &AxisValue) -> ValueAttributes {
        self.values
            .get(&(axis.clone(), value.clone()))
            .copied()
            .unwrap_or_default()
    }

    /// Candidate rules of a service: those whose flow selector covers it (REQ-010-11).
    pub fn candidates(&self, service: &Key) -> impl Iterator<Item = &Rule> {
        self.by_service
            .get(service)
            .into_iter()
            .flat_map(|c| c.rules.iter().map(AsRef::as_ref))
    }

    /// The axes the candidate rules of a service cite, in order.
    pub fn cited(&self, service: &Key) -> impl Iterator<Item = &AxisKey> {
        self.by_service
            .get(service)
            .into_iter()
            .flat_map(|c| c.cited.iter())
    }

    /// Rules of a pattern of a given effect that cover the service.
    pub(crate) fn pattern_rules<'a>(
        &'a self,
        pattern: &Key,
        service: &'a Key,
        effect: Effect,
    ) -> impl Iterator<Item = &'a Rule> {
        self.by_pattern
            .get(pattern)
            .into_iter()
            .flatten()
            .filter_map(|k| self.rules.get(k).map(AsRef::as_ref))
            .filter(move |r| {
                r.effect == effect
                    && self.services.get(service).is_some_and(|s| {
                        r.flows.services.contains(&s.key)
                            || r.flows.flow_types.contains(&s.flow_type)
                    })
            })
    }

    /// Services a port of a transport stands for (REQ-010-14).
    #[must_use]
    pub fn services_for_port(&self, transport: Transport, port: u16) -> Vec<&Service> {
        self.services
            .values()
            .filter(|s| s.covers_port(transport, port))
            .collect()
    }

    /// Services of a flow type (REQ-010-14).
    #[must_use]
    pub fn services_of_type(&self, flow_type: &Key) -> Vec<&Service> {
        self.services
            .values()
            .filter(|s| &s.flow_type == flow_type)
            .collect()
    }
}
