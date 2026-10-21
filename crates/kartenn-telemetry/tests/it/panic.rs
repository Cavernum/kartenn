//! A panic is logged, and reported on the standard error with its secrets masked (spec
//! 012, REQ-012-28, 33; tasks 012-18, 20).

use kartenn_telemetry::capture::{CaptureLayer, Output};
use tracing_subscriber::layer::SubscriberExt;

#[test]
fn a_panic_is_logged_with_its_place() {
    kartenn_telemetry::panic::install();
    let (output, mut receiver) = Output::new("stdout", "error", 16).unwrap();
    let subscriber = tracing_subscriber::registry().with(CaptureLayer::new(vec![output]));
    let caught = tracing::subscriber::with_default(subscriber, || {
        std::panic::catch_unwind(|| panic!("invariant broken: {}", 42))
    });
    assert!(caught.is_err());
    let record = receiver.try_recv().unwrap();
    assert_eq!(record.name, "sys.panic");
    assert_eq!(record.fields["panic"], "invariant broken: 42");
    assert!(
        record.fields["location"].contains("tests/it/panic.rs"),
        "{:?}",
        record.fields
    );
}

/// The panic of `the_report_on_stderr_is_masked`, in a process of its own.
#[test]
fn panicking_child() {
    if std::env::var_os("KARTENN_PANIC_CHILD").is_none() {
        return;
    }
    kartenn_telemetry::panic::install();
    panic!("cannot connect to postgres://kartenn:canary-panic@db/kartenn");
}

#[test]
fn the_report_on_stderr_is_masked() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        // Its path in the one integration binary of the crate (ADR 0027).
        .args(["--exact", "panic::panicking_child", "--nocapture"])
        .env("KARTENN_PANIC_CHILD", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("panicked at"), "{stderr}");
    assert!(
        stderr.contains("postgres://kartenn:***@db/kartenn"),
        "{stderr}"
    );
    assert!(!stderr.contains("canary-panic"), "REQ-012-28: {stderr}");
}
