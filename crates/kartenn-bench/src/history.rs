//! The local history of the measures (ADR 0041, decision 4; task 028-3).
//!
//! Every verdict of a run is a [`Figure`]: its measure, its budget, a value, a unit, a
//! limit when it has one, and a verdict. With `--history`, a run adds one JSON line per
//! figure to `.bench/history.jsonl` at the root of the main checkout, the parent of
//! `git rev-parse --git-common-dir`: every worktree writes to the same file. The file is
//! kept out of git by the entry `/.bench/` of `.git/info/exclude`, common to the worktrees,
//! added when it is missing. A line carries what the bisection reads (task 028-5): the
//! commit and whether the tree was modified, the bench, the measure, the scale, the seed,
//! the value, its unit and limit, the verdict, the load of the machine at the start, and
//! the fingerprint of the template the run measured on, if any. No path, no user name.
//!
//! Two worktrees may write at the same moment: the lines of a run are written by a single
//! write, in append mode, under an exclusive lock of the file; a line is never interleaved
//! with another nor cut.

use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, bail};
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// The history, from the root of the main checkout.
pub const FILE: &str = ".bench/history.jsonl";
/// The entry of `.git/info/exclude` that keeps the history out of git.
pub const EXCLUDED: &str = "/.bench/";

/// The unit of a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// A duration.
    Seconds,
    /// An overhead.
    Percent,
    /// A size.
    Bytes,
    /// A rate.
    PerSecond,
    /// A number of things (events lost, errors).
    Count,
}

impl Unit {
    /// Its name in the history.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Seconds => "s",
            Self::Percent => "%",
            Self::Bytes => "B",
            Self::PerSecond => "/s",
            Self::Count => "count",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [
            Self::Seconds,
            Self::Percent,
            Self::Bytes,
            Self::PerSecond,
            Self::Count,
        ]
        .into_iter()
        .find(|u| u.as_str() == s)
    }

    /// `value` for a reader: a duration as the report prints it, else the number and the
    /// unit.
    fn show(self, value: f64) -> String {
        match self {
            _ if !value.is_finite() => "?".into(),
            Self::Seconds => format!("{:.2?}", Duration::from_secs_f64(value.max(0.0))),
            Self::Percent => format!("{value:.2} %"),
            Self::Bytes => format!("{value:.0} B"),
            Self::PerSecond => format!("{value:.1}/s"),
            Self::Count => format!("{value:.0}"),
        }
    }
}

/// The verdict of a figure: a budget held or exceeded, or a figure reported without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Info,
}

impl Verdict {
    /// Its name in the history.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Info => "info",
        }
    }

    const fn of(pass: bool) -> Self {
        if pass { Self::Pass } else { Self::Fail }
    }

    fn parse(s: &str) -> Option<Self> {
        [Self::Pass, Self::Fail, Self::Info]
            .into_iter()
            .find(|v| v.as_str() == s)
    }
}

/// What a run measured for one budget of a measure: a `PASS`, `FAIL` or `INFO` line of
/// its report, and a line of the history.
#[derive(Debug, Clone, PartialEq)]
pub struct Figure {
    /// `<bench>.<measure>`.
    pub measure: &'static str,
    /// The budget, as the report names it.
    pub budget: String,
    pub value: f64,
    pub unit: Unit,
    /// The limit of the budget, in `unit`; none for a figure without one.
    pub limit: Option<f64>,
    pub verdict: Verdict,
}

impl Figure {
    /// A duration against its limit: passes when it does not exceed it.
    pub(crate) fn time(
        measure: &'static str,
        budget: impl Into<String>,
        measured: Duration,
        limit: Duration,
    ) -> Self {
        Self::check(
            measure,
            budget,
            measured.as_secs_f64(),
            Unit::Seconds,
            Some(limit.as_secs_f64()),
            measured <= limit,
        )
    }

