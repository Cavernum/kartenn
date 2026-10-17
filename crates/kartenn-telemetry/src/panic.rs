//! Panics in the logs (spec 012, REQ-012-33; ASVS 16.5.4): before the default handler, an
//! ERROR `sys.panic` with the message, the place and the trace of the current span.

use std::panic::PanicHookInfo;

fn message(info: &PanicHookInfo<'_>) -> String {
    info.payload()
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_owned())
}

/// Installs the hook, keeping the previous one after it.
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        tracing::error!(
            name: "sys.panic",
            location = %location,
            panic = %message(info),
            "a task panicked"
        );
        previous(info);
    }));
}
