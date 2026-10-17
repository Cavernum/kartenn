//! A panic is logged before the default handler (spec 012, REQ-012-33; task 012-18).

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
        record.fields["location"].contains("tests/panic.rs"),
        "{:?}",
        record.fields
    );
}
