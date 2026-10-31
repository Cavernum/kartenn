//! A measure run alone (`--only`) measures what it measures in the whole bench, at the
//! scale of the tests (ADR 0041, decision 1; task 028-1): for each bench, a measure with
//! prerequisites or a setup of its own, whose forgotten prerequisite would show in the
//! counts of its report (objects changed, nodes in scope, flows concerned…).
//!
//! The timings of the small scale move with the load of the machine: lines are compared
//! with their durations, percentages and load averages blanked, not by their verdicts.
//! The runs take one bench at a time (an advisory lock of the cluster), each on an empty
//! database of its own in the cluster of `DATABASE_URL`, named as the databases of the
//! tests (ADR 0028): `kartenn-testing-clean` drops those a failed test leaves.

use std::collections::BTreeMap;
use std::process::Command;

use sqlx::{Connection, PgConnection};
use tokio::runtime::Runtime;

/// The last line of a run at the scale of the tests, written after the verdicts.
const CAVEAT: &str = "échelle de test : ne vaut pas mesure de budget";
/// The advisory lock that runs the benches of these tests one at a time (`kb_only`).
const SERIAL: i64 = 0x006b_625f_6f6e_6c79;

/// `DATABASE_URL` aimed at the database `name`.
fn url_of(name: &str) -> String {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let (url, query) = url
        .split_once('?')
        .map_or((url.as_str(), None), |(u, q)| (u, Some(q)));
    let (server, _) = url.rsplit_once('/').expect("DATABASE_URL names a database");
    match query {
        Some(query) => format!("{server}/{name}?{query}"),
        None => format!("{server}/{name}"),
    }
}

/// A connection to the cluster that holds [`SERIAL`] until it is dropped.
struct Serial {
    runtime: Runtime,
    conn: PgConnection,
}

impl Serial {
    fn take() -> Self {
        let runtime = Runtime::new().expect("a runtime");
        let conn = runtime.block_on(async {
            let mut conn = PgConnection::connect_with(&kartenn_testing::master_options())
                .await
                .expect("connecting to DATABASE_URL");
            sqlx::query("SELECT pg_advisory_lock($1)")
                .bind(SERIAL)
                .execute(&mut conn)
                .await
                .expect("taking the lock of the benches of the tests");
            conn
        });
        Self { runtime, conn }
    }

    /// `kartenn-bench <bench> <url> 42 small [--only <only>]` on an empty database; its
    /// standard output. The database is dropped when the run went to its end.
    fn run(&mut self, bench: &str, only: Option<&str>) -> String {
        let name = kartenn_testing::database_name(&format!(
            "kartenn_bench::only::{bench}::{}",
            only.unwrap_or("*")
        ));
        self.runtime
            .block_on(kartenn_testing::create_empty(&mut self.conn, &name))
            .expect("creating the database of the bench");
        let mut command = Command::new(env!("CARGO_BIN_EXE_kartenn-bench"));
        command
            .args([bench, &url_of(&name), "42", "small"])
            .env_remove("K_BENCH_SCALE");
        if let Some(only) = only {
            command.args(["--only", only]);
        }
        let output = command.output().expect("running kartenn-bench");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            stdout.lines().any(|l| l == CAVEAT),
            "{bench} {only:?} did not reach its verdicts:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        self.runtime
            .block_on(kartenn_testing::drop_database(&mut self.conn, &name))
            .expect("dropping the database of the bench");
        stdout
    }
}

/// The name a line of the report gives its budget: up to its first `: `, or to the columns
/// of the tables of `telemetry`.
fn key(rest: &str) -> String {
    let end = rest
        .find(": ")
        .or_else(|| rest.find("  "))
        .unwrap_or(rest.len());
    rest[..end].trim().to_owned()
}

/// A word that moves with the load: a duration (`1.2s`, `40ms`, `0ns`), a decimal, a
/// percentage, whether a target was met.
fn moves(word: &str) -> bool {
    let core = word.trim_matches(|c: char| "(),;:<+".contains(c));
    let numeric = |n: &str| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit() || c == '.');
    (core.contains('.') && core.chars().any(|c| c.is_ascii_digit()))
        || core.ends_with('%')
        || ["ns", "µs", "ms", "s"]
            .iter()
            .any(|unit| core.strip_suffix(unit).is_some_and(numeric))
        || core == "met"
        || core == "missed"
}

