//! Scale (ADR 0032): `small` as last argument is the dataset of the tests; otherwise
//! `K_BENCH_SCALE=reduced` (the default, about a tenth of the volumes) or `full` (the
//! reference dataset). The first line of the output names the scale; below `full`, the
//! last one says that the verdicts are not a budget measure. An implementation agent runs
//! the reduced scale only; the full measure that ticks a budget task is run once, by the
//! orchestrator, at the end of a series.
//!
//! `kartenn-bench gen <seed> <output-dir> [small]`: writes the synthetic dataset as
//! PostgreSQL `COPY` text files (`domains.tsv`, `prefixes.tsv`, `ranges.tsv`,
//! `addresses.tsv`) and prints counts and timing.
//!
//! `kartenn-bench props <database-url> <seed> [small]`: loads the dataset into an empty
//! database and checks the budgets of the properties (REQ-003-26); exits with an error if
//! one is exceeded.
//!
//! `kartenn-bench core <database-url> <seed> [small]`: the same for the budgets of the
//! core (REQ-002-45, REQ-004-38).
//!
//! `kartenn-bench flow <database-url> <seed> [small]`: the budgets of the flow registry
//! (REQ-011-19) with the four modules, a policy of 10⁴ rules and 10⁵ flows.
//!
//! `kartenn-bench rbac <database-url> <seed> [small]`: those budgets with access control,
//! for a person with 50 groups of the issuer and 1 000 bindings through 200 groups of
//! kartenn, and its overhead (REQ-005-29, REQ-025-15).
//!
//! `kartenn-bench http <database-url> <seed> [small]`: those budgets through the router
//! on a local port, with their overhead, then a load of 200 users and 50 writes per
//! second (REQ-006-39).
//!
//! `kartenn-bench telemetry <database-url> <seed> [small]`: the overhead of the telemetry
//! on those budgets, in three configurations (REQ-012-23), and the volume of the audit
//! of reads (REQ-012-18); each configuration runs in a child (`telemetry-run`).
//!
//! `kartenn-bench policy <database-url> <seed> [small]`: the budgets of the flow policy
//! module (REQ-010-28, 54) on the dataset of `core`, the VLAN and machines modules deployed.
//!
//! `kartenn-bench events <database-url> <seed> [small]`: the budgets of the event stream
//! (REQ-022-26): latency at 50 changesets per second, memory per stream, a storm of
//! resumptions, the overhead of the reader, the visibility of a batch.
//!
//! `kartenn-bench seed <database-url> <seed> [small]`: loads the dataset of `core`, and a
//! domain whose container has 10⁵ networks, into a database migrated by
//! `kartenn-server migrate`, for the budgets of the browser (`bench-web`, REQ-007-32);
//! prints `domain=<id> container=<id>`.
//!
//! Named measures (ADR 0041): `kartenn-bench <bench> --list` prints the measures of a bench
//! (`<bench>.<measure>`, those « de préparation », their prerequisites);
//! `kartenn-bench <bench> <database-url> <seed> [small] --only <m1,m2>` runs the preparation,
//! then those measures and their prerequisites only.
//!
//! Prepared database (ADR 0041, decision 2): with `--template`, which the `bench-*` scripts
//! pass on the bench instance, the prepared database becomes the template
//! `kartenn_bench_tpl_<bench>_<fingerprint>` and the run measures on a copy of it; the next
//! run with the same fingerprint copies it instead of preparing ([`kartenn_bench::template`]).
//!
//! History (ADR 0041, decision 4): with `--history`, which the `bench-*` scripts pass, the
//! run adds one JSON line per figure (budget of a measure) to `.bench/history.jsonl` at the
//! root of the main checkout of the current directory; `--history=<file>` writes to `file`
//! instead. `kartenn-bench history <measure> [--scale full|reduced|small]` prints the series
//! of a measure ([`kartenn_bench::history`]).
//!
//! Measures touched (ADR 0041, decision 3): `kartenn-bench affected` reads changed paths,
//! one per line, and prints the benches and measures they call for, one line per bench
//! (`<script> [<m1,m2>]`), for the script `bench-affected` ([`kartenn_bench::affected`]).

use std::fs::File;
use std::io::{BufWriter, Read as _};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, bail};
use kartenn_bench::history::{self, Entry, Figure, Run};
use kartenn_bench::measures::{Bench, Selection, bench};
use kartenn_bench::template::Reuse;
use kartenn_bench::{Scale, generate};

