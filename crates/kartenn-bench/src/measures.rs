//! The named measures of the benches (ADR 0041, decision 1).
//!
//! A bench is a deterministic preparation (seed, scale) followed by measures named
//! `<bench>.<measure>`, in the order of its catalogue. A measure starts from the state the
//! preparation leaves, or from the one left by the measures it declares as prerequisites:
//! a [`Selection`] runs those before it. A measure that times the preparation itself is
//! marked « de préparation »: it only runs with the preparation.
//!
//! Some measures share a setup of their own (the VLANs of `core`, the inference rules of
//! `policy`…): the bench runs it when one of them is selected, as part of its preparation.
//! One measure may give several lines of the report (the same loop, interleaved draws).
//!
//! The measures draw in turn from the generator of their bench, as before the catalogue:
//! the whole bench draws what it drew. A measure run alone draws what follows the
//! preparation and its prerequisites: other draws, from the same distribution.

use std::fmt::Write as _;

use anyhow::bail;

/// A measure of a bench.
#[derive(Debug)]
pub struct Measure {
    /// `<bench>.<measure>`.
    pub name: &'static str,
    /// The measures whose state this one starts from: run before it.
    pub after: &'static [&'static str],
    /// It times the preparation itself, and only runs with it.
    pub preparation: bool,
}

impl Measure {
    pub(crate) const fn new(name: &'static str) -> Self {
        Self {
            name,
            after: &[],
            preparation: false,
        }
    }

    pub(crate) const fn after(self, after: &'static [&'static str]) -> Self {
        Self { after, ..self }
    }

    pub(crate) const fn preparation(self) -> Self {
        Self {
            preparation: true,
            ..self
        }
    }
}

/// A bench and its measures, in their order of execution.
#[derive(Debug)]
pub struct Bench {
    /// The command of `kartenn-bench`.
    pub name: &'static str,
    pub measures: &'static [Measure],
}

/// The benches of `kartenn-bench` that have measures.
pub static BENCHES: [&Bench; 8] = [
    &crate::props::BENCH,
    &crate::core_budgets::BENCH,
    &crate::rbac::BENCH,
    &crate::http::BENCH,
    &crate::policy_budgets::BENCH,
    &crate::flow_budgets::BENCH,
    &crate::events_budgets::BENCH,
    &crate::telemetry::BENCH,
];

/// The bench of the command `name`.
pub fn bench(name: &str) -> Option<&'static Bench> {
    BENCHES.iter().copied().find(|b| b.name == name)
}

impl Bench {
    fn position(&self, name: &str) -> Option<usize> {
        self.measures.iter().position(|m| m.name == name)
    }

    /// One line per measure: its name, whether it is « de préparation », its
    /// prerequisites (`kartenn-bench <bench> --list`).
    pub fn list(&self) -> String {
        let mut out = String::new();
        for m in self.measures {
            out.push_str(m.name);
            if m.preparation {
                out.push_str(" (de préparation)");
            }
            if !m.after.is_empty() {
                let _ = write!(out, " (après {})", m.after.join(", "));
            }
            out.push('\n');
        }
        out
    }

    /// Every measure.
    pub fn all(&'static self) -> Selection {
        let every = vec![true; self.measures.len()];
        Selection {
            bench: self,
            chosen: every.clone(),
            runs: every,
        }
    }

    /// The measures `only` names (`m1,m2`), with their prerequisites; every measure
    /// without `only`.
    ///
    /// # Errors
    ///
    /// A name that is no measure of this bench, or no name at all: the message lists the
    /// measures.
    pub fn select(&'static self, only: Option<&str>) -> anyhow::Result<Selection> {
        let Some(only) = only else {
            return Ok(self.all());
        };
        let mut chosen = vec![false; self.measures.len()];
        for name in only.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            let Some(i) = self.position(name) else {
                bail!(
                    "`{name}` is no measure of the bench `{}`; its measures:\n{}",
                    self.name,
                    self.list().trim_end()
                );
            };
            chosen[i] = true;
        }
        if !chosen.contains(&true) {
            bail!(
                "--only names no measure; those of the bench `{}`:\n{}",
                self.name,
                self.list().trim_end()
            );
        }
        // The prerequisites come before their measures (tested): one pass from the end.
        let mut runs = chosen.clone();
        for i in (0..self.measures.len()).rev() {
            if runs[i] {
                for name in self.measures[i].after {
                    if let Some(j) = self.position(name) {
                        runs[j] = true;
                    }
                }
            }
        }
        Ok(Selection {
            bench: self,
            chosen,
            runs,
        })
    }
}

