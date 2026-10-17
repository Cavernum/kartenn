//! Panics in the logs (spec 012, REQ-012-28, 33; ASVS 16.5.4): an ERROR `sys.panic` with
//! the message, the place and the trace of the current span, then the report of the
//! default handler on the standard error, with the credentials of its URLs masked.

use std::backtrace::{Backtrace, BacktraceStatus};
use std::panic::PanicHookInfo;

use crate::capture::redact_credentials;

fn message(info: &PanicHookInfo<'_>) -> String {
    info.payload()
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_owned())
}

/// Installs the hook in place of the default handler, which would write the message
/// unmasked; the report keeps its form (thread, place, message, backtrace under
/// `RUST_BACKTRACE`).
pub fn install() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let message = redact_credentials(&message(info));
        tracing::error!(
            name: "sys.panic",
            location = %location,
            panic = %message,
            "a task panicked"
        );
        let thread = std::thread::current();
        eprintln!(
            "thread '{}' panicked at {location}:\n{message}",
            thread.name().unwrap_or("<unnamed>")
        );
        let backtrace = Backtrace::capture();
        if backtrace.status() == BacktraceStatus::Captured {
            eprintln!("stack backtrace:\n{backtrace}");
        } else {
            eprintln!(
                "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"
            );
        }
    }));
}
