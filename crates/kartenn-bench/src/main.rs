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
//! `kartenn-bench rbac <database-url> <seed> [small]`: the overhead of access control on
//! those budgets, for a person with 50 groups and 1 000 bindings (REQ-005-29).
//!
//! `kartenn-bench telemetry <database-url> <seed> [small]`: the overhead of the telemetry
//! on those budgets, in three configurations (REQ-012-23), and the volume of the audit
//! of reads (REQ-012-18); each configuration runs in a child (`telemetry-run`).

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, bail};
use kartenn_bench::{Scale, generate};

fn scale(rest: &[String]) -> Scale {
    if rest.first().map(String::as_str) == Some("small") {
        Scale::SMALL
    } else {
        Scale::REFERENCE
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, seed, dir, rest @ ..] if command == "gen" => generate_files(seed, dir, rest),
        [command, database, seed, rest @ ..]
            if command == "props" || command == "core" || command == "rbac" =>
        {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let data = generate(seed, scale(rest));
            let runtime = tokio::runtime::Runtime::new()?;
            let (report, ok) = match command.as_str() {
                "props" => runtime.block_on(kartenn_bench::props::run(database, &data, seed))?,
                "rbac" => runtime.block_on(kartenn_bench::rbac::run(database, &data, seed))?,
                _ => runtime.block_on(kartenn_bench::core_budgets::run(database, &data, seed))?,
            };
            print!("{report}");
            if !ok {
                bail!("a budget is exceeded");
            }
            Ok(())
        }
        [command, database, seed, rest @ ..] if command == "telemetry" => {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let small = rest.first().map(String::as_str) == Some("small");
            let data = generate(seed, scale(rest));
            let runtime = tokio::runtime::Runtime::new()?;
            let (report, ok) =
                runtime.block_on(kartenn_bench::telemetry::run(database, &data, seed, small))?;
            print!("{report}");
            if !ok {
                bail!("an overhead is exceeded");
            }
            Ok(())
        }
        [command, database, seed, config, result, rest @ ..] if command == "telemetry-run" => {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let data = generate(seed, scale(rest));
            tokio::runtime::Runtime::new()?.block_on(kartenn_bench::telemetry::measure(
                database,
                &data,
                seed,
                config.as_ref(),
                result.as_ref(),
            ))
        }
        _ => bail!(
            "usage: kartenn-bench gen <seed> <output-dir> [small] | props|core|rbac|telemetry <database-url> <seed> [small]"
        ),
    }
}

fn generate_files(seed: &str, dir: &str, rest: &[String]) -> anyhow::Result<()> {
    let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let started = Instant::now();
    let data = generate(seed, scale(rest));
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
