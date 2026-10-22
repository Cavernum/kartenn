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
//! for a person with 50 groups and 1 000 bindings, and its overhead (REQ-005-29).
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
//! module (REQ-010-28, 54) after those of the core's dataset, VLAN and machines modules.
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
            if ["props", "core", "rbac", "http", "policy", "flow"].contains(&command.as_str()) =>
        {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let data = generate(seed, scale(rest));
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
            print!("{report}");
            if !ok {
                bail!("a budget is exceeded");
            }
            Ok(())
        }
        [command, database, seed, rest @ ..] if command == "seed" => {
            let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
            let data = generate(seed, scale(rest));
            let line = tokio::runtime::Runtime::new()?
                .block_on(kartenn_bench::core_budgets::seed(database, &data, seed))?;
            print!("{line}");
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
            "usage: kartenn-bench gen <seed> <output-dir> [small] | props|core|rbac|http|telemetry|seed <database-url> <seed> [small]"
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