    /// A value whose verdict is `pass`, against `limit` if it has one.
    pub(crate) fn check(
        measure: &'static str,
        budget: impl Into<String>,
        value: f64,
        unit: Unit,
        limit: Option<f64>,
        pass: bool,
    ) -> Self {
        Self {
            measure,
            budget: budget.into(),
            value,
            unit,
            limit,
            verdict: Verdict::of(pass),
        }
    }

    /// A value reported without a budget.
    pub(crate) fn info(
        measure: &'static str,
        budget: impl Into<String>,
        value: f64,
        unit: Unit,
    ) -> Self {
        Self {
            measure,
            budget: budget.into(),
            value,
            unit,
            limit: None,
            verdict: Verdict::Info,
        }
    }

    /// The budget is exceeded.
    pub fn fails(&self) -> bool {
        self.verdict == Verdict::Fail
    }
}

/// What every line of a run shares.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// When the run started, UTC (RFC 3339).
    pub date: String,
    /// The commit measured, and whether its tree was modified.
    pub commit: String,
    pub dirty: bool,
    pub bench: String,
    /// `full`, `reduced` or `small`.
    pub scale: String,
    pub seed: u64,
    /// The load average of the machine over a minute when the run started.
    pub load: Option<f64>,
    /// The fingerprint of the template of the run (task 028-2), with `--template`.
    pub template: Option<String>,
}

impl Run {
    /// The run that starts now, in the checkout of `dir`.
    ///
    /// # Errors
    ///
    /// `git` fails in `dir`.
    pub fn start(
        dir: &Path,
        bench: &str,
        scale: &str,
        seed: u64,
        load: Option<f64>,
        template: Option<&str>,
    ) -> anyhow::Result<Self> {
        let (commit, dirty) = revision(dir)?;
        Ok(Self {
            date: now(),
            commit,
            dirty,
            bench: bench.to_owned(),
            scale: scale.to_owned(),
            seed,
            load,
            template: template.map(str::to_owned),
        })
    }
}

/// A line of the history.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub run: Run,
    pub measure: String,
    pub budget: String,
    pub value: f64,
    pub unit: Unit,
    pub limit: Option<f64>,
    pub verdict: Verdict,
}

impl Entry {
    pub fn of(run: &Run, figure: &Figure) -> Self {
        Self {
            run: run.clone(),
            measure: figure.measure.to_owned(),
            budget: figure.budget.clone(),
            value: figure.value,
            unit: figure.unit,
            limit: figure.limit,
            verdict: figure.verdict,
        }
    }

    /// The JSON line, without its end of line, its fields in this order. A value that is
    /// not a number is `null`.
    pub fn line(&self) -> String {
        let fields = [
            ("date", json!(self.run.date)),
            ("commit", json!(self.run.commit)),
            ("dirty", json!(self.run.dirty)),
            ("bench", json!(self.run.bench)),
            ("measure", json!(self.measure)),
            ("budget", json!(self.budget)),
            ("scale", json!(self.run.scale)),
            ("seed", json!(self.run.seed)),
            ("value", json!(self.value)),
            ("unit", json!(self.unit.as_str())),
            ("limit", json!(self.limit)),
            ("verdict", json!(self.verdict.as_str())),
            ("load", json!(self.run.load)),
            ("template", json!(self.run.template)),
        ];
        let mut line = String::from("{");
        for (i, (key, value)) in fields.iter().enumerate() {
            let comma = if i == 0 { "" } else { "," };
            let _ = write!(line, "{comma}\"{key}\":{value}");
        }
        line.push('}');
        line
    }