/// A line of the report without its verdict, its moving words blanked.
fn steady(rest: &str) -> String {
    rest.split_whitespace()
        .map(|w| if moves(w) { "#" } else { w })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `PASS` and `FAIL` lines of a report by budget, in their order, made [`steady`].
fn budgets(report: &str) -> BTreeMap<String, Vec<String>> {
    let mut budgets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in report.lines() {
        if let Some(("PASS" | "FAIL", rest)) = line.split_once(' ') {
            budgets.entry(key(rest)).or_default().push(steady(rest));
        }
    }
    budgets
}

/// `only` alone reports fewer budgets than the whole bench, all of them in the whole
/// bench; on `same`, the same line as there, its timings blanked (the last of a budget:
/// the modelled overhead of `telemetry`); `present` there and alone (a line whose counts
/// follow the draws, which differ alone).
fn alone_as_in_the_whole(bench: &str, only: &str, same: &[&str], present: &[&str]) {
    let mut serial = Serial::take();
    let whole = serial.run(bench, None);
    let alone = serial.run(bench, Some(only));
    let asked = format!("mesures : {}", only.replace(',', ", "));
    assert!(alone.lines().any(|l| l.starts_with(&asked)), "{alone}");
    let (all, mine) = (budgets(&whole), budgets(&alone));
    let lines = |b: &BTreeMap<String, Vec<String>>| b.values().map(Vec::len).sum::<usize>();
    assert!(
        lines(&mine) < lines(&all),
        "{only} ran every measure:\n{alone}"
    );
    for budget in mine.keys() {
        assert!(
            all.contains_key(budget),
            "{budget}: alone, not in the whole bench\n{whole}"
        );
    }
    for budget in same {
        let last =
            |b: &BTreeMap<String, Vec<String>>| b.get(*budget).and_then(|v| v.last()).cloned();
        let in_alone = last(&mine);
        assert!(in_alone.is_some(), "{budget}: not reported alone\n{alone}");
        assert_eq!(
            in_alone,
            last(&all),
            "{budget}: alone\n{alone}\nin the whole bench\n{whole}"
        );
    }
    for budget in present {
        assert!(
            mine.contains_key(*budget) && all.contains_key(*budget),
            "{budget}: alone\n{alone}\nin the whole bench\n{whole}"
        );
    }
}

// The benches at the scale of the tests take minutes and a database each: ignored by
// `k-check`, run with `k-check -p kartenn-bench --run-ignored all`.

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn props() {
    alone_as_in_the_whole(
        "props",
        "props.origins-3-domains",
        &[
            "changeset, differential up to 10^4",
            "value of every root (up to 10^6)",
            "origins of 3 domains removed (P10)",
        ],
        &[],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn core() {
    alone_as_in_the_whole(
        "core",
        "core.machines-suggestions,core.stats",
        &[
            "children of a container of 10^4, page of 100, p95",
            "import",
            "stats, reader of everything, route from the snapshot, p95",
            "stats, 1 000 bindings, computation, p95",
        ],
        &[
            "suggest addresses, p95",
            "suggest prefixes, p95",
            "suggest machines as the reader of one owner, p95",
            "suggest VLANs, p95",
        ],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn core_machines() {
    alone_as_in_the_whole(
        "core",
        "core.machines-read,core.machines-network",
        &[
            "read a machine with its interfaces and addresses, p95",
            "import",
        ],
        &["machines of a /24, p95"],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn rbac() {
    alone_as_in_the_whole(
        "rbac",
        "rbac.read-past",
        &[
            "read an object, p95",
            "read an object as known in the past, p95",
        ],
        &[],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn http() {
    alone_as_in_the_whole(
        "http",
        "http.read-past",
        &[
            "read an object, p95",
            "read an object as known in the past, p95",
        ],
        &[],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn policy() {
    alone_as_in_the_whole(
        "policy",
        "policy.inference-preview",
        &[
            "control of the inference on a changeset of 10⁴ objects, added",
            "preview of a rule of inference in its simulation, added",
        ],
        &[],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn flow() {
    alone_as_in_the_whole(
        "flow",
        "flow.rule-written",
        &[
            "request a flow interface x interface 16 x 16, triage included, p95",
            "re-evaluate the flows of a reclassified container, added",
            "re-evaluate the flows of a flow type on a rule written",
        ],
        &[],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn events() {
    alone_as_in_the_whole(
        "events",
        "events.storm",
        &[
            "memory per idle stream",
            "resumption of every stream of an instance",
            "unitary changeset during the storm, p95 (INV-34)",
        ],
        &[],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn telemetry() {
    alone_as_in_the_whole(
        "telemetry",
        "telemetry.filtered-page",
        &["filtered page of 100"],
        &[],
    );
}

#[test]
fn steady_lines_blank_what_moves() {
    assert_eq!(
        steady("read an object, p95: 3.2ms with access control, budget 6ms (2ms without, +50.0 %)"),
        "read an object, p95: # with access control, budget # # without, # #"
    );
    assert_eq!(
        steady("x: 0ns (limit 1s; 30001 nodes in scope; load 6.59; target < 10ms: met)"),
        "x: # (limit # 30001 nodes in scope; load # target < # #"
    );
    assert_eq!(
        steady("filtered page of 100   12   3    1.20%   2.31%"),
        "filtered page of 100 12 3 # #"
    );
}

/// `--list` prints the catalogue, `--list --only` the measures that would run; an unknown
/// name is refused with the catalogue, before any database is touched.
#[test]
fn list_and_unknown_names() {
    let bench = || Command::new(env!("CARGO_BIN_EXE_kartenn-bench"));
    let list = bench().args(["props", "--list"]).output().unwrap();
    assert!(list.status.success());
    let list = String::from_utf8(list.stdout).unwrap();
    assert!(
        list.starts_with("props.origins-every-root (de préparation)\n"),
        "{list}"
    );
    assert!(list.contains("props.origins-3-domains (après props.value-every-root)\n"));
    let selected = bench()
        .args(["props", "--list", "--only", "props.value-every-root"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(selected.stdout).unwrap(),
        "mesures : props.value-every-root ; préalables : props.changeset-10k\n"
    );
    for args in [
        &[
            "props",
            "postgres://nowhere.invalid/x",
            "42",
            "small",
            "--only",
            "props.nope",
        ][..],
        &["props", "--list", "--only", "props.nope"],
    ] {
        let refused = bench().args(args).output().unwrap();
        assert!(!refused.status.success(), "{args:?}");
        let error = String::from_utf8_lossy(&refused.stderr);
        assert!(
            error.contains("`props.nope` is no measure of the bench `props`"),
            "{error}"
        );
        assert!(error.contains(list.trim_end()), "{error}");
    }
}
