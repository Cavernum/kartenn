//! The bisection of a measure (ADR 0041, decision 5; task 028-5): what `bench-bisect` asks
//! of the histories.
//!
//! The script runs `git bisect run` in a worktree of its own. At the good commit, at the
//! bad one and at each step, [`RUNS`] runs of the measure alone add their lines to the
//! history of the bisection (`--history=<file>`, never the history of the checkout). A
//! commit is judged on the median of its runs for one budget of the measure, the one
//! [`track`] follows, against the middle between the medians of the good and bad commits.
//! The verdict is read from those lines, never from the exit code of a run: a run exits
//! with 1 both when a budget is exceeded and on an error.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::bail;

use crate::history::{Entry, Unit, Verdict};

/// Runs of the measure at each commit; the criterion is their median.
pub const RUNS: usize = 3;

/// The least gap between the medians of the good and bad commits, relative to the good one,
/// worth bisecting: below it, the two are within the noise of a run.
pub const LEAST_GAP: f64 = 0.10;

/// Whether `ancestor` is an ancestor of `of` (or `of` itself) in the repository of `dir`;
/// not when git does not know it (a commit of a branch since removed).
pub fn is_ancestor(dir: &Path, ancestor: &str, of: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["merge-base", "--is-ancestor", ancestor, of])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The good commit by default: that of the newest run of `entries` where `measure` held at
/// `scale`, on a clean tree, an ancestor of `bad` other than `bad`. A run held when none of
/// its lines for the measure failed, or that of `budget` only when it is given.
pub fn last_good(
    entries: &[Entry],
    measure: &str,
    scale: &str,
    budget: Option<&str>,
    bad: &str,
    is_ancestor: impl Fn(&str) -> bool,
) -> Option<String> {
    // The runs in the order of the history, each with whether it held.
    let mut runs: Vec<(&str, bool)> = Vec::new();
    let mut index: HashMap<(&str, &str), usize> = HashMap::new();
    for e in entries.iter().filter(|e| {
        e.measure == measure
            && e.run.scale == scale
            && !e.run.dirty
            && budget.is_none_or(|b| e.budget == b)
    }) {
        let failed = e.verdict == Verdict::Fail;
        let key = (e.run.date.as_str(), e.run.commit.as_str());
        if let Some(&i) = index.get(&key) {
            runs[i].1 &= !failed;
        } else {
            index.insert(key, runs.len());
            runs.push((&e.run.commit, !failed));
        }
    }
    runs.iter()
        .rev()
        .filter(|(commit, held)| *held && *commit != bad)
        .map(|(commit, _)| *commit)
        .find(|commit| is_ancestor(commit))
        .map(str::to_owned)
}

/// The figures of a budget at a commit: the median of its last [`RUNS`] values.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub median: f64,
    pub unit: Unit,
    /// Values the median is taken over, and how many of them failed their budget.
    pub runs: usize,
    pub fails: usize,
}

/// The samples of each budget of `measure` at `commit`, by budget.
pub fn samples(entries: &[Entry], measure: &str, commit: &str) -> BTreeMap<String, Sample> {
    let mut lines: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for e in entries
        .iter()
        .filter(|e| e.measure == measure && e.run.commit == commit && e.value.is_finite())
    {
        lines.entry(&e.budget).or_default().push(e);
    }
    lines
        .into_iter()
        .map(|(budget, all)| {
            let last = &all[all.len().saturating_sub(RUNS)..];
            let mut values: Vec<f64> = last.iter().map(|e| e.value).collect();
            values.sort_by(f64::total_cmp);
            let n = values.len();
            let median = if n % 2 == 1 {
                values[n / 2]
            } else {
                f64::midpoint(values[n / 2 - 1], values[n / 2])
            };
            let sample = Sample {
                median,
                unit: last[0].unit,
                runs: n,
                fails: last.iter().filter(|e| e.verdict == Verdict::Fail).count(),
            };
            (budget.to_owned(), sample)
        })
        .collect()
}

/// Why a budget is the one followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// Named by `--budget`.
    Asked,
    /// Exceeded at the bad commit (by most of its runs), with the largest gap if several are.
    Exceeded,
    /// None is exceeded at the bad commit (a drift): the largest gap.
    Gap,
}

