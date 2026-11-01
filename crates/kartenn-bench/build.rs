//! The fingerprint of the templates of the benches (ADR 0041, decision 2) covers the
//! workspace crates their preparation goes through: here, the hash of the sources of each
//! workspace crate `kartenn-bench` depends on (its `Cargo.toml`, `build.rs`, `src/`,
//! `migrations/`, `data/` — the files its sources include),
//! its own workspace dependencies, and the hash of `Cargo.lock`, written to
//! `$OUT_DIR/crates.rs`. Rebuilt when one of them changes.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// 64-bit FNV-1a, as `template.rs`; each field preceded by its length.
struct Fnv(u64);

impl Fnv {
    fn field(&mut self, bytes: &[u8]) {
        for b in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// A hash as a Rust literal, its digits grouped by four.
fn literal(value: u64) -> String {
    let s = format!("{value:016x}");
    format!("0x{}_{}_{}_{}", &s[..4], &s[4..8], &s[8..12], &s[12..])
}

/// The files under `dir`, recursively, sorted.
fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// The workspace crates (`path = "../<crate>"`) of the `[dependencies]` and
/// `[build-dependencies]` of a manifest, target-specific ones included: what its library
/// is built from, without its development dependencies.
fn workspace_dependencies(manifest: &str) -> Vec<String> {
    let mut built = false;
    let mut out = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            let section = line.trim_matches(|c| c == '[' || c == ']');
            built = section == "dependencies"
                || section == "build-dependencies"
                || (section.starts_with("target.")
                    && (section.ends_with(".dependencies")
                        || section.ends_with(".build-dependencies")));
            continue;
        }
        if built
            && let Some((_, rest)) = line.split_once("path = \"../")
            && let Some((name, _)) = rest.split_once('"')
        {
            out.push(name.to_owned());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn main() {
    let bench = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let crates = bench.parent().expect("the crates directory").to_owned();
    let root = crates.parent().expect("the workspace").to_owned();
    let read = |path: &Path| fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let mut seen: BTreeMap<String, (u64, Vec<String>)> = BTreeMap::new();
    let manifest = String::from_utf8(read(&bench.join("Cargo.toml"))).expect("UTF-8");
    let mut todo = workspace_dependencies(&manifest);
    while let Some(name) = todo.pop() {
        if seen.contains_key(&name) {
            continue;
        }
        let dir = crates.join(&name);
        let manifest_path = dir.join("Cargo.toml");
        let manifest = String::from_utf8(read(&manifest_path)).expect("UTF-8");
        let dependencies = workspace_dependencies(&manifest);
        println!("cargo:rerun-if-changed={}", manifest_path.display());
        let mut paths = vec![manifest_path];
        // Only paths that exist: cargo reruns a build script that names a missing one at
        // every build, and recompiles this crate each time (three builds per bench run).
        let script = dir.join("build.rs");
        if script.is_file() {
            println!("cargo:rerun-if-changed={}", script.display());
            paths.push(script);
        }
        for sub in ["src", "migrations", "data"] {
            let sub = dir.join(sub);
            if sub.exists() {
                println!("cargo:rerun-if-changed={}", sub.display());
            }
            files(&sub, &mut paths);
        }
        let mut h = Fnv(0xcbf2_9ce4_8422_2325);
        for path in &paths {
            let relative = path.strip_prefix(&dir).unwrap_or(path);
            h.field(relative.to_string_lossy().as_bytes());
            h.field(&read(path));
        }
        todo.extend(dependencies.iter().cloned());
        seen.insert(name, (h.0, dependencies));
    }

    let lock = root.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    h.field(&read(&lock));

    let mut out = String::new();
    writeln!(out, "/// The hash of `Cargo.lock`.").unwrap();
    writeln!(out, "pub(crate) const CARGO_LOCK: u64 = {};", literal(h.0)).unwrap();
    writeln!(
        out,
        "/// Each workspace crate `kartenn-bench` depends on: its name, the hash of its \
         sources, its own workspace dependencies."
    )
    .unwrap();
    writeln!(out, "pub(crate) const CRATES: &[(&str, u64, &[&str])] = &[").unwrap();
    for (name, (hash, dependencies)) in &seen {
        writeln!(
            out,
            "    ({name:?}, {}, &{dependencies:?}),",
            literal(*hash)
        )
        .unwrap();
    }
    writeln!(out, "];").unwrap();
    let path = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("crates.rs");
    fs::write(path, out).expect("writing crates.rs");
}
