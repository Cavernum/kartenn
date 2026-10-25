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

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, bail};
use kartenn_bench::{Scale, generate};

/// The scale of a run (ADR 0032): `small` as last argument, for tests; otherwise
/// `K_BENCH_SCALE`, `reduced` (default, about a tenth of the volumes) or `full` (the
/// reference dataset, the only one whose verdict is a budget measure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Full,
    Reduced,
    Small,
}

impl Level {
    fn of(rest: &[String]) -> anyhow::Result<Self> {
        if rest.first().map(String::as_str) == Some("small") {
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

    /// The first line of the output.
    const fn announce(self) -> &'static str {
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
        [command, database, seed, rest @ ..]
            if ["props", "core", "rbac", "http", "policy", "flow"].contains(&command.as_str()) =>
        {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let level = Level::of(rest)?;
            println!("{}", level.announce());
            let data = generate(seed, level.scale());
            let runtime = tokio::runtime::Runtime::new()?;
            let (report, ok) = match command.as_str() {
                "props" => runtime.block_on(kartenn_bench::props::run(database, &data, seed))?,
                "rbac" => runtime.block_on(kartenn_bench::rbac::run(database, &data, seed))?,
                "http" => runtime.block_on(kartenn_bench::http::run(database, &data, seed))?,
                "policy" => {
                    runtime.block_on(kartenn_bench::policy_budgets::run(database, &data, seed))?
                }
                "flow" => {
                    runtime.block_on(kartenn_bench::flow_budgets::run(database, &data, seed))?
                }
                _ => runtime.block_on(kartenn_bench::core_budgets::run(database, &data, seed))?,
            };
            level.verdict(&report, ok, "a budget is exceeded")
        }
        [command, database, _seed, rest @ ..] if command == "events" => {
            let level = Level::of(rest)?;
            println!("{}", level.announce());
            let sizes = match level {
                Level::Full => kartenn_bench::events_budgets::Sizes::FULL,
                Level::Reduced => kartenn_bench::events_budgets::Sizes::REDUCED,
                Level::Small => kartenn_bench::events_budgets::Sizes::SMALL,
            };
            let (report, ok) = tokio::runtime::Runtime::new()?
                .block_on(kartenn_bench::events_budgets::run(database, sizes))?;
            level.verdict(&report, ok, "a budget is exceeded")
        }
        [command, database, seed, rest @ ..] if command == "seed" => {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            // The standard output is the ids of the seeded domain (`bench-web`).
            let level = Level::of(rest)?;
            eprintln!("{}", level.announce());
            let data = generate(seed, level.scale());
            let line = tokio::runtime::Runtime::new()?
                .block_on(kartenn_bench::core_budgets::seed(database, &data, seed))?;
            print!("{line}");
            Ok(())
        }
        [command, database, seed, rest @ ..] if command == "telemetry" => {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let level = Level::of(rest)?;
            println!("{}", level.announce());
            let data = generate(seed, level.scale());
            let runtime = tokio::runtime::Runtime::new()?;
            let small = level == Level::Small;
            let (report, ok) =
                runtime.block_on(kartenn_bench::telemetry::run(database, &data, seed, small))?;
            level.verdict(&report, ok, "an overhead is exceeded")
        }
        // A child of `telemetry`: the same scale, through the same argument or environment.
        [command, database, seed, config, result, rest @ ..] if command == "telemetry-run" => {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let data = generate(seed, Level::of(rest)?.scale());
            tokio::runtime::Runtime::new()?.block_on(kartenn_bench::telemetry::measure(
                database,
                &data,
                seed,
                config.as_ref(),
                result.as_ref(),
            ))
        }
        _ => bail!(
            "usage: kartenn-bench gen <seed> <output-dir> [small] | props|core|rbac|http|events|telemetry|seed <database-url> <seed> [small]"
        ),
    }
}

fn generate_files(seed: &str, dir: &str, rest: &[String]) -> anyhow::Result<()> {
    let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let level = Level::of(rest)?;
    println!("{}", level.announce());
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
