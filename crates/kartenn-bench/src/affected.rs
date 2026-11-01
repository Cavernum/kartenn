//! The measures a change touches (ADR 0041, decision 3; task 028-4).
//!
//! In doubt, a change runs more, never less. [`PATHS`] associates each measure of the
//! catalogues with the paths it exercises outside the fingerprint of its bench: the
//! modules of `kartenn-bench` its code goes through, from the file that times it, and the
//! workspace crates they name, whole (`crates/<crate>/`), with their own workspace
//! dependencies. No single file of another crate is listed: the plumbing of a crate (its
//! pipeline, its models, its router) is reached by every measure that goes through the
//! crate. A test derives the table again from the sources and checks it is the same.
//!
//! For a changed path ([`touched`]), each bench runs:
//! - whole, when the path is in its fingerprint (ADR 0041, decision 2: the sources of its
//!   preparation, the crates its preparation goes through, every migration, `Cargo.lock`),
//!   since its template is made again;
//! - otherwise the measures whose paths cover it (whole when all of them do);
//! - otherwise nothing.
//!
//! A path in no fingerprint nor in the table: the crate root, the manifest and the build
//! script of `kartenn-bench` and the manifest of the workspace run every bench; the
//! configuration of the benches (`devenv.nix`, `devenv.lock`) runs nothing but calls for a
//! full run; under `web/v2/`, `web/` or `clients/python/`, the bench script outside
//! `kartenn-bench` it concerns is proposed, not run ([`PROPOSED`]); anything else
//! (documentation, specs, tests, crates no bench exercises) runs nothing.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::measures::{BENCHES, Bench};
use crate::template;

/// This crate.
const BENCH_CRATE: &str = "kartenn-bench";

/// The files of `kartenn-bench` that time nothing a change of them could slow down: the
/// selection of the measures, the history, this table.
const UNMEASURED: &[&str] = &[
    "crates/kartenn-bench/src/affected.rs",
    "crates/kartenn-bench/src/history.rs",
    "crates/kartenn-bench/src/measures.rs",
];

/// The files of `kartenn-bench` every bench goes through: its crate root (the generator,
/// the modules), the commands, its manifest and its build script.
const CRATE_ROOT: &[&str] = &[
    "crates/kartenn-bench/src/lib.rs",
    "crates/kartenn-bench/src/main.rs",
    "crates/kartenn-bench/Cargo.toml",
    "crates/kartenn-bench/build.rs",
];

/// The files of the workspace whose change concerns every bench: the versions of the
/// dependencies (in every fingerprint), the profiles.
const WORKSPACE: &[&str] = &["Cargo.lock", "Cargo.toml"];

/// The configuration of the benches: the scripts, the instance and its settings.
const CONFIG: &[&str] = &["devenv.nix", "devenv.lock"];

/// What a change of the configuration of the benches calls for.
pub const CONFIG_WARNING: &str = "configuration des bancs touchée : un passage complet est \
     conseillé, après redémarrage de `k-pg-bench` si `pgBench` a changé";

/// The bench scripts outside `kartenn-bench` (Playwright, Python): the start of the paths
/// that concern them, and the script. The first match wins.
pub const PROPOSED: &[(&str, &str)] = &[
    ("web/v2/", "bench-ui"),
    ("web/", "bench-web"),
    ("clients/python/", "bench-cli"),
];

/// Files of plumbing, shared by the measures of several benches, that the table must never
/// name on their own (a test): a change of them runs every bench that goes through their
/// crate.
pub const PLUMBING: &[&str] = &[
    "crates/kartenn-core/src/objects.rs",
    "crates/kartenn-service/src/authz.rs",
    "crates/kartenn-service/src/operations.rs",
    "crates/kartenn-service/src/pipeline.rs",
    "crates/kartenn-store-pg/src/audit_journal.rs",
    "crates/kartenn-store-pg/src/changesets.rs",
    "crates/kartenn-store-pg/src/modules.rs",
    "crates/kartenn-store-pg/src/outbox.rs",
    "crates/kartenn-store-pg/src/rbac.rs",
    "crates/kartenn-store-pg/src/write.rs",
    "crates/kartenn-rest/src/cursor.rs",
    "crates/kartenn-rest/src/dated.rs",
    "crates/kartenn-rest/src/filter.rs",
    "crates/kartenn-rest/src/objects.rs",
    "crates/kartenn-server/src/app.rs",
];

