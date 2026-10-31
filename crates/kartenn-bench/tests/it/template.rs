//! The prepared database kept as a template (ADR 0041, decision 2; task 028-2), at the
//! scale of the tests: a second run copies the template instead of preparing and measures
//! the same; a measure « de préparation » prepares again and replaces the template; a
//! template with a connection open is not copied, and the error says so.
//!
//! The templates of `props` live in the cluster of `DATABASE_URL` for the time of the test,
//! under the lock of the benches of the tests; it drops them before and after.

use sqlx::Connection;
use sqlx::postgres::PgConnection;

use crate::only::{Serial, budgets};

/// The line of the report that says how the run got its prepared database.
fn prepared(report: &str) -> &str {
    report
        .lines()
        .find(|l| l.starts_with("INFO prepared database: "))
        .unwrap_or_else(|| panic!("no line on the prepared database:\n{report}"))
}

// A bench at the small scale, three times: ignored by `k-check`, run with
// `k-check -p kartenn-bench --run-ignored all`.
#[test]
#[ignore = "benches at the small scale: k-check -p kartenn-bench --run-ignored all"]
fn a_second_run_copies_the_template() {
    let mut serial = Serial::take();
    serial.drop_templates("props");
    let only = Some("props.origins-3-domains");

    let first = serial.run_with("props", only, &["--template"]);
    assert!(prepared(&first).contains("template registered"), "{first}");
    let registered = serial.templates("props");
    assert_eq!(registered.len(), 1, "{registered:?}");
    assert!(!registered[0].1, "a template accepts no connection");

    let second = serial.run_with("props", only, &["--template"]);
    let line = prepared(&second);
    assert!(
        line.contains(&format!("copy of {}", registered[0].0)) && !line.contains("registered"),
        "{second}"
    );
    assert_eq!(budgets(&first), budgets(&second), "{first}\n{second}");

    // A measure « de préparation » prepares again, and replaces the template.
    let forced = serial.run_with("props", Some("props.origins-every-root"), &["--template"]);
    assert!(
        prepared(&forced).contains("template registered"),
        "{forced}"
    );
    assert!(
        budgets(&forced).contains_key("origins of every root (P10)"),
        "{forced}"
    );
    assert_eq!(serial.templates("props").len(), 1);

    // A connection open on the template: not copied, and the error names it.
    let name = registered[0].0.clone();
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
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(!refused.status.success(), "{stderr}");
    assert!(
        stderr.contains(&format!("the template {name} has 1 connection(s) open")),
        "{stderr}"
    );
    serial
        .runtime
        .block_on(open.close())
        .expect("closing the connection");
    serial.drop_templates("props");
}