/// What follows the seed: `small`, `--only <m1,m2>` (or `--only=<m1,m2>`) and `--template`,
/// in any order.
#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    small: bool,
    only: Option<String>,
    /// Keep the prepared database as a template and measure on a copy (ADR 0041).
    template: bool,
    /// Add the figures of the run to a history (`--history[=<file>]`).
    history: Option<HistoryFile>,
}

/// The history a run writes to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum HistoryFile {
    /// `.bench/history.jsonl` at the root of the main checkout (`--history`).
    Checkout,
    /// Another file (`--history=<file>`: tests, a bisection).
    At(PathBuf),
}

/// Where a run adds its lines of history, and what they share.
struct Journal {
    file: PathBuf,
    run: Run,
}

impl Journal {
    fn write(&self, figures: &[Figure]) -> anyhow::Result<()> {
        let entries: Vec<Entry> = figures.iter().map(|f| Entry::of(&self.run, f)).collect();
        history::append(&self.file, &entries)
            .with_context(|| format!("writing the history {}", self.file.display()))
    }
}

impl Options {
    fn parse(rest: &[String]) -> anyhow::Result<Self> {
        let mut options = Self::default();
        let mut args = rest.iter();
        while let Some(arg) = args.next() {
            let only = match arg.as_str() {
                "small" => {
                    options.small = true;
                    continue;
                }
                "--template" => {
                    options.template = true;
                    continue;
                }
                "--history" => {
                    options.set_history(HistoryFile::Checkout)?;
                    continue;
                }
                "--only" => args.next().context("--only <m1,m2>")?.clone(),
                other => {
                    if let Some(file) = other.strip_prefix("--history=") {
                        if file.is_empty() {
                            bail!("--history= without a file");
                        }
                        options.set_history(HistoryFile::At(PathBuf::from(file)))?;
                        continue;
                    }
                    match other.strip_prefix("--only=") {
                        Some(names) => names.to_owned(),
                        None => bail!(
                            "unexpected argument `{other}`: [small] [--only <m1,m2>] [--template] \
                             [--history[=<file>]]"
                        ),
                    }
                }
            };
            if options.only.replace(only).is_some() {
                bail!("--only twice");
            }
        }
        Ok(options)
    }

    /// The measures of the bench `name` this run selects.
    fn selection(&self, name: &str) -> anyhow::Result<Selection> {
        bench(name)
            .with_context(|| format!("no bench `{name}`"))?
            .select(self.only.as_deref())
    }

    /// How the run gets its prepared database: the template of `selection` for `seed` and
    /// the `volumes` of `level`, used under `--template`.
    fn reuse(
        &self,
        selection: &Selection,
        seed: u64,
        level: Level,
        volumes: &impl std::fmt::Debug,
    ) -> Reuse {
        Reuse::new(
            self.template,
            selection,
            seed,
            &format!("{level:?} {volumes:?}"),
        )
    }

    fn set_history(&mut self, file: HistoryFile) -> anyhow::Result<()> {
        if self.history.replace(file).is_some() {
            bail!("--history twice");
        }
        Ok(())
    }

    /// The history of the run under `--history`: the exclusion of `.bench/` checked, the
    /// commit and the state of the tree of the current directory read now, at the start.
    fn journal(
        &self,
        bench: &str,
        level: Level,
        seed: u64,
        reuse: &Reuse,
        load: Option<f64>,
    ) -> anyhow::Result<Option<Journal>> {
        let Some(target) = &self.history else {
            return Ok(None);
        };
        let here = std::env::current_dir()?;
        let file = match target {
            HistoryFile::Checkout => history::prepare(&here)?,
            HistoryFile::At(file) => file.clone(),
        };
        let run = Run::start(&here, bench, level.name(), seed, load, reuse.fingerprint())?;
        Ok(Some(Journal { file, run }))
    }
}

/// The scale of a run (ADR 0032): `small` after the seed, for tests; otherwise
/// `K_BENCH_SCALE`, `reduced` (default, about a tenth of the volumes) or `full` (the
/// reference dataset, the only one whose verdict is a budget measure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Full,
    Reduced,
    Small,
}

