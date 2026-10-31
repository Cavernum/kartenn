//! A measure run alone (`--only`) gives the verdict it gives in the whole bench, at the
//! scale of the tests (ADR 0041, decision 1; task 028-1): for each bench, a measure with
//! prerequisites or a setup of its own, whose forgotten prerequisite would show.
//!
//! Each run is the binary on an empty database of its own in the cluster of
//! `DATABASE_URL`, named as the databases of the tests (ADR 0028): `kartenn-testing-clean`
//! drops those a failed test leaves.

use std::collections::BTreeMap;
use std::process::Command;

use sqlx::{Connection, PgConnection};

/// The last line of a run at the scale of the tests, written after the verdicts.
const CAVEAT: &str = "échelle de test : ne vaut pas mesure de budget";

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

/// `kartenn-bench <bench> <url> 42 small [--only <only>]` on an empty database; its standard
/// output. The database is dropped when the run went to its end.
fn run(bench: &str, only: Option<&str>) -> String {
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let name = kartenn_testing::database_name(&format!(
        "kartenn_bench::only::{bench}::{}",
        only.unwrap_or("*")
    ));
    let mut conn = runtime.block_on(async {
        let mut conn = PgConnection::connect_with(&kartenn_testing::master_options())
            .await
            .expect("connecting to DATABASE_URL");
        kartenn_testing::create_empty(&mut conn, &name)
            .await
            .expect("creating the database of the bench");
        conn
    });
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
    runtime.block_on(async {
        kartenn_testing::drop_database(&mut conn, &name)
            .await
            .expect("dropping the database of the bench");
        let _ = conn.close().await;
    });
    stdout
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

/// The verdicts of a report by budget, in their order: `PASS` and `FAIL` lines.
fn verdicts(report: &str) -> BTreeMap<String, Vec<&'static str>> {
    let mut verdicts: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
    for line in report.lines() {
        let verdict = match line.split_once(' ') {
            Some(("PASS", rest)) => ("PASS", rest),
            Some(("FAIL", rest)) => ("FAIL", rest),
            _ => continue,
        };
        verdicts.entry(key(verdict.1)).or_default().push(verdict.0);
    }
    verdicts
}

/// `measure` alone gives the verdicts it gives in the whole bench on `budgets`, its lines,
/// and fewer verdicts than the whole bench. The last verdict of a budget counts: the
/// modelled overhead in `telemetry`, whose first table may turn a line into `INFO` with the
/// noise. The verdicts of the prerequisites are not compared: some are noise at this scale
/// (the control of the inference of `policy`, in the order of its budget).
fn alone_as_in_the_whole(bench: &str, measure: &str, budgets: &[&str]) {
    let whole = run(bench, None);
    let alone = run(bench, Some(measure));
    assert!(
        alone
            .lines()
            .any(|l| l.starts_with(&format!("mesures : {measure}"))),
        "{alone}"
    );
    let (all, only) = (verdicts(&whole), verdicts(&alone));
    assert!(
        only.values().map(Vec::len).sum::<usize>() < all.values().map(Vec::len).sum(),
        "{measure} ran every measure:\n{alone}"
    );
    for budget in budgets {
        let last = |verdicts: &BTreeMap<String, Vec<&'static str>>| {
            verdicts.get(*budget).and_then(|v| v.last().copied())
        };
        let (in_whole, in_alone) = (last(&all), last(&only));
        assert!(in_alone.is_some(), "{budget}: no verdict alone\n{alone}");
        assert_eq!(
            in_alone, in_whole,
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
        &["origins of 3 domains removed (P10)"],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn core() {
    alone_as_in_the_whole(
        "core",
        "core.machines-read",
        &["read a machine with its interfaces and addresses, p95"],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn rbac() {
    alone_as_in_the_whole(
        "rbac",
        "rbac.read-past",
        &["read an object as known in the past, p95"],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn http() {
    alone_as_in_the_whole(
        "http",
        "http.read-past",
        &["read an object as known in the past, p95"],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn policy() {
    alone_as_in_the_whole(
        "policy",
        "policy.inference-preview",
        &["preview of a rule of inference in its simulation, added"],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn flow() {
    alone_as_in_the_whole(
        "flow",
        "flow.nonconformities-page",
        &["page of non-conformities, p95"],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn events() {
    alone_as_in_the_whole(
        "events",
        "events.storm",
        &[
            "resumption of every stream of an instance",
            "unitary changeset during the storm, p95 (INV-34)",
        ],
    );
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn telemetry() {
    alone_as_in_the_whole(
        "telemetry",
        "telemetry.filtered-page",
        &["filtered page of 100"],
    );
}

/// `--list` prints the catalogue; an unknown name is refused with it, before any database
/// is touched.
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
    let refused = bench()
        .args([
            "props",
            "postgres://nowhere.invalid/x",
            "42",
            "small",
            "--only",
            "props.nope",
        ])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let error = String::from_utf8_lossy(&refused.stderr);
    assert!(
        error.contains("`props.nope` is no measure of the bench `props`"),
        "{error}"
    );
    assert!(error.contains(list.trim_end()), "{error}");
}