/// The budget a bisection follows, and its medians at both ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Tracked {
    pub budget: String,
    pub unit: Unit,
    pub good: f64,
    pub bad: f64,
    pub why: Why,
}

/// The gap between `good` and `bad`, relative to `good`.
fn gap(good: f64, bad: f64) -> f64 {
    if good == 0.0 {
        if bad == 0.0 { 0.0 } else { f64::INFINITY }
    } else {
        (bad - good).abs() / good.abs()
    }
}

impl Tracked {
    /// The middle between the good and bad medians.
    pub fn threshold(&self) -> f64 {
        f64::midpoint(self.good, self.bad)
    }

    /// A median on the side of the bad one, the threshold included.
    pub fn is_bad(&self, median: f64) -> bool {
        if self.bad > self.good {
            median >= self.threshold()
        } else {
            median <= self.threshold()
        }
    }

    /// One line for the reader.
    pub fn describe(&self) -> String {
        let why = match self.why {
            Why::Asked => "demandé",
            Why::Exceeded => "dépassé au mauvais commit",
            Why::Gap => "aucun budget dépassé au mauvais commit : le plus grand écart",
        };
        format!(
            "budget suivi « {} » ({why}) : bon {}, mauvais {}, seuil {}",
            self.budget,
            self.unit.show(self.good),
            self.unit.show(self.bad),
            self.unit.show(self.threshold()),
        )
    }
}

/// The budget to follow from the samples of the good and bad commits: `budget` when given;
/// otherwise among those the bad commit exceeds, or all of them when it exceeds none, the
/// one whose medians differ the most, relative to the good one.
///
/// # Errors
///
/// No budget at both commits, `budget` not one of them, or a gap below [`LEAST_GAP`].
pub fn track(
    good: &BTreeMap<String, Sample>,
    bad: &BTreeMap<String, Sample>,
    budget: Option<&str>,
) -> anyhow::Result<Tracked> {
    let both: Vec<(&String, &Sample, &Sample)> = good
        .iter()
        .filter_map(|(name, g)| bad.get(name).map(|b| (name, g, b)))
        .collect();
    if both.is_empty() {
        bail!("no figure of the measure at both the good and the bad commit");
    }
    let chosen = if let Some(asked) = budget {
        let Some(&found) = both.iter().find(|(name, ..)| name.as_str() == asked) else {
            let mut names = String::new();
            for (name, ..) in &both {
                let _ = write!(names, "\n  {name}");
            }
            bail!("no budget `{asked}` at both commits; the budgets of the measure:{names}");
        };
        (found, Why::Asked)
    } else {
        let exceeded: Vec<_> = both
            .iter()
            .copied()
            .filter(|(_, _, b)| b.fails * 2 > b.runs)
            .collect();
        let (pool, why) = if exceeded.is_empty() {
            (both, Why::Gap)
        } else {
            (exceeded, Why::Exceeded)
        };
        let widest = pool
            .into_iter()
            .reduce(|a, b| {
                if gap(b.1.median, b.2.median) > gap(a.1.median, a.2.median) {
                    b
                } else {
                    a
                }
            })
            .unwrap_or_else(|| unreachable!("a non-empty pool"));
        (widest, why)
    };
    let ((name, g, b), why) = chosen;
    let tracked = Tracked {
        budget: name.clone(),
        unit: b.unit,
        good: g.median,
        bad: b.median,
        why,
    };
    let relative = gap(g.median, b.median);
    if relative < LEAST_GAP {
        bail!(
            "the good and the bad commits are {:.1} % apart on `{name}`, within the noise \
             (at least {:.0} %): nothing to bisect ({})",
            relative * 100.0,
            LEAST_GAP * 100.0,
            tracked.describe()
        );
    }
    Ok(tracked)
}

/// The verdict of a step of `git bisect run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Good,
    Bad,
    /// No figure of the budget followed at the commit (the measure failed or does not
    /// exist there): skipped, as a commit that does not compile.
    Skip,
}

