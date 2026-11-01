//! The prepared database kept as a template (ADR 0041, decision 2; task 028-2), at the
//! scale of the tests: a second run copies the template instead of preparing, and measures
//! and reports the same; a measure « de préparation » prepares again and replaces the
//! template; three templates are kept per bench; a client connected to the template or to
//! the prepared database stops the run, and the error names it.
//!
//! The templates live in the cluster of `DATABASE_URL` for the time of a test, under the
//! lock of the benches of the tests; each test drops those of its bench before and after
//! (`kartenn-testing-clean` drops those a failed test leaves).

use std::process::{Command, Output};

use sqlx::Connection;
use sqlx::postgres::PgConnection;

use crate::only::{Serial, budgets, database, url_of};

/// The line of the report that says how the run got its prepared database.
fn prepared(report: &str) -> &str {
    report
        .lines()
        .find(|l| l.starts_with("INFO prepared database: "))
        .unwrap_or_else(|| panic!("no line on the prepared database:\n{report}"))
}

/// The `INFO` lines of a report but the one on the prepared database.
fn info(report: &str) -> Vec<&str> {
    report
        .lines()
        .filter(|l| l.starts_with("INFO ") && !l.starts_with("INFO prepared database: "))
        .collect()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A run that went to its verdicts: its report, and whether it registered a template.
fn verdicts(bench: &str, output: &Output) -> (String, bool) {
    let (stdout, stderr) = (text(&output.stdout), text(&output.stderr));
    assert!(output.status.success(), "{bench}:\n{stdout}\n{stderr}");
    let registered = stderr
        .lines()
        .any(|l| l.starts_with("template kartenn_bench_tpl_") && l.ends_with(" registered"));
    (stdout, registered)
}

/// Creates the empty database `name` of the cluster, from `template0`.
fn create(serial: &mut Serial, name: &str) {
    serial
        .runtime
        .block_on(
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                r#"CREATE DATABASE "{name}" TEMPLATE template0"#
            )))
            .execute(&mut serial.conn),
        )
        .expect("creating a database");
}

// Benches at the small scale: ignored by `k-check`, run with
// `k-check -p kartenn-bench --run-ignored all`.
#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn a_second_run_copies_the_template() {
    let mut serial = Serial::take();
    serial.drop_templates("props");
    let only = Some("props.origins-3-domains");

    let (first, registered) = verdicts("props", &serial.output("props", only, &["--template"]));
    assert!(registered, "{first}");
    let kept = serial.templates("props");
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(!kept[0].1, "a template accepts no connection");
    assert!(prepared(&first).contains("warmed by"), "{first}");

    let (second, registered) = verdicts("props", &serial.output("props", only, &["--template"]));
    assert!(!registered, "{second}");
    assert!(
        prepared(&second).contains(&format!("copy of {}", kept[0].0)),
        "{second}"
    );
    assert_eq!(budgets(&first), budgets(&second), "{first}\n{second}");

    // A measure « de préparation » prepares again, and replaces the template.
    let forced = serial.output("props", Some("props.origins-every-root"), &["--template"]);
    let (forced, registered) = verdicts("props", &forced);
    assert!(registered, "{forced}");
    assert!(
        budgets(&forced).contains_key("origins of every root (P10)"),
        "{forced}"
    );
    assert_eq!(serial.templates("props"), kept);

    // A client connected to the template: not copied, and the error names it.
    let name = kept[0].0.clone();
    let open = serial.runtime.block_on(async {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            r#"ALTER DATABASE "{name}" WITH ALLOW_CONNECTIONS true"#
        )))
        .execute(&mut serial.conn)
        .await
        .expect("opening the template");
        PgConnection::connect_with(&kartenn_testing::master_options().database(&name))
            .await
            .expect("connecting to the template")
    });
    let refused = serial.output("props", only, &["--template"]);
    let stderr = text(&refused.stderr);
    assert!(!refused.status.success(), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "the template {name} has 1 client connection(s) open"
        )),
        "{stderr}"
    );
    serial
        .runtime
        .block_on(open.close())
        .expect("closing the connection");
    serial.drop_templates("props");
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn a_copied_run_reports_what_the_preparation_reported() {
    let mut serial = Serial::take();
    serial.drop_templates("core");
    let only = Some("core.read-object");
    let (first, registered) = verdicts("core", &serial.output("core", only, &["--template"]));
    assert!(registered, "{first}");
    let (second, registered) = verdicts("core", &serial.output("core", only, &["--template"]));
    assert!(!registered, "{second}");
    serial.drop_templates("core");
    assert!(
        info(&first)
            .iter()
            .any(|l| l.starts_with("INFO checkpoint before the measures: ")),
        "{first}"
    );
    assert_eq!(info(&first), info(&second), "{first}\n{second}");
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn the_three_newest_templates_are_kept() {
    let mut serial = Serial::take();
    serial.drop_templates("props");
    let older: Vec<String> = (1..=3)
        .map(|i| format!("kartenn_bench_tpl_props_{i:016x}"))
        .collect();
    for name in &older {
        create(&mut serial, name);
    }
    let (report, registered) = verdicts(
        "props",
        &serial.output("props", Some("props.origins-3-domains"), &["--template"]),
    );
    assert!(registered, "{report}");
    let kept: Vec<String> = serial
        .templates("props")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    serial.drop_templates("props");
    assert_eq!(kept.len(), 3, "{kept:?}");
    assert_eq!(kept[..2], older[1..], "the oldest dropped: {kept:?}");
    assert!(!older.contains(&kept[2]), "the new one kept: {kept:?}");
}

#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn a_client_on_the_prepared_database_stops_the_run() {
    let mut serial = Serial::take();
    serial.drop_templates("events");
    let only = Some("events.visibility");
    let name = database("events", only);
    let open = serial.runtime.block_on(async {
        kartenn_testing::create_empty(&mut serial.conn, &name)
            .await
            .expect("creating the database of the bench");
        PgConnection::connect_with(&kartenn_testing::master_options().database(&name))
            .await
            .expect("connecting to the database of the bench")
    });
    let refused = Command::new(env!("CARGO_BIN_EXE_kartenn-bench"))
        .args(["events", &url_of(&name), "42", "small", "--template"])
        .args(["--only", "events.visibility"])
        .env_remove("K_BENCH_SCALE")
        .output()
        .expect("running kartenn-bench");
    serial.runtime.block_on(async {
        open.close().await.expect("closing the connection");
        kartenn_testing::drop_database(&mut serial.conn, &name)
            .await
            .expect("dropping the database of the bench");
    });
    let left = serial.templates("events");
    serial.drop_templates("events");
    let stderr = text(&refused.stderr);
    assert!(!refused.status.success(), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "the prepared database {name} has 1 client connection(s) open"
        )),
        "{stderr}"
    );
    assert!(left.is_empty(), "no template registered: {left:?}");
}

#[test]
fn template_is_refused_outside_the_databases_of_the_benches() {
    let output = Command::new(env!("CARGO_BIN_EXE_kartenn-bench"))
        .args([
            "events",
            "postgres://bench@127.0.0.1:1/kartenn",
            "42",
            "small",
            "--template",
        ])
        .env_remove("K_BENCH_SCALE")
        .output()
        .expect("running kartenn-bench");
    let stderr = text(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("not `kartenn`"), "{stderr}");
}
