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

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, bail};
use kartenn_bench::measures::{Selection, bench};
use kartenn_bench::{Scale, generate};

/// What follows the seed: `small`, and `--only <m1,m2>` (or `--only=<m1,m2>`), in any order.
#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    small: bool,
    only: Option<String>,
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
                "--only" => args.next().context("--only <m1,m2>")?.clone(),
                other => match other.strip_prefix("--only=") {
                    Some(names) => names.to_owned(),
                    None => bail!("unexpected argument `{other}`: [small] [--only <m1,m2>]"),
                },
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

    fn verdict(self, report: &str, ok: bool, failure: &str) -> anyhow::Result<()> {
        print!("{report}");
        if let Some(caveat) = self.caveat() {
            println!("{caveat}");
        }
        if !ok {
            bail!("{failure}");
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, seed, dir, rest @ ..] if command == "gen" => generate_files(seed, dir, rest),
        [command, flag] if flag == "--list" => {
            let bench = bench(command).with_context(|| format!("no bench `{command}`"))?;
            print!("{}", bench.list());
            Ok(())
        }
        [command, database, seed, rest @ ..]
            if ["props", "core", "rbac", "http", "policy", "flow"].contains(&command.as_str()) =>
        {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let options = Options::parse(rest)?;
            let selection = options.selection(command)?;
            let level = Level::of(options.small)?;
            level.announce(&selection);
            let data = generate(seed, level.scale());
            let runtime = tokio::runtime::Runtime::new()?;
            let s = &selection;
            let (report, ok) = match command.as_str() {
                "props" => runtime.block_on(kartenn_bench::props::run(database, &data, seed, s))?,
                "rbac" => runtime.block_on(kartenn_bench::rbac::run(database, &data, seed, s))?,
                "http" => runtime.block_on(kartenn_bench::http::run(database, &data, seed, s))?,
                "policy" => runtime
                    .block_on(kartenn_bench::policy_budgets::run(database, &data, seed, s))?,
                "flow" => {
                    runtime.block_on(kartenn_bench::flow_budgets::run(database, &data, seed, s))?
                }
                _ => {
                    runtime.block_on(kartenn_bench::core_budgets::run(database, &data, seed, s))?
                }
            };
            level.verdict(&report, ok, "a budget is exceeded")
        }
        [command, database, seed, rest @ ..] if command == "events" => {
            // The seed draws nothing here; it is checked as elsewhere.
            let _: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let options = Options::parse(rest)?;
            let selection = options.selection(command)?;
            let level = Level::of(options.small)?;
            level.announce(&selection);
            let sizes = match level {
                Level::Full => kartenn_bench::events_budgets::Sizes::FULL,
                Level::Reduced => kartenn_bench::events_budgets::Sizes::REDUCED,
                Level::Small => kartenn_bench::events_budgets::Sizes::SMALL,
            };
            let (report, ok) = tokio::runtime::Runtime::new()?.block_on(
                kartenn_bench::events_budgets::run(database, sizes, &selection),
            )?;
            level.verdict(&report, ok, "a budget is exceeded")
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
            let runtime = tokio::runtime::Runtime::new()?;
            let (report, ok) = runtime.block_on(kartenn_bench::telemetry::run(
                database,
                &data,
                seed,
                options.small,
                &selection,
            ))?;
            level.verdict(&report, ok, "an overhead is exceeded")
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
             | props|core|rbac|http|policy|flow|events|telemetry <database-url> <seed> [small] [--only <m1,m2>] \
             | props|core|rbac|http|policy|flow|events|telemetry --list \
             | seed <database-url> <seed> [small]"
        ),
    }
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
                Options { small, only: names },
                "{args:?}"
            );
        }
        for args in [&["--only"][..], &["large"], &["--only", "a", "--only=b"]] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn an_unknown_measure_is_refused() {
        let options = parse(&["--only", "props.nope"]).unwrap();
        let error = options.selection("props").unwrap_err().to_string();
        assert!(error.contains("props.origins-3-domains"), "{error}");
        assert!(options.selection("stats").is_err());
    }
}