    /// A line of the history read back.
    ///
    /// # Errors
    ///
    /// Not a JSON object, or a field missing or of another type.
    pub fn parse(line: &str) -> anyhow::Result<Self> {
        let v: Value = serde_json::from_str(line)?;
        let text = |k: &str| -> anyhow::Result<String> {
            v.get(k)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .with_context(|| format!("`{k}`: a string"))
        };
        // `null` for a value that was not a number.
        let number = |k: &str| -> anyhow::Result<Option<f64>> {
            match v.get(k) {
                Some(Value::Null) => Ok(None),
                Some(n) => n
                    .as_f64()
                    .map(Some)
                    .with_context(|| format!("`{k}`: a number")),
                None => bail!("`{k}` is missing"),
            }
        };
        let unit = text("unit")?;
        let verdict = text("verdict")?;
        let template = match v.get("template") {
            Some(Value::Null) => None,
            _ => Some(text("template")?),
        };
        Ok(Self {
            run: Run {
                date: text("date")?,
                commit: text("commit")?,
                dirty: v
                    .get("dirty")
                    .and_then(Value::as_bool)
                    .context("`dirty`: a boolean")?,
                bench: text("bench")?,
                scale: text("scale")?,
                seed: v
                    .get("seed")
                    .and_then(Value::as_u64)
                    .context("`seed`: an unsigned integer")?,
                load: number("load")?,
                template,
            },
            measure: text("measure")?,
            budget: text("budget")?,
            value: number("value")?.unwrap_or(f64::NAN),
            unit: Unit::parse(&unit).with_context(|| format!("no unit `{unit}`"))?,
            limit: number("limit")?,
            verdict: Verdict::parse(&verdict).with_context(|| format!("no verdict `{verdict}`"))?,
        })
    }
}