/// The modules of the dataset of `bench-core` and of its preparation, through which every
/// measure goes.
const CORE: &[&str] = &[
    "crates/kartenn-bench/src/core_budgets.rs",
    "crates/kartenn-bench/src/props.rs",
    "crates/kartenn-bench/src/template.rs",
];
/// The person, its groups and its bindings.
const RBAC: &[&str] = &["crates/kartenn-bench/src/rbac.rs"];
/// The router and what it goes through, outside the fingerprint of `core`.
const ROUTER: &[&str] = &[
    "crates/kartenn-bench/src/vlan_budgets.rs",
    "crates/kartenn-auth/",
    "crates/kartenn-rest/",
    "crates/kartenn-telemetry/",
    "crates/syslog-sender/",
];
const MACHINES: &[&str] = &[
    "crates/kartenn-bench/src/machine_budgets.rs",
    "crates/kartenn-machines/",
];
const STATS: &[&str] = &[
    "crates/kartenn-bench/src/stats_budgets.rs",
    "crates/kartenn-machines/",
];
const HTTP: &[&str] = &["crates/kartenn-bench/src/http.rs"];
const POLICY: &[&str] = &[
    "crates/kartenn-bench/src/policy_budgets.rs",
    "crates/kartenn-bench/src/vlan_budgets.rs",
];
const FLOW: &[&str] = &[
    "crates/kartenn-bench/src/flow_budgets.rs",
    "crates/kartenn-bench/src/vlan_budgets.rs",
];
const EVENTS: &[&str] = &["crates/kartenn-bench/src/events_budgets.rs"];
/// The server started under each configuration of the telemetry, with its modules.
const TELEMETRY: &[&str] = &[
    "crates/kartenn-bench/src/telemetry.rs",
    "crates/kartenn-auth/",
    "crates/kartenn-flow/",
    "crates/kartenn-machines/",
    "crates/kartenn-policy/",
    "crates/kartenn-rest/",
    "crates/kartenn-server/",
    "crates/kartenn-telemetry/",
    "crates/syslog-sender/",
];

/// Each measure of the catalogues and the paths it exercises outside the fingerprint of
/// its bench (derived again from the sources by a test).
pub static PATHS: &[(&str, &[&[&str]])] = &[
    ("props.origins-every-root", &[CORE]),
    ("props.explained-resolution", &[CORE]),
    ("props.grouped-resolution", &[CORE]),
    ("props.filtered-pages", &[CORE]),
    ("props.changeset-10k", &[CORE]),
    ("props.value-every-root", &[CORE]),
    ("props.origins-3-domains", &[CORE]),
    ("core.read-object", &[CORE]),
    ("core.history-index", &[CORE]),
    ("core.filtered-page", &[CORE]),
    ("core.subtree", &[CORE]),
    ("core.roots-page", &[CORE]),
    ("core.wide-children", &[CORE]),
    ("core.subtree-past", &[CORE]),
    ("core.unitary-changeset", &[CORE]),
    ("core.sustained", &[CORE]),
    ("core.import", &[CORE]),
    ("core.vlan-read", &[CORE, RBAC, ROUTER]),
    ("core.vlan-filtered-page", &[CORE, RBAC, ROUTER]),
    ("core.vlan-occupancy", &[CORE, RBAC, ROUTER]),
    ("core.vlan-create", &[CORE, RBAC, ROUTER]),
    ("core.machines-read", &[CORE, RBAC, ROUTER, MACHINES]),
    ("core.machines-owner-page", &[CORE, RBAC, ROUTER, MACHINES]),
    ("core.machines-site-page", &[CORE, RBAC, ROUTER, MACHINES]),
    ("core.machines-network", &[CORE, RBAC, ROUTER, MACHINES]),
    ("core.machines-create", &[CORE, RBAC, ROUTER, MACHINES]),
    ("core.machines-suggestions", &[CORE, RBAC, ROUTER, MACHINES]),
    ("core.machines-import", &[CORE, RBAC, ROUTER, MACHINES]),
    ("core.stats", &[CORE, RBAC, ROUTER, STATS]),
    ("rbac.read-object", &[CORE, RBAC]),
    ("rbac.read-past", &[CORE, RBAC]),
    ("rbac.filtered-page", &[CORE, RBAC]),
    ("rbac.subtree", &[CORE, RBAC]),
    ("rbac.unitary-changeset", &[CORE, RBAC]),
    ("rbac.resolution", &[CORE, RBAC]),
    ("http.read-object", &[CORE, RBAC, HTTP]),
    ("http.read-past", &[CORE, RBAC, HTTP]),
    ("http.filtered-page", &[CORE, RBAC, HTTP]),
    ("http.subtree", &[CORE, RBAC, HTTP]),
    ("http.unitary-changeset", &[CORE, RBAC, HTTP]),
    ("http.labels", &[CORE, RBAC, HTTP]),
    ("http.load", &[CORE, RBAC, HTTP]),
    ("policy.snapshot-cold", &[CORE, RBAC, POLICY]),
    ("policy.verdict-pair", &[CORE, RBAC, POLICY]),
    ("policy.interface-pair", &[CORE, RBAC, POLICY]),
    ("policy.raw-port", &[CORE, RBAC, POLICY]),
    ("policy.rules-differential", &[CORE, RBAC, POLICY]),
    ("policy.audit-report", &[CORE, RBAC, POLICY]),
    ("policy.inference-suggestion", &[CORE, RBAC, POLICY]),
    ("policy.inference-control", &[CORE, RBAC, POLICY]),
    ("policy.inference-preview", &[CORE, RBAC, POLICY]),
    ("policy.inference-check", &[CORE, RBAC, POLICY]),
    ("flow.request-interfaces", &[CORE, RBAC, FLOW]),
    ("flow.reclassify", &[CORE, RBAC, FLOW]),
    ("flow.rule-written", &[CORE, RBAC, FLOW]),
    ("flow.policy-written", &[CORE, RBAC, FLOW]),
    ("flow.flows-page", &[CORE, RBAC, FLOW]),
    ("flow.nonconformities-page", &[CORE, RBAC, FLOW]),
    ("flow.endpoints-flows-page", &[CORE, RBAC, FLOW]),
    ("flow.endpoints-nonconformities-page", &[CORE, RBAC, FLOW]),
    ("events.reader-overhead", &[CORE, RBAC, EVENTS]),
    ("events.latency", &[CORE, RBAC, EVENTS]),
    ("events.memory", &[CORE, RBAC, EVENTS]),
    ("events.storm", &[CORE, RBAC, EVENTS]),
    ("events.visibility", &[CORE, RBAC, EVENTS]),
    ("telemetry.read-object", &[CORE, TELEMETRY]),
    ("telemetry.filtered-page", &[CORE, TELEMETRY]),
    ("telemetry.subtree", &[CORE, TELEMETRY]),
    ("telemetry.unitary-changeset", &[CORE, TELEMETRY]),
    ("telemetry.sustained", &[CORE, TELEMETRY]),
    ("telemetry.audit-volume", &[CORE, TELEMETRY]),
];