impl Level {
    fn of(small: bool) -> anyhow::Result<Self> {
        if small {
            return Ok(Self::Small);
        }
        match std::env::var("K_BENCH_SCALE").as_deref() {
            Ok("full") => Ok(Self::Full),
            Ok("reduced" | "") | Err(std::env::VarError::NotPresent) => Ok(Self::Reduced),
            Ok(other) => bail!("K_BENCH_SCALE must be `reduced` or `full`, not `{other}`"),
            Err(e) => bail!("K_BENCH_SCALE: {e}"),
        }
    }

    const fn scale(self) -> Scale {
        match self {
            Self::Full => Scale::REFERENCE,
            Self::Reduced => Scale::REDUCED,
            Self::Small => Scale::SMALL,
        }
    }

    /// The first lines of the output: the scale, then the measures under `--only`.
    fn announce(self, selection: &Selection) {
        println!("{}", self.scale_line());
        if let Some(line) = selection.announce() {
            println!("{line}");
        }
    }

    /// The first line of the output.
    const fn scale_line(self) -> &'static str {
        match self {
            Self::Full => "échelle complète (K_BENCH_SCALE=full) : mesure de budget",
            Self::Reduced => {
                "échelle réduite (K_BENCH_SCALE=reduced, volumes ÷ 10 environ) ; \
                 K_BENCH_SCALE=full pour une mesure de budget"
            }
            Self::Small => "échelle de test (small)",
        }
    }

    /// The last line of the output, after the verdicts, below the full scale.
    const fn caveat(self) -> Option<&'static str> {
        match self {
            Self::Full => None,
            Self::Reduced => Some("échelle réduite : ne vaut pas mesure de budget"),
            Self::Small => Some("échelle de test : ne vaut pas mesure de budget"),
        }
    }

    /// Its name in the history.
    const fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Reduced => "reduced",
            Self::Small => "small",
        }
    }

    /// The report, the caveat of the scale, the figures added to the history; fails when a
    /// budget is exceeded.
    fn verdict(
        self,
        report: &str,
        figures: &[Figure],
        journal: Option<&Journal>,
        failure: &str,
    ) -> anyhow::Result<()> {
        print!("{report}");
        if let Some(caveat) = self.caveat() {
            println!("{caveat}");
        }
        if let Some(journal) = journal {
            journal.write(figures)?;
        }
        if figures.iter().any(Figure::fails) {
            bail!("{failure}");
        }
        Ok(())
    }
}

