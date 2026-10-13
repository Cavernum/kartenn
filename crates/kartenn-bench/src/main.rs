//! `kartenn-bench gen <seed> <output-dir> [small]`: writes the synthetic dataset as
//! PostgreSQL `COPY` text files (`domains.tsv`, `prefixes.tsv`, `ranges.tsv`,
//! `addresses.tsv`) and prints counts and timing.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, bail};
use kartenn_bench::{Scale, generate};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [command, seed, dir, rest @ ..] = args.as_slice() else {
        bail!("usage: kartenn-bench gen <seed> <output-dir> [small]");
    };
    if command != "gen" {
        bail!("unknown command {command}");
    }
    let seed: u64 = seed.parse().context("seed must be an unsigned integer")?;
    let scale = if rest.first().map(String::as_str) == Some("small") {
        Scale::SMALL
    } else {
        Scale::REFERENCE
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let started = Instant::now();
    let data = generate(seed, scale);
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