/// The measures of a run.
#[derive(Debug, Clone)]
pub struct Selection {
    bench: &'static Bench,
    /// Asked for.
    chosen: Vec<bool>,
    /// Asked for, or a prerequisite of one that is.
    runs: Vec<bool>,
}

impl Selection {
    /// Whether the measure `name` runs.
    ///
    /// # Panics
    ///
    /// In a debug build, when `name` is no measure of the bench: a bench asks only for
    /// the names of its catalogue.
    pub fn runs(&self, name: &str) -> bool {
        let position = self.bench.position(name);
        debug_assert!(
            position.is_some(),
            "{name} is not in the catalogue of {}",
            self.bench.name
        );
        position.is_some_and(|i| self.runs[i])
    }

    /// Whether a measure whose name starts with `prefix` runs: for a setup shared by
    /// several measures.
    pub fn any(&self, prefix: &str) -> bool {
        self.bench
            .measures
            .iter()
            .zip(&self.runs)
            .any(|(m, runs)| *runs && m.name.starts_with(prefix))
    }

    /// Whether every measure runs.
    pub fn is_all(&self) -> bool {
        self.chosen.iter().all(|c| *c)
    }

    /// The names asked for, as `--only` takes them; `None` for every measure.
    pub fn only(&self) -> Option<String> {
        if self.is_all() {
            return None;
        }
        let names: Vec<&str> = self.names(&self.chosen).collect();
        Some(names.join(","))
    }

    /// The line that follows the scale under `--only`: the measures asked for and their
    /// prerequisites.
    pub fn announce(&self) -> Option<String> {
        let only = self.only()?;
        let before: Vec<&str> = self
            .bench
            .measures
            .iter()
            .zip(self.chosen.iter().zip(&self.runs))
            .filter(|(_, (chosen, runs))| **runs && !**chosen)
            .map(|(m, _)| m.name)
            .collect();
        Some(if before.is_empty() {
            format!("mesures : {}", only.replace(',', ", "))
        } else {
            format!(
                "mesures : {} ; préalables : {}",
                only.replace(',', ", "),
                before.join(", ")
            )
        })
    }

    fn names<'a>(&'a self, which: &'a [bool]) -> impl Iterator<Item = &'static str> + 'a {
        self.bench
            .measures
            .iter()
            .zip(which)
            .filter(|(_, w)| **w)
            .map(|(m, _)| m.name)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn catalogues_are_well_formed() {
        let mut benches = BTreeSet::new();
        for bench in BENCHES {
            assert!(benches.insert(bench.name), "{} twice", bench.name);
            assert!(!bench.measures.is_empty(), "{} has no measure", bench.name);
            let mut seen = BTreeSet::new();
            for m in bench.measures {
                let short = m
                    .name
                    .strip_prefix(bench.name)
                    .and_then(|n| n.strip_prefix('.'))
                    .unwrap_or_else(|| {
                        panic!("{} is not named <{}>.<measure>", m.name, bench.name)
                    });
                assert!(
                    !short.is_empty()
                        && short
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                    "{}: lowercase, digits and dashes",
                    m.name
                );
                for before in m.after {
                    assert!(
                        seen.contains(before),
                        "{}: its prerequisite {before} does not come before it in {}",
                        m.name,
                        bench.name
                    );
                }
                assert!(
                    !m.preparation || m.after.is_empty(),
                    "{}: a measure of the preparation has no prerequisite",
                    m.name
                );
                assert!(seen.insert(m.name), "{} twice", m.name);
            }
        }
    }