#[allow(clippy::too_many_lines)] // one arm per command
fn main() -> anyhow::Result<()> {
    // The load of the machine when the run starts, for the history.
    let load = history::load();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, rest @ ..] if command == "history" => series(rest),
        [command] if command == "affected" => affected(),
        [command, seed, dir, rest @ ..] if command == "gen" => generate_files(seed, dir, rest),
        [command, flag, rest @ ..] if flag == "--list" => list(command, rest),
        [command, database, seed, rest @ ..]
            if ["props", "core", "rbac", "http", "policy", "flow"].contains(&command.as_str()) =>
        {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let options = Options::parse(rest)?;
            let selection = options.selection(command)?;
            let level = Level::of(options.small)?;
            level.announce(&selection);
            let data = generate(seed, level.scale());
            let reuse = options.reuse(&selection, seed, level, &level.scale());
            let journal = options.journal(command, level, seed, &reuse, load)?;
            let runtime = tokio::runtime::Runtime::new()?;
            let (s, r) = (&selection, &reuse);
            let (report, figures) = match command.as_str() {
                "props" => {
                    runtime.block_on(kartenn_bench::props::run(database, &data, seed, s, r))?
                }
                "rbac" => {
                    runtime.block_on(kartenn_bench::rbac::run(database, &data, seed, s, r))?
                }
                "http" => {
                    runtime.block_on(kartenn_bench::http::run(database, &data, seed, s, r))?
                }
                "policy" => runtime.block_on(kartenn_bench::policy_budgets::run(
                    database, &data, seed, s, r,
                ))?,
                "flow" => runtime.block_on(kartenn_bench::flow_budgets::run(
                    database, &data, seed, s, r,
                ))?,
                _ => runtime.block_on(kartenn_bench::core_budgets::run(
                    database, &data, seed, s, r,
                ))?,
            };
            level.verdict(&report, &figures, journal.as_ref(), "a budget is exceeded")
        }
        [command, database, seed, rest @ ..] if command == "events" => {
            // The seed draws nothing here; it is checked as elsewhere.
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let options = Options::parse(rest)?;
            let selection = options.selection(command)?;
            let level = Level::of(options.small)?;
            level.announce(&selection);
            let sizes = match level {
                Level::Full => kartenn_bench::events_budgets::Sizes::FULL,
                Level::Reduced => kartenn_bench::events_budgets::Sizes::REDUCED,
                Level::Small => kartenn_bench::events_budgets::Sizes::SMALL,
            };
            let reuse = options.reuse(&selection, seed, level, &sizes);
            let journal = options.journal(command, level, seed, &reuse, load)?;
            let (report, figures) = tokio::runtime::Runtime::new()?.block_on(
                kartenn_bench::events_budgets::run(database, sizes, &selection, &reuse),
            )?;
            level.verdict(&report, &figures, journal.as_ref(), "a budget is exceeded")
        }
        [command, database, seed, rest @ ..] if command == "seed" => {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            // The standard output is the ids of the seeded domain (`bench-web`).
            let level = Level::of(rest.first().map(String::as_str) == Some("small"))?;
            eprintln!("{}", level.scale_line());
            let data = generate(seed, level.scale());
            let line = tokio::runtime::Runtime::new()?
                .block_on(kartenn_bench::core_budgets::seed(database, &data, seed))?;
            print!("{line}");
            Ok(())
        }
        [command, database, seed, rest @ ..] if command == "telemetry" => {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let options = Options::parse(rest)?;
            let selection = options.selection(command)?;
            let level = Level::of(options.small)?;
            level.announce(&selection);
            let data = generate(seed, level.scale());
            let reuse = options.reuse(&selection, seed, level, &level.scale());
            let journal = options.journal(command, level, seed, &reuse, load)?;
            let runtime = tokio::runtime::Runtime::new()?;
            let (report, figures) = runtime.block_on(kartenn_bench::telemetry::run(
                database,
                &data,
                seed,
                options.small,
                &selection,
                &reuse,
            ))?;
            level.verdict(
                &report,
                &figures,
                journal.as_ref(),
                "an overhead is exceeded",
            )
        }
        // A child of `telemetry`: the same scale and measures, through the same arguments or
        // environment.
        [command, database, seed, config, result, rest @ ..] if command == "telemetry-run" => {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let options = Options::parse(rest)?;
            let selection = options.selection("telemetry")?;
            let data = generate(seed, Level::of(options.small)?.scale());
            tokio::runtime::Runtime::new()?.block_on(kartenn_bench::telemetry::measure(
                database,
                &data,
                seed,
                config.as_ref(),
                result.as_ref(),
                &selection,
            ))
        }
        _ => bail!(
            "usage: kartenn-bench gen <seed> <output-dir> [small] \
             | props|core|rbac|http|policy|flow|events|telemetry <database-url> <seed> [small] [--only <m1,m2>] [--template] [--history[=<file>]] \
             | props|core|rbac|http|policy|flow|events|telemetry --list [--only <m1,m2>] \
             | history <measure> [--scale full|reduced|small] \
             | affected < <paths> \
             | seed <database-url> <seed> [small]"
        ),
    }
}

/// `<bench> --list`: the catalogue; with `--only`, the names checked and the measures that
/// would run.
fn list(command: &str, rest: &[String]) -> anyhow::Result<()> {
    let selection = Options::parse(rest)?.selection(command)?;
    match selection.announce() {
        Some(line) => println!("{line}"),
        None => print!("{}", bench(command).map(Bench::list).unwrap_or_default()),
    }
    Ok(())
}

/// `history <measure> [--scale <scale>]`: the series of a measure of the catalogues in the
/// history of the main checkout of the current directory, oldest first.
fn series(rest: &[String]) -> anyhow::Result<()> {
    const USAGE: &str = "usage: kartenn-bench history <measure> [--scale full|reduced|small]";
    let (measure, scale) = match rest {
        [measure] => (measure, None),
        [measure, flag, scale] if flag == "--scale" => (measure, Some(scale.as_str())),
        _ => bail!("{USAGE}"),
    };
    if scale.is_some_and(|s| !["full", "reduced", "small"].contains(&s)) {
        bail!("{USAGE}");
    }
    let name = measure.split_once('.').map_or(measure.as_str(), |(b, _)| b);
    bench(name)
        .with_context(|| format!("`{measure}` names no bench"))?
        .select(Some(measure))?;
    let (_, file) = history::location(&std::env::current_dir()?)?;
    let (entries, unread) = history::read(&file)?;
    print!("{}", history::series(&entries, measure, scale));
    if unread > 0 {
        eprintln!("{unread} lignes illisibles de l'historique ignorées");
    }
    Ok(())
}