impl Step {
    /// Its word for the script.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Bad => "bad",
            Self::Skip => "skip",
        }
    }
}

/// The verdict of a commit from its samples, and the median it was taken on.
pub fn judge(tracked: &Tracked, samples: &BTreeMap<String, Sample>) -> (Step, Option<f64>) {
    match samples.get(&tracked.budget) {
        None => (Step::Skip, None),
        Some(s) if tracked.is_bad(s.median) => (Step::Bad, Some(s.median)),
        Some(s) => (Step::Good, Some(s.median)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::Run;

    const GOOD: &str = "1111111111111111111111111111111111111111";
    const BAD: &str = "2222222222222222222222222222222222222222";

    fn entry(
        date: &str,
        commit: &str,
        dirty: bool,
        budget: &str,
        value: f64,
        verdict: Verdict,
    ) -> Entry {
        Entry {
            run: Run {
                date: date.into(),
                commit: commit.into(),
                dirty,
                bench: "core".into(),
                scale: "reduced".into(),
                seed: 42,
                load: None,
                template: None,
            },
            measure: "core.read-object".into(),
            budget: budget.into(),
            value,
            unit: Unit::Seconds,
            limit: Some(0.005),
            verdict,
        }
    }

    fn sample(median: f64, runs: usize, fails: usize) -> Sample {
        Sample {
            median,
            unit: Unit::Seconds,
            runs,
            fails,
        }
    }

    #[test]
    fn the_good_commit_is_the_newest_clean_run_that_held() {
        use Verdict::{Fail, Pass};
        let old = "0000000000000000000000000000000000000000";
        let gone = "3333333333333333333333333333333333333333";
        let mut entries = vec![
            entry("2026-11-01T08:00:00Z", old, false, "current", 0.001, Pass),
            entry("2026-11-01T08:00:00Z", old, false, "past", 0.002, Pass),
            entry("2026-11-01T09:00:00Z", GOOD, false, "current", 0.001, Pass),
            entry("2026-11-01T09:00:00Z", GOOD, false, "past", 0.002, Pass),
            // Held, but on a modified tree.
            entry("2026-11-01T10:00:00Z", GOOD, true, "current", 0.001, Pass),
            // One budget exceeded: the run did not hold.
            entry("2026-11-01T11:00:00Z", GOOD, false, "current", 0.001, Pass),
            entry("2026-11-01T11:00:00Z", GOOD, false, "past", 0.009, Fail),
            // Not an ancestor of the bad commit.
            entry("2026-11-01T12:00:00Z", gone, false, "current", 0.001, Pass),
            // The bad commit itself.
            entry("2026-11-01T13:00:00Z", BAD, false, "current", 0.001, Pass),
        ];
        let ancestor = |c: &str| c != gone;
        let good = |entries: &[Entry], budget| {
            last_good(
                entries,
                "core.read-object",
                "reduced",
                budget,
                BAD,
                ancestor,
            )
        };
        assert_eq!(good(&entries, None).as_deref(), Some(GOOD));
        // Following `current` only, the run of 11:00 held.
        assert_eq!(good(&entries, Some("current")).as_deref(), Some(GOOD));
        // Another scale or measure: none.
        assert_eq!(
            last_good(&entries, "core.read-object", "full", None, BAD, ancestor),
            None
        );
        assert_eq!(
            last_good(&entries, "core.stats", "reduced", None, BAD, ancestor),
            None
        );
        // The run of 09:00 removed: the one before.
        entries.drain(2..4);
        assert_eq!(good(&entries, None).as_deref(), Some(old));
        assert_eq!(good(&entries, Some("past")).as_deref(), Some(old));
    }

    #[test]
    fn a_sample_is_the_median_of_the_last_runs() {
        use Verdict::{Fail, Pass};
        let entries = [
            entry("1", GOOD, false, "past", 0.100, Fail),
            entry("2", GOOD, false, "past", 0.003, Pass),
            entry("3", GOOD, false, "past", 0.009, Fail),
            entry("4", GOOD, false, "past", 0.002, Pass),
            entry("4", GOOD, false, "current", 0.001, Pass),
            entry("5", GOOD, false, "current", f64::NAN, Pass),
            entry("5", GOOD, false, "current", 0.004, Pass),
            entry("6", BAD, false, "past", 0.050, Fail),
        ];
        let s = samples(&entries, "core.read-object", GOOD);
        assert_eq!(s.keys().collect::<Vec<_>>(), ["current", "past"]);
        assert_eq!(s["past"], sample(0.003, 3, 1));
        // Two values: their mean; a value that is not a number is left out.
        assert!((s["current"].median - 0.0025).abs() < 1e-12);
        assert_eq!(s["current"].runs, 2);
        assert!(samples(&entries, "core.stats", GOOD).is_empty());
    }

    fn of(pairs: &[(&str, Sample)]) -> BTreeMap<String, Sample> {
        pairs
            .iter()
            .map(|(n, s)| ((*n).to_owned(), s.clone()))
            .collect()
    }

    #[test]
    fn the_budget_followed_is_the_exceeded_one_or_the_widest_gap() {
        let good = of(&[
            ("current", sample(0.001, 3, 0)),
            ("past", sample(0.002, 3, 0)),
            ("rate", sample(100.0, 3, 0)),
        ]);
        // `past` exceeded by two runs of three; `current` moved more but holds.
        let bad = of(&[
            ("current", sample(0.010, 3, 0)),
            ("past", sample(0.009, 3, 2)),
            ("rate", sample(100.0, 3, 0)),
        ]);
        let t = track(&good, &bad, None).unwrap();
        assert_eq!((t.budget.as_str(), t.why), ("past", Why::Exceeded));
        assert!((t.threshold() - 0.0055).abs() < 1e-12);
        // Exceeded by one run of three only: not exceeded, the widest gap.
        let drift = of(&[
            ("current", sample(0.004, 3, 0)),
            ("past", sample(0.003, 3, 1)),
            ("rate", sample(100.0, 3, 0)),
        ]);
        let t = track(&good, &drift, None).unwrap();
        assert_eq!((t.budget.as_str(), t.why), ("current", Why::Gap));
        // Asked for.
        let t = track(&good, &bad, Some("current")).unwrap();
        assert_eq!((t.budget.as_str(), t.why), ("current", Why::Asked));
        // Unknown, listed with the budgets.
        let e = track(&good, &bad, Some("nope")).unwrap_err().to_string();
        assert!(
            e.contains("`nope`") && e.contains("\n  current\n  past\n  rate"),
            "{e}"
        );
        // Within the noise.
        let e = track(&good, &bad, Some("rate")).unwrap_err().to_string();
        assert!(e.contains("nothing to bisect"), "{e}");
        // Nothing in common.
        assert!(track(&good, &BTreeMap::new(), None).is_err());
    }

    #[test]
    fn a_step_is_judged_against_the_middle() {
        let slower = Tracked {
            budget: "past".into(),
            unit: Unit::Seconds,
            good: 0.002,
            bad: 0.010,
            why: Why::Exceeded,
        };
        let at = |median| of(&[("past", sample(median, 3, 0))]);
        assert_eq!(judge(&slower, &at(0.0059)), (Step::Good, Some(0.0059)));
        assert_eq!(judge(&slower, &at(0.006)), (Step::Bad, Some(0.006)));
        assert_eq!(judge(&slower, &at(0.012)).0, Step::Bad);
        assert_eq!(
            judge(&slower, &of(&[("current", sample(0.1, 3, 0))])),
            (Step::Skip, None)
        );
        // A rate that falls: bad below the middle.
        let fewer = Tracked {
            budget: "past".into(),
            unit: Unit::PerSecond,
            good: 100.0,
            bad: 40.0,
            why: Why::Exceeded,
        };
        assert_eq!(judge(&fewer, &at(71.0)).0, Step::Good);
        assert_eq!(judge(&fewer, &at(70.0)).0, Step::Bad);
        assert!(
            fewer.describe().contains("seuil 70.0/s"),
            "{}",
            fewer.describe()
        );
        // A count that was nought.
        assert!(gap(0.0, 0.0).abs() < f64::EPSILON);
        assert!(gap(0.0, 3.0).is_infinite());
    }
}