/// The paths of `measure`, in the order of the table, without repetition; none for a
/// name the table does not have.
pub fn paths(measure: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for (_, groups) in PATHS.iter().filter(|(m, _)| *m == measure) {
        for path in groups.iter().flat_map(|g| g.iter().copied()) {
            if !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// Whether the entry of the table `listed` (a file, or a directory ending with `/`)
/// covers the path `path`.
fn covers(listed: &str, path: &str) -> bool {
    if listed.ends_with('/') {
        path.starts_with(listed)
    } else {
        path == listed
    }
}

/// The crate of a path under `crates/`, and the rest of the path in it.
fn crate_of(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix("crates/")?.split_once('/')
}

/// Whether `rest`, a path in a crate, is what it is built from: what the fingerprint of
/// the templates covers (`build.rs` of this crate).
fn is_code(rest: &str) -> bool {
    rest == "Cargo.toml"
        || rest == "build.rs"
        || rest.starts_with("src/")
        || rest.starts_with("migrations/")
        || rest.starts_with("data/")
}

/// The crates in the fingerprint of `bench`: those its preparation goes through, with
/// their workspace dependencies.
pub fn fingerprinted(bench: &str) -> Vec<&'static str> {
    template::crate_hashes(template::crates(bench))
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

/// Whether `path`, code of a crate (`name`, `rest`), is in the fingerprint of `bench`.
fn in_fingerprint(bench: &str, name: &str, rest: &str) -> bool {
    if name == BENCH_CRATE {
        return rest
            .strip_prefix("src/")
            .is_some_and(|file| template::sources(bench).iter().any(|(f, _)| *f == file));
    }
    // Every fingerprint covers the migrations of the core and of the modules.
    (rest.starts_with("migrations/") && template::is_dependency(name))
        || fingerprinted(bench).contains(&name)
}

/// The crates `bench` exercises, sorted: those of its fingerprint, those its measures go
/// through, and `kartenn-bench`.
///
/// # Panics
///
/// On a bench that has no preparation listed in [`template`].
pub fn exercised(bench: &Bench) -> Vec<&'static str> {
    let mut out = fingerprinted(bench.name);
    for measure in bench.measures {
        for path in paths(measure.name) {
            if let Some((name, _)) = crate_of(path)
                && !out.contains(&name)
            {
                out.push(name);
            }
        }
    }
    if !out.contains(&BENCH_CRATE) {
        out.push(BENCH_CRATE);
    }
    out.sort_unstable();
    out
}

/// What a bench runs for a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// Every measure, its template made again: the path is in its fingerprint.
    Prepared,
    /// Every measure.
    Whole,
    /// These measures, in the order of the catalogue; their prerequisites come with them
    /// (`--only`).
    Only(Vec<&'static str>),
}

/// What a change of one path calls for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touch {
    /// These benches, in the order of the catalogues.
    Benches(Vec<(&'static str, Run)>),
    /// Every bench, whole: the crate root of `kartenn-bench`, the manifest of the
    /// workspace.
    Every,
    /// The configuration of the benches: nothing run, a full run advised.
    Config,
    /// A bench script outside `kartenn-bench`.
    Proposed(&'static str),
    /// Nothing a bench measures.
    Nothing,
}

/// What a change of `path` (relative to the root of the repository) calls for.
pub fn touched(path: &str) -> Touch {
    let path = path.trim().trim_start_matches("./");
    if WORKSPACE.contains(&path) || CRATE_ROOT.contains(&path) {
        return Touch::Every;
    }
    if CONFIG.contains(&path) {
        return Touch::Config;
    }
    if let Some((_, script)) = PROPOSED.iter().find(|(start, _)| path.starts_with(start)) {
        return Touch::Proposed(script);
    }
    let Some((name, rest)) = crate_of(path) else {
        return Touch::Nothing;
    };
    if !is_code(rest) || UNMEASURED.contains(&path) {
        return Touch::Nothing;
    }
    let mut runs = Vec::new();
    for bench in BENCHES {
        if in_fingerprint(bench.name, name, rest) {
            runs.push((bench.name, Run::Prepared));
            continue;
        }
        // The measures that list the path, and those that start from the state they leave
        // (prerequisites come before their measures in a catalogue: one pass).
        let mut only: Vec<&'static str> = Vec::new();
        for m in bench.measures {
            if paths(m.name).iter().any(|listed| covers(listed, path))
                || m.after.iter().any(|a| only.contains(a))
            {
                only.push(m.name);
            }
        }
        if only.len() == bench.measures.len() {
            runs.push((bench.name, Run::Whole));
        } else if !only.is_empty() {
            runs.push((bench.name, Run::Only(only)));
        }
    }
    if !runs.is_empty() {
        Touch::Benches(runs)
    } else if name == BENCH_CRATE {
        // A module of the benches the table does not know: every bench, in doubt.
        Touch::Every
    } else {
        Touch::Nothing
    }
}

/// What a change calls for: the benches of `kartenn-bench` to run, in the order of the
/// catalogues, the scripts to propose, whether the configuration of the benches changed,
/// and one line per path that calls for something.
#[derive(Debug, Default)]
pub struct Affected {
    pub runs: Vec<(&'static str, Run)>,
    pub proposed: Vec<&'static str>,
    pub config: bool,
    pub explained: String,
    /// The paths that call for nothing.
    pub unconcerned: usize,
}

/// The line that explains what a path calls for.
fn explain(runs: &[(&'static str, Run)]) -> String {
    let named = |which: fn(&Run) -> bool| -> Vec<&str> {
        runs.iter()
            .filter(|(_, r)| which(r))
            .map(|(b, _)| *b)
            .collect()
    };
    let mut parts = Vec::new();
    let prepared = named(|r| *r == Run::Prepared);
    if !prepared.is_empty() {
        parts.push(format!(
            "bancs entiers (empreinte) : {}",
            prepared.join(", ")
        ));
    }
    let whole = named(|r| *r == Run::Whole);
    if !whole.is_empty() {
        parts.push(format!(
            "bancs entiers (toutes leurs mesures) : {}",
            whole.join(", ")
        ));
    }
    let only: Vec<&str> = runs
        .iter()
        .filter_map(|(_, r)| match r {
            Run::Only(m) => Some(m.iter().copied()),
            _ => None,
        })
        .flatten()
        .collect();
    if !only.is_empty() {
        parts.push(format!("mesures : {}", only.join(", ")));
    }
    parts.join(" ; ")
}

/// What the change of `paths` calls for.
pub fn affected<'a>(paths: impl IntoIterator<Item = &'a str>) -> Affected {
    let mut whole: BTreeSet<&'static str> = BTreeSet::new();
    let mut measures: BTreeSet<&'static str> = BTreeSet::new();
    let mut out = Affected::default();
    let mut seen = BTreeSet::new();
    for path in paths {
        let path = path.trim().trim_start_matches("./");
        if path.is_empty() || !seen.insert(path.to_owned()) {
            continue;
        }
        let line = match touched(path) {
            Touch::Benches(runs) => {
                for (bench, run) in &runs {
                    match run {
                        Run::Prepared | Run::Whole => {
                            whole.insert(bench);
                        }
                        Run::Only(m) => measures.extend(m.iter().copied()),
                    }
                }
                explain(&runs)
            }
            Touch::Every => {
                whole.extend(BENCHES.iter().map(|b| b.name));
                "bancs entiers : tous".to_owned()
            }
            Touch::Config => {
                out.config = true;
                CONFIG_WARNING.to_owned()
            }
            Touch::Proposed(script) => {
                if !out.proposed.contains(&script) {
                    out.proposed.push(script);
                }
                format!("proposé : {script} (hors kartenn-bench, non lancé)")
            }
            Touch::Nothing => {
                out.unconcerned += 1;
                continue;
            }
        };
        let _ = writeln!(out.explained, "{path} → {line}");
    }
    for bench in BENCHES {
        if whole.contains(bench.name) {
            out.runs.push((bench.name, Run::Whole));
            continue;
        }
        let only: Vec<&'static str> = bench
            .measures
            .iter()
            .map(|m| m.name)
            .filter(|m| measures.contains(m))
            .collect();
        if !only.is_empty() {
            out.runs.push((bench.name, Run::Only(only)));
        }
    }
    out
}

/// The script of the bench `name` (`bench-properties` for `props`).
pub fn script(name: &str) -> String {
    match name {
        "props" => "bench-properties".to_owned(),
        other => format!("bench-{other}"),
    }
}

impl Affected {
    /// One line per bench to run, for the script `bench-affected`: its script, then the
    /// measures for `--only` when not all of them run.
    pub fn plan(&self) -> String {
        let mut out = String::new();
        for (bench, run) in &self.runs {
            match run {
                Run::Only(m) => {
                    let _ = writeln!(out, "{} {}", script(bench), m.join(","));
                }
                Run::Prepared | Run::Whole => {
                    let _ = writeln!(out, "{}", script(bench));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;
    use crate::template::named_crates;

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("the root of the repository")
    }

    /// The modules of `kartenn-bench` and their text.
    fn modules() -> BTreeMap<String, String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "rs"))
            .map(|p| {
                let file = p.file_name().unwrap().to_string_lossy().into_owned();
                (file, std::fs::read_to_string(&p).unwrap())
            })
            .collect()
    }

    /// The modules a text refers to: `crate::<module>`, `crate::{<module>, …}`.
    fn refers_to(text: &str, modules: &BTreeMap<String, String>) -> BTreeSet<String> {
        let ident = |s: &str| -> String {
            s.trim()
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                .collect()
        };
        let mut out = BTreeSet::new();
        let mut rest = text;
        while let Some(at) = rest.find("crate::") {
            rest = &rest[at + "crate::".len()..];
            let names: Vec<String> = match rest.strip_prefix('{') {
                Some(group) => group
                    .split('}')
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .map(ident)
                    .collect(),
                None => vec![ident(rest)],
            };
            for name in names {
                let file = format!("{name}.rs");
                if modules.contains_key(&file) {
                    out.insert(file);
                }
            }
        }
        out
    }

    /// The modules neither the measures nor the derivation go through.
    const OUTSIDE: &[&str] = &[
        "affected.rs",
        "history.rs",
        "measures.rs",
        "lib.rs",
        "main.rs",
    ];

    /// The module that times `measure`: the one that names it.
    fn file_of(measure: &str, modules: &BTreeMap<String, String>) -> String {
        let quoted = format!("\"{measure}\"");
        let files: Vec<&String> = modules
            .iter()
            .filter(|(f, t)| !OUTSIDE.contains(&f.as_str()) && t.contains(&quoted))
            .map(|(f, _)| f)
            .collect();
        assert_eq!(files.len(), 1, "{measure}: named by {files:?}");
        files[0].clone()
    }

    /// The module of the catalogue of `bench`.
    fn home(bench: &Bench, modules: &BTreeMap<String, String>) -> String {
        let name = format!("name: \"{}\",", bench.name);
        let files: Vec<&String> = modules
            .iter()
            .filter(|(f, t)| {
                !OUTSIDE.contains(&f.as_str()) && t.contains("static BENCH") && t.contains(&name)
            })
            .map(|(f, _)| f)
            .collect();
        assert_eq!(files.len(), 1, "{}: catalogue in {files:?}", bench.name);
        files[0].clone()
    }

    /// The modules the code of `measure` goes through, from the module that times it. The
    /// module of a catalogue runs the measures of the other modules of its bench (`run`):
    /// those edges are followed only for their own measures.
    fn reach(measure: &str, modules: &BTreeMap<String, String>) -> BTreeSet<String> {
        let start = file_of(measure, modules);
        let mut dispatch: Vec<(String, String)> = Vec::new();
        for bench in BENCHES {
            let home = home(bench, modules);
            for m in bench.measures {
                let file = file_of(m.name, modules);
                if file != home {
                    dispatch.push((home.clone(), file));
                }
            }
        }
        let mut seen = BTreeSet::new();
        let mut todo = vec![start.clone()];
        while let Some(file) = todo.pop() {
            if OUTSIDE.contains(&file.as_str()) || !seen.insert(file.clone()) {
                continue;
            }
            for next in refers_to(&modules[&file], modules) {
                if next != start && dispatch.contains(&(file.clone(), next.clone())) {
                    continue;
                }
                todo.push(next);
            }
        }
        seen
    }

    /// The paths of `measure` of `bench`, derived from the sources: the modules it goes
    /// through, and the crates they name (but `template.rs`, which names the modules for
    /// their migrations, in every fingerprint), with their workspace dependencies, outside
    /// the fingerprint of the bench.
    fn derived(bench: &Bench, measure: &str, modules: &BTreeMap<String, String>) -> Vec<String> {
        let reached = reach(measure, modules);
        let mut named: Vec<String> = Vec::new();
        for file in reached.iter().filter(|f| *f != "template.rs") {
            named.extend(named_crates(&modules[file]));
        }
        let named: Vec<&str> = named.iter().map(String::as_str).collect();
        let fingerprint = fingerprinted(bench.name);
        let mut out: Vec<String> = reached
            .iter()
            .map(|f| format!("crates/kartenn-bench/src/{f}"))
            .collect();
        out.extend(
            template::crate_hashes(&named)
                .into_iter()
                .map(|(name, _)| name)
                .filter(|name| !fingerprint.contains(name))
                .map(|name| format!("crates/{name}/")),
        );
        out.sort();
        out
    }

    #[test]
    fn every_measure_has_its_paths_and_its_file() {
        let modules = modules();
        let names: Vec<&str> = BENCHES
            .iter()
            .flat_map(|b| b.measures.iter())
            .map(|m| m.name)
            .collect();
        for (measure, _) in PATHS {
            assert!(
                names.contains(measure),
                "{measure}: no measure of the catalogues"
            );
            assert_eq!(
                PATHS.iter().filter(|(m, _)| m == measure).count(),
                1,
                "{measure}: twice in the table"
            );
        }
        for name in names {
            let file = format!("crates/kartenn-bench/src/{}", file_of(name, &modules));
            assert!(paths(name).contains(&file.as_str()), "{name}: not {file}");
        }
    }

    /// The table is the one the sources give: a module or a crate a measure goes through,
    /// and missing from its paths, would leave a change of it unmeasured.
    #[test]
    fn the_table_is_derived_from_the_sources() {
        let modules = modules();
        for bench in BENCHES {
            for m in bench.measures {
                let mut listed: Vec<String> =
                    paths(m.name).into_iter().map(str::to_owned).collect();
                listed.sort();
                assert_eq!(listed, derived(bench, m.name, &modules), "{}", m.name);
            }
        }
    }

    #[test]
    fn the_derivation_follows_the_imports() {
        let modules = modules();
        assert_eq!(
            refers_to(
                "use crate::{props, Dataset}; crate::rbac::x; crate::nope; crate::Scale",
                &modules
            ),
            BTreeSet::from(["props.rs".to_owned(), "rbac.rs".to_owned()])
        );
        // The helpers of `core_budgets` used elsewhere do not run the measures of `core`
        // in other modules; its own `run` does, for them only.
        let props = reach("props.changeset-10k", &modules);
        assert!(props.contains("core_budgets.rs"), "{props:?}");
        assert!(!props.contains("machine_budgets.rs"), "{props:?}");
        let read = reach("core.read-object", &modules);
        assert!(!read.contains("vlan_budgets.rs"), "{read:?}");
        let stats = reach("core.stats", &modules);
        assert!(
            stats.contains("vlan_budgets.rs") && stats.contains("rbac.rs"),
            "{stats:?}"
        );
        // Every module is gone through by a measure, or is outside the measures.
        let reached: BTreeSet<String> = BENCHES
            .iter()
            .flat_map(|b| b.measures.iter())
            .flat_map(|m| reach(m.name, &modules))
            .collect();
        for file in modules.keys() {
            assert!(
                reached.contains(file) || OUTSIDE.contains(&file.as_str()),
                "{file}: no measure goes through it"
            );
        }
        for file in OUTSIDE {
            let path = format!("crates/kartenn-bench/src/{file}");
            assert!(
                UNMEASURED.contains(&path.as_str()) || CRATE_ROOT.contains(&path.as_str()),
                "{path}"
            );
        }
    }

    /// Outside `kartenn-bench`, the table lists whole crates only: no file of plumbing on
    /// its own.
    #[test]
    fn the_table_lists_modules_and_whole_crates() {
        for (measure, _) in PATHS {
            for path in paths(measure) {
                let (name, rest) =
                    crate_of(path).unwrap_or_else(|| panic!("{measure}: {path} is in no crate"));
                if name == BENCH_CRATE {
                    assert!(
                        rest.starts_with("src/") && root().join(path).is_file(),
                        "{measure}: {path}"
                    );
                } else {
                    assert!(
                        rest.is_empty() && root().join(path).is_dir(),
                        "{measure}: {path} is no whole crate"
                    );
                }
            }
        }
        for file in PLUMBING {
            assert!(root().join(file).is_file(), "{file}");
            assert!(
                PATHS.iter().all(|(m, _)| !paths(m).contains(file)),
                "{file}: plumbing, listed on its own"
            );
        }
    }

    /// Every crate a bench exercises is in its fingerprint or listed by one of its
    /// measures, and every workspace crate the benches build is exercised by one.
    #[test]
    fn every_crate_a_bench_exercises_is_covered() {
        let mut all: BTreeSet<&str> = BTreeSet::new();
        for bench in BENCHES {
            let fingerprint = fingerprinted(bench.name);
            for name in exercised(bench) {
                let dir = format!("crates/{name}/");
                assert!(
                    name == BENCH_CRATE
                        || fingerprint.contains(&name)
                        || bench
                            .measures
                            .iter()
                            .any(|m| paths(m.name).contains(&dir.as_str())),
                    "{}: exercises {name}, uncovered",
                    bench.name
                );
                all.insert(name);
            }
        }
        for (name, _) in template::crate_hashes(&["kartenn-server"]) {
            assert!(all.contains(name), "{name}: no bench exercises it");
        }
        let core = exercised(crate::measures::bench("core").unwrap());
        assert!(
            core.contains(&"kartenn-machines"),
            "by its measures: {core:?}"
        );
    }

    fn every(run: &Run) -> Vec<(&'static str, Run)> {
        BENCHES.iter().map(|b| (b.name, run.clone())).collect()
    }

    fn only(names: &[&'static str]) -> Run {
        Run::Only(names.to_vec())
    }

    const VLAN_MACHINES_STATS: &[&str] = &[
        "core.vlan-read",
        "core.vlan-filtered-page",
        "core.vlan-occupancy",
        "core.vlan-create",
        "core.machines-read",
        "core.machines-owner-page",
        "core.machines-site-page",
        "core.machines-network",
        "core.machines-create",
        "core.machines-suggestions",
        "core.machines-import",
        "core.stats",
    ];

    #[test]
    fn a_file_of_kartenn_flow_runs_the_benches_it_reaches() {
        // In the fingerprints of `http` (kartenn-server builds every module in), `policy`
        // and `flow`; gone through by every measure of `telemetry`.
        let expected = Touch::Benches(vec![
            ("http", Run::Prepared),
            ("policy", Run::Prepared),
            ("flow", Run::Prepared),
            ("telemetry", Run::Whole),
        ]);
        assert_eq!(touched("crates/kartenn-flow/src/read.rs"), expected);
        assert_eq!(touched("crates/kartenn-flow/src/model.rs"), expected);
        let a = affected(["crates/kartenn-flow/src/read.rs"]);
        assert_eq!(
            a.plan(),
            "bench-http\nbench-policy\nbench-flow\nbench-telemetry\n"
        );
        assert_eq!(
            a.explained,
            "crates/kartenn-flow/src/read.rs → bancs entiers (empreinte) : http, policy, flow \
             ; bancs entiers (toutes leurs mesures) : telemetry\n"
        );
    }

    #[test]
    fn the_core_its_data_and_the_migrations_run_every_bench() {
        for path in [
            "crates/kartenn-core/src/objects.rs",
            "crates/kartenn-core/data/iana/iana-ipv4-special-registry.csv",
            "crates/kartenn-service/src/pipeline.rs",
            "crates/kartenn-store-pg/src/rbac.rs",
            "crates/kartenn-store-pg/migrations/0999_probe.sql",
            "crates/kartenn-flow/migrations/0999_probe.sql",
        ] {
            assert_eq!(
                touched(path),
                Touch::Benches(every(&Run::Prepared)),
                "{path}"
            );
        }
    }

    #[test]
    fn a_file_of_the_router_runs_what_goes_through_it() {
        let expected = Touch::Benches(vec![
            ("core", only(VLAN_MACHINES_STATS)),
            ("http", Run::Prepared),
            ("policy", Run::Prepared),
            ("flow", Run::Prepared),
            ("events", Run::Prepared),
            ("telemetry", Run::Whole),
        ]);
        assert_eq!(touched("crates/kartenn-rest/src/cursor.rs"), expected);
        assert_eq!(touched("crates/kartenn-auth/src/tokens.rs"), expected);
        // The machines: their measures, the statistics, and what starts after them.
        assert_eq!(
            touched("crates/kartenn-machines/src/read.rs"),
            Touch::Benches(vec![
                ("core", only(&VLAN_MACHINES_STATS[4..])),
                ("http", Run::Prepared),
                ("policy", Run::Prepared),
                ("flow", Run::Prepared),
                ("telemetry", Run::Whole),
            ])
        );
        // The server: `http` prepares through it, `telemetry` measures through it.
        assert_eq!(
            touched("crates/kartenn-server/src/config.rs"),
            Touch::Benches(vec![("http", Run::Prepared), ("telemetry", Run::Whole)])
        );
    }

    #[test]
    fn the_modules_of_kartenn_bench() {
        // A source of a preparation: the whole benches it prepares; elsewhere, the measures
        // that go through it.
        assert_eq!(
            touched("crates/kartenn-bench/src/vlan_budgets.rs"),
            Touch::Benches(vec![
                ("core", only(VLAN_MACHINES_STATS)),
                ("policy", Run::Prepared),
                ("flow", Run::Prepared),
            ])
        );
        // A module of measures alone: its measures, and those that start after them.
        assert_eq!(
            touched("crates/kartenn-bench/src/machine_budgets.rs"),
            Touch::Benches(vec![("core", only(&VLAN_MACHINES_STATS[4..]))])
        );
        assert_eq!(
            touched("crates/kartenn-bench/src/stats_budgets.rs"),
            Touch::Benches(vec![("core", only(&["core.stats"]))])
        );
        // Shared helpers: every bench that imports them.
        assert_eq!(
            touched("crates/kartenn-bench/src/core_budgets.rs"),
            Touch::Benches(vec![
                ("props", Run::Whole),
                ("core", Run::Prepared),
                ("rbac", Run::Prepared),
                ("http", Run::Prepared),
                ("policy", Run::Prepared),
                ("flow", Run::Prepared),
                ("events", Run::Whole),
                ("telemetry", Run::Prepared),
            ])
        );
        assert_eq!(
            touched("crates/kartenn-bench/src/rbac.rs"),
            Touch::Benches(vec![
                ("core", only(VLAN_MACHINES_STATS)),
                ("rbac", Run::Prepared),
                ("http", Run::Prepared),
                ("policy", Run::Whole),
                ("flow", Run::Prepared),
                ("events", Run::Prepared),
            ])
        );
        // The crate root, the commands, the manifest: every bench.
        for path in [
            "crates/kartenn-bench/src/main.rs",
            "crates/kartenn-bench/src/lib.rs",
            "crates/kartenn-bench/Cargo.toml",
        ] {
            assert_eq!(touched(path), Touch::Every, "{path}");
        }
        // Its tests, the selection, the history and this table: nothing.
        for path in [
            "crates/kartenn-bench/tests/it/only.rs",
            "crates/kartenn-bench/src/measures.rs",
            "crates/kartenn-bench/src/history.rs",
            "crates/kartenn-bench/src/affected.rs",
        ] {
            assert_eq!(touched(path), Touch::Nothing, "{path}");
        }
    }

    #[test]
    fn a_whole_bench_takes_over_its_measures() {
        let a = affected([
            "crates/kartenn-bench/src/stats_budgets.rs",
            "crates/kartenn-machines/src/read.rs",
        ]);
        assert_eq!(
            a.runs,
            [
                ("core", only(&VLAN_MACHINES_STATS[4..])),
                ("http", Run::Whole),
                ("policy", Run::Whole),
                ("flow", Run::Whole),
                ("telemetry", Run::Whole),
            ]
        );
        assert_eq!(
            a.plan().lines().next().unwrap(),
            format!("bench-core {}", VLAN_MACHINES_STATS[4..].join(","))
        );
        let a = affected([
            "crates/kartenn-core/src/objects.rs",
            "crates/kartenn-bench/src/stats_budgets.rs",
        ]);
        assert_eq!(a.runs, every(&Run::Whole));
    }

    #[test]
    fn web_documentation_and_configuration_run_nothing() {
        let a = affected([
            "web/v2/ui/src/components/Button.vue",
            "web/shell/src/main.ts",
            "clients/python/bench/cli.py",
            "docs/adr/0041-bancs-cibles.md",
            "specs/028-bancs-cibles/tasks.md",
            "crates/kartenn-store-pg/tests/it/main.rs",
            "crates/kartenn-dev-idp/src/main.rs",
            "devenv.nix",
            "devenv.lock",
        ]);
        assert!(a.runs.is_empty(), "{:?}", a.runs);
        assert_eq!(a.proposed, ["bench-ui", "bench-web", "bench-cli"]);
        assert!(a.config);
        assert_eq!(a.unconcerned, 4);
        assert_eq!(a.plan(), "");
        assert!(
            a.explained
                .contains(&format!("devenv.nix → {CONFIG_WARNING}"))
        );
    }

    #[test]
    fn the_lock_runs_every_bench_and_paths_are_read_once() {
        let a = affected(["Cargo.lock", "./Cargo.lock", "", "Cargo.toml"]);
        assert_eq!(a.runs, every(&Run::Whole));
        assert_eq!(a.explained.lines().count(), 2, "{}", a.explained);
    }

    #[test]
    fn scripts_are_those_of_devenv() {
        let devenv = std::fs::read_to_string(root().join("devenv.nix")).unwrap();
        for bench in BENCHES {
            let script = script(bench.name);
            assert!(
                devenv.contains(&format!("scripts.{script} =\n    bench \"{}\"", bench.name)),
                "{script}: no script of devenv.nix runs {}",
                bench.name
            );
        }
        for (_, script) in PROPOSED {
            assert!(
                devenv.contains(&format!("scripts.{script} =")),
                "{script}: no such script"
            );
        }
    }
}