/// `affected`: the changed paths on the standard input, one per line; on the standard
/// output, one line per bench to run (its script, then the measures for `--only` when not
/// all of them run); on the error output, what each path calls for, and the scripts
/// outside `kartenn-bench` to propose.
fn affected() -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("reading the changed paths")?;
    let affected = kartenn_bench::affected::affected(input.lines());
    eprint!("{}", affected.explained);
    if affected.unconcerned > 0 {
        eprintln!(
            "{} chemins sans mesure (documentation, tests, web, crates qu'aucun banc n'exerce…)",
            affected.unconcerned
        );
    }
    if affected.runs.is_empty() {
        eprintln!("aucune mesure de kartenn-bench touchée");
    }
    if affected.config {
        eprintln!(
            "avertissement : {}",
            kartenn_bench::affected::CONFIG_WARNING
        );
    }
    for script in &affected.proposed {
        eprintln!(
            "à proposer : {script} (banc hors kartenn-bench, que bench-affected ne lance pas)"
        );
    }
    print!("{}", affected.plan());
    Ok(())
}

fn generate_files(seed: &str, dir: &str, rest: &[String]) -> anyhow::Result<()> {
    let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let level = Level::of(rest.first().map(String::as_str) == Some("small"))?;
    println!("{}", level.scale_line());
    let started = Instant::now();
    let data = generate(seed, level.scale());
    let generated = started.elapsed();

    let open = |name: &str| -> anyhow::Result<BufWriter<File>> {
        let path = dir.join(name);
        Ok(BufWriter::new(
            File::create(&path).with_context(|| format!("creating {}", path.display()))?,
        ))
    };
    data.write_domains(&mut open("domains.tsv")?)?;
    data.write_prefixes(&mut open("prefixes.tsv")?)?;
    data.write_ranges(&mut open("ranges.tsv")?)?;
    data.write_addresses(&mut open("addresses.tsv")?)?;

    println!(
        "domains={} prefixes={} ranges={} addresses={} max_container_depth={} generated_in={:?} total={:?}",
        data.domains.len(),
        data.prefixes.len(),
        data.ranges.len(),
        data.addresses.len(),
        data.max_container_depth(),
        generated,
        started.elapsed(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> anyhow::Result<Options> {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        Options::parse(&args)
    }

    #[test]
    fn small_and_only_in_any_order() {
        let only = |names: &str| Some(names.to_owned());
        for (args, small, names) in [
            (&[][..], false, None),
            (&["small"][..], true, None),
            (
                &["--only", "props.changeset-10k"][..],
                false,
                only("props.changeset-10k"),
            ),
            (&["small", "--only=a,b"][..], true, only("a,b")),
            (&["--only", "a", "small"][..], true, only("a")),
        ] {
            assert_eq!(
                parse(args).unwrap(),
                Options {
                    small,
                    only: names,
                    ..Options::default()
                },
                "{args:?}"
            );
        }
        assert_eq!(
            parse(&["small", "--template", "--only", "a"]).unwrap(),
            Options {
                small: true,
                only: only("a"),
                template: true,
                history: None,
            }
        );
        for args in [&["--only"][..], &["large"], &["--only", "a", "--only=b"]] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn history_to_the_checkout_or_to_a_file() {
        assert_eq!(
            parse(&["--history", "small"]).unwrap().history,
            Some(HistoryFile::Checkout)
        );
        assert_eq!(
            parse(&["--template", "--history=runs.jsonl"])
                .unwrap()
                .history,
            Some(HistoryFile::At(PathBuf::from("runs.jsonl")))
        );
        for args in [&["--history="][..], &["--history", "--history=a"]] {
            assert!(parse(args).is_err(), "{args:?}");
        }
        assert_eq!(Level::Reduced.name(), "reduced");
        assert_eq!(Level::Full.name(), "full");
    }

    #[test]
    fn an_unknown_measure_is_refused() {
        let options = parse(&["--only", "props.nope"]).unwrap();
        let error = options.selection("props").unwrap_err().to_string();
        assert!(error.contains("props.origins-3-domains"), "{error}");
        assert!(options.selection("stats").is_err());
    }
}