/// UTC now, to the second (RFC 3339).
fn now() -> String {
    let now = OffsetDateTime::now_utc();
    now.replace_nanosecond(0)
        .unwrap_or(now)
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// The load average of the machine over a minute, if the system gives it.
pub fn load() -> Option<f64> {
    std::fs::read_to_string("/proc/loadavg")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// `git -C dir <args>`, its standard output trimmed.
fn git(dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .context("running git")?;
    if !output.status.success() {
        bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim_end().to_owned())
}

/// The commit of the checkout of `dir`, and whether its tree is modified (a tracked file
/// changed, or an untracked file that is not ignored).
///
/// # Errors
///
/// `git` fails in `dir`.
pub fn revision(dir: &Path) -> anyhow::Result<(String, bool)> {
    let commit = git(dir, &["rev-parse", "HEAD"])?;
    let dirty = !git(dir, &["status", "--porcelain"])?.is_empty();
    Ok((commit, dirty))
}

/// Where the checkout of `dir` keeps its history: the common git directory of its
/// worktrees, and `.bench/history.jsonl` in its parent, the main checkout.
///
/// # Errors
///
/// `git` fails in `dir`, or the common directory has no parent.
pub fn location(dir: &Path) -> anyhow::Result<(PathBuf, PathBuf)> {
    let common = PathBuf::from(git(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    let root = common
        .parent()
        .with_context(|| format!("{} has no parent", common.display()))?;
    let file = root.join(FILE);
    Ok((common, file))
}

/// Adds [`EXCLUDED`] to `info/exclude` of the common git directory `common` when no line
/// of it is that entry, and nothing else: the rest of the file is kept as it is. Under an
/// exclusive lock of the file, two worktrees add it once. Whether it was added.
///
/// # Errors
///
/// The file cannot be read, locked or written.
pub fn exclude(common: &Path) -> anyhow::Result<bool> {
    let info = common.join("info");
    std::fs::create_dir_all(&info)?;
    let path = info.join("exclude");
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    file.lock()?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    if content.lines().any(|l| l.trim_end() == EXCLUDED) {
        return Ok(false);
    }
    let separator = if content.is_empty() || content.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    file.write_all(format!("{separator}{EXCLUDED}\n").as_bytes())?;
    Ok(true)
}

/// The history of the checkout of `dir`, out of git: its file, [`EXCLUDED`] added to the
/// exclusions when missing.
///
/// # Errors
///
/// `git` fails in `dir`, or the exclusions cannot be written.
pub fn prepare(dir: &Path) -> anyhow::Result<PathBuf> {
    let (common, file) = location(dir)?;
    exclude(&common)?;
    Ok(file)
}

/// Appends the lines of `entries` to `path` (its directory created): one write in append
/// mode, under an exclusive lock of the file.
///
/// # Errors
///
/// The file cannot be opened, locked or written.
pub fn append(path: &Path, entries: &[Entry]) -> anyhow::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut lines = String::new();
    for entry in entries {
        lines.push_str(&entry.line());
        lines.push('\n');
    }
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    file.lock()?;
    file.write_all(lines.as_bytes())?;
    file.flush()?;
    Ok(())
}

/// The lines of the history at `path`, in their order, and how many could not be read;
/// none when the file does not exist.
///
/// # Errors
///
/// The file exists and cannot be read.
pub fn read(path: &Path) -> anyhow::Result<(Vec<Entry>, usize)> {
    let mut text = String::new();
    match File::open(path) {
        Ok(mut file) => {
            file.lock_shared()?;
            file.read_to_string(&mut text)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    }
    let mut entries = Vec::new();
    let mut unread = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match Entry::parse(line) {
            Ok(entry) => entries.push(entry),
            Err(_) => unread += 1,
        }
    }
    Ok((entries, unread))
}

/// The series of `measure` (at `scale` only, if given), oldest first: one row per line,
/// its budget last.
pub fn series(entries: &[Entry], measure: &str, scale: Option<&str>) -> String {
    let rows: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.measure == measure && scale.is_none_or(|s| e.run.scale == s))
        .collect();
    let mut out = String::new();
    if rows.is_empty() {
        let _ = writeln!(
            out,
            "aucun passage de {measure}{}",
            scale
                .map(|s| format!(" à l'échelle {s}"))
                .unwrap_or_default()
        );
        return out;
    }
    let _ = writeln!(
        out,
        "{:<20} {:<13} {:<7} {:>6} {:>12} {:>12} {:<4} {:>5}  budget",
        "date", "commit", "échelle", "graine", "valeur", "seuil", "", "charge"
    );
    for e in rows {
        let commit = format!(
            "{}{}",
            e.run.commit.get(..12).unwrap_or(&e.run.commit),
            if e.run.dirty { "+" } else { "" }
        );
        let _ = writeln!(
            out,
            "{:<20} {:<13} {:<7} {:>6} {:>12} {:>12} {:<4} {:>5}  {}",
            e.run.date,
            commit,
            e.run.scale,
            e.run.seed,
            e.unit.show(e.value),
            e.limit.map_or_else(|| "-".into(), |l| e.unit.show(l)),
            e.verdict.as_str().to_uppercase(),
            e.run.load.map_or_else(|| "?".into(), |l| format!("{l:.2}")),
            e.budget
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> Run {
        Run {
            date: "2026-11-01T10:00:00Z".into(),
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            dirty: true,
            bench: "props".into(),
            scale: "reduced".into(),
            seed: 42,
            load: Some(3.25),
            template: Some("00ff00ff00ff00ff".into()),
        }
    }

    fn figure(budget: &str) -> Figure {
        Figure::time(
            "props.changeset-10k",
            budget,
            Duration::from_millis(410),
            Duration::from_millis(2_500),
        )
    }

    #[test]
    fn a_line_is_written_and_read_back() {
        let entry = Entry::of(&run(), &figure("changeset, differential up to 10^4"));
        let line = entry.line();
        assert!(!line.contains('\n'));
        assert_eq!(
            line,
            concat!(
                r#"{"date":"2026-11-01T10:00:00Z","#,
                r#""commit":"0123456789abcdef0123456789abcdef01234567","dirty":true,"#,
                r#""bench":"props","measure":"props.changeset-10k","#,
                r#""budget":"changeset, differential up to 10^4","scale":"reduced","seed":42,"#,
                r#""value":0.41,"unit":"s","limit":2.5,"verdict":"pass","load":3.25,"#,
                r#""template":"00ff00ff00ff00ff"}"#
            )
        );
        assert_eq!(Entry::parse(&line).unwrap(), entry);

        // Without template, load nor limit; a value that is not a number.
        let mut bare = run();
        bare.template = None;
        bare.load = None;
        let info = Figure::info("telemetry.audit-volume", "audit", f64::NAN, Unit::Bytes);
        let read = Entry::parse(&Entry::of(&bare, &info).line()).unwrap();
        assert!(read.value.is_nan() && read.limit.is_none() && read.run.template.is_none());
        assert_eq!((read.unit, read.verdict), (Unit::Bytes, Verdict::Info));

        assert!(Entry::parse(r#"{"date":"x"}"#).is_err());
        assert!(Entry::parse("not json").is_err());
    }

    #[test]
    fn units_and_verdicts_round_trip() {
        for unit in [
            Unit::Seconds,
            Unit::Percent,
            Unit::Bytes,
            Unit::PerSecond,
            Unit::Count,
        ] {
            assert_eq!(Unit::parse(unit.as_str()), Some(unit));
        }
        for verdict in [Verdict::Pass, Verdict::Fail, Verdict::Info] {
            assert_eq!(Verdict::parse(verdict.as_str()), Some(verdict));
        }
        let over = Figure::time("m", "b", Duration::from_secs(2), Duration::from_secs(1));
        assert!(over.fails());
        assert!(!figure("b").fails());
    }

    #[test]
    fn the_series_of_a_measure() {
        let mut full = run();
        full.scale = "full".into();
        full.dirty = false;
        let entries = vec![
            Entry::of(&run(), &figure("changeset")),
            Entry::of(&full, &figure("changeset")),
            Entry::of(
                &run(),
                &Figure::time(
                    "props.grouped-resolution",
                    "grouped",
                    Duration::ZERO,
                    Duration::ZERO,
                ),
            ),
        ];
        let all = series(&entries, "props.changeset-10k", None);
        assert_eq!(all.lines().count(), 3, "{all}");
        assert!(all.contains("0123456789ab+ reduced"), "{all}");
        assert!(all.contains("0123456789ab  full"), "{all}");
        assert!(all.contains("410.00ms"), "{all}");
        assert!(all.contains("2.50s"), "{all}");
        assert!(
            all.lines()
                .nth(1)
                .unwrap()
                .ends_with("PASS  3.25  changeset"),
            "{all}"
        );
        let full = series(&entries, "props.changeset-10k", Some("full"));
        assert_eq!(full.lines().count(), 2, "{full}");
        assert_eq!(
            series(&entries, "props.changeset-10k", Some("small")),
            "aucun passage de props.changeset-10k à l'échelle small\n"
        );
    }

    /// A scratch directory of the test, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "kartenn-bench-history-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn git_ok(dir: &Path, args: &[&str]) -> String {
        git(dir, args).unwrap_or_else(|e| panic!("{e}"))
    }

    /// A repository with one commit at `scratch/main`, and a worktree at `scratch/other`.
    fn repository(scratch: &Scratch) -> (PathBuf, PathBuf) {
        let main = scratch.0.join("main");
        std::fs::create_dir_all(&main).unwrap();
        git_ok(&main, &["init", "-q", "-b", "main"]);
        std::fs::write(main.join("README"), "kartenn\n").unwrap();
        git_ok(&main, &["add", "README"]);
        git_ok(
            &main,
            &[
                "-c",
                "user.name=bench",
                "-c",
                "user.email=bench@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                "init",
            ],
        );
        let other = scratch.0.join("other");
        git_ok(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "other",
                other.to_str().unwrap(),
            ],
        );
        (main, other)
    }

    /// Two worktrees write to the same file, at the same moment, lines longer than a page:
    /// every line is whole; the history and the exclusions leave `git status` clean in
    /// both, and the exclusions keep what they held.
    #[test]
    fn two_worktrees_write_to_the_same_history() {
        let scratch = Scratch::new("worktrees");
        let (main, other) = repository(&scratch);
        let common = main.join(".git");
        std::fs::write(common.join("info/exclude"), "# kept\n*.local").unwrap();

        let here = prepare(&main).unwrap();
        let there = prepare(&other).unwrap();
        assert_eq!(here, there);
        assert_eq!(here, main.canonicalize().unwrap().join(FILE));
        assert_eq!(
            std::fs::read_to_string(common.join("info/exclude")).unwrap(),
            "# kept\n*.local\n/.bench/\n"
        );
        assert!(!exclude(&common).unwrap());

        let (commit, dirty) = revision(&other).unwrap();
        assert_eq!(commit, git_ok(&main, &["rev-parse", "HEAD"]));
        assert!(!dirty);

        let long = "x".repeat(10_000);
        let writers: Vec<_> = [main.clone(), other.clone()]
            .into_iter()
            .enumerate()
            .flat_map(|(w, dir)| {
                let long = long.clone();
                (0..4).map(move |t| {
                    let (dir, long) = (dir.clone(), long.clone());
                    std::thread::spawn(move || {
                        let file = prepare(&dir).unwrap();
                        let mut run = run();
                        run.bench = format!("w{w}t{t}");
                        for i in 0..25 {
                            let entries: Vec<Entry> = (0..3)
                                .map(|k| Entry::of(&run, &figure(&format!("{i}.{k} {long}"))))
                                .collect();
                            append(&file, &entries).unwrap();
                        }
                    })
                })
            })
            .collect();
        for w in writers {
            w.join().unwrap();
        }
        let (entries, unread) = read(&here).unwrap();
        assert_eq!(unread, 0);
        assert_eq!(entries.len(), 2 * 4 * 25 * 3);
        // The lines of one append stay together, in their order.
        for chunk in entries.chunks(3) {
            let i = chunk[0].budget.split('.').next().unwrap();
            for (k, e) in chunk.iter().enumerate() {
                assert_eq!(e.run.bench, chunk[0].run.bench);
                assert!(
                    e.budget.starts_with(&format!("{i}.{k} ")),
                    "{}",
                    &e.budget[..8]
                );
                assert!(e.budget.ends_with(&long));
            }
        }

        for dir in [&main, &other] {
            assert_eq!(
                git_ok(dir, &["status", "--porcelain"]),
                "",
                "{}",
                dir.display()
            );
        }
        assert!(!revision(&main).unwrap().1);
    }

    #[test]
    fn exclusions_are_created_or_completed_once() {
        let scratch = Scratch::new("exclude");
        let common = scratch.0.join("git");
        assert!(exclude(&common).unwrap());
        assert!(!exclude(&common).unwrap());
        assert_eq!(
            std::fs::read_to_string(common.join("info/exclude")).unwrap(),
            "/.bench/\n"
        );
        std::fs::write(common.join("info/exclude"), "a\n/.bench/  \nb\n").unwrap();
        assert!(!exclude(&common).unwrap());
        assert_eq!(
            std::fs::read_to_string(common.join("info/exclude")).unwrap(),
            "a\n/.bench/  \nb\n"
        );
    }

    #[test]
    fn a_missing_history_reads_empty_and_bad_lines_are_counted() {
        let scratch = Scratch::new("read");
        let path = scratch.0.join(FILE);
        assert_eq!(read(&path).unwrap(), (Vec::new(), 0));
        append(&path, &[Entry::of(&run(), &figure("b"))]).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"truncated\n\n")
            .unwrap();
        let (entries, unread) = read(&path).unwrap();
        assert_eq!((entries.len(), unread), (1, 1));
    }
}