    /// The names are those of the history (ADR 0041, decision 4): renaming one is a
    /// decision, not a side effect.
    #[test]
    fn lists_are_stable() {
        let lists: Vec<String> = BENCHES
            .iter()
            .map(|b| format!("[{}]\n{}", b.name, b.list()))
            .collect();
        assert_eq!(lists.join(""), EXPECTED);
    }

    const EXPECTED: &str = "\
[props]
props.origins-every-root (de préparation)
props.explained-resolution
props.grouped-resolution
props.filtered-pages
props.changeset-10k
props.value-every-root (après props.changeset-10k)
props.origins-3-domains (après props.value-every-root)
[core]
core.read-object
core.history-index
core.filtered-page
core.subtree
core.roots-page
core.wide-children
core.subtree-past
core.unitary-changeset
core.sustained
core.import
core.vlan-read
core.vlan-filtered-page
core.vlan-occupancy
core.vlan-create
core.machines-read (après core.unitary-changeset, core.sustained, core.import)
core.machines-owner-page
core.machines-site-page
core.machines-network (après core.wide-children, core.unitary-changeset, core.sustained, core.import)
core.machines-create
core.machines-suggestions (après core.wide-children, core.unitary-changeset, core.sustained, core.import, core.vlan-create, core.machines-create)
core.machines-import
core.stats (après core.wide-children, core.unitary-changeset, core.sustained, core.import, core.vlan-create, core.machines-create, core.machines-import)
[rbac]
rbac.read-object
rbac.read-past (après rbac.read-object)
rbac.filtered-page
rbac.subtree
rbac.unitary-changeset
rbac.resolution
[http]
http.read-object
http.read-past (après http.read-object)
http.filtered-page
http.subtree
http.unitary-changeset
http.labels
http.load
[policy]
policy.snapshot-cold
policy.verdict-pair
policy.interface-pair
policy.raw-port
policy.rules-differential
policy.audit-report (après policy.rules-differential)
policy.inference-suggestion
policy.inference-control
policy.inference-preview (après policy.inference-control)
policy.inference-check (après policy.inference-control)
[flow]
flow.request-interfaces
flow.reclassify
flow.rule-written (après flow.request-interfaces, flow.reclassify)
flow.policy-written (après flow.rule-written)
flow.flows-page (après flow.request-interfaces)
flow.nonconformities-page (après flow.policy-written)
flow.endpoints-flows-page
flow.endpoints-nonconformities-page (après flow.policy-written)
[events]
events.reader-overhead
events.latency
events.memory
events.storm (après events.memory)
events.visibility
[telemetry]
telemetry.read-object
telemetry.filtered-page
telemetry.subtree
telemetry.unitary-changeset
telemetry.sustained
telemetry.audit-volume
";

    #[test]
    fn only_runs_the_measures_and_their_prerequisites() {
        let props = bench("props").unwrap();
        let selection = props.select(Some("props.origins-3-domains")).unwrap();
        assert!(selection.runs("props.origins-3-domains"));
        assert!(selection.runs("props.value-every-root"));
        assert!(selection.runs("props.changeset-10k"), "transitively");
        assert!(!selection.runs("props.explained-resolution"));
        assert!(!selection.is_all());
        assert_eq!(selection.only().as_deref(), Some("props.origins-3-domains"));
        assert_eq!(
            selection.announce().as_deref(),
            Some(
                "mesures : props.origins-3-domains ; préalables : props.changeset-10k, \
                 props.value-every-root"
            )
        );
        let all = props.select(None).unwrap();
        assert!(all.is_all() && all.only().is_none() && all.announce().is_none());
        let core = bench("core")
            .unwrap()
            .select(Some("core.vlan-read"))
            .unwrap();
        assert!(core.any("core.vlan-") && !core.any("core.machines-"));
    }

    #[test]
    fn an_unknown_name_is_refused_with_the_list() {
        let props = bench("props").unwrap();
        for only in [
            "props.nope",
            "core.import",
            "props.explained-resolution,nope",
            ",",
            "",
        ] {
            let error = props.select(Some(only)).unwrap_err().to_string();
            assert!(
                error.contains(&props.list().trim_end().to_string()),
                "{only}: {error}"
            );
        }
        assert!(bench("stats").is_none());
    }
}
