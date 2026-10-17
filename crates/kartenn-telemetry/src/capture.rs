//! Capture of the technical logs (spec 012, REQ-012-4, 5, 22, 28, 29): each `tracing`
//! event is turned once into a [`Record`] (severity, name, fields; secrets masked, values
//! bounded), then handed to the bounded queue of every output whose filter takes it. A full
//! queue drops the record and counts it: the code that logs never waits for an output.
//! Formatting and writing happen in the task of each output ([`drive`]).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock};
use std::time::SystemTime;

use kartenn_core::audit::Severity;
use tokio::sync::mpsc;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::{Context, Filter, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Longest value kept, in octets (REQ-012-29).
pub const VALUE_MAX: usize = 2048;

/// Field names whose value is never written (REQ-012-28), matched case-insensitively
/// within the name.
const MASKED: [&str; 8] = [
    "password",
    "passwd",
    "token",
    "authorization",
    "cookie",
    "secret",
    "dsn",
    "connection_string",
];

/// A technical log event, captured once for every output.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// When it was emitted.
    pub time: SystemTime,
    /// Severity (REQ-012-4).
    pub severity: Severity,
    /// Stable name: the `name:` of the event, its target otherwise (REQ-012-5).
    pub name: String,
    /// Module that emitted it.
    pub target: String,
    /// Message.
    pub message: String,
    /// Other fields, masked and bounded.
    pub fields: BTreeMap<String, String>,
    /// W3C trace and span of the current span, when OpenTelemetry is on (REQ-012-6).
    pub trace_id: Option<String>,
    /// See `trace_id`.
    pub span_id: Option<String>,
}

const fn severity(level: Level) -> Severity {
    match level {
        Level::ERROR => Severity::Error,
        Level::WARN => Severity::Warn,
        Level::INFO => Severity::Info,
        Level::DEBUG => Severity::Debug,
        Level::TRACE => Severity::Trace,
    }
}

/// `value` cut to [`VALUE_MAX`] octets on a character boundary, with a mark.
fn bounded(value: String) -> String {
    if value.len() <= VALUE_MAX {
        return value;
    }
    let mut end = VALUE_MAX;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…[+{} octets]", &value[..end], value.len() - end)
}

fn masked(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    MASKED.iter().any(|m| name.contains(m))
}

#[derive(Default)]
struct Visitor {
    message: String,
    fields: BTreeMap<String, String>,
}

impl Visitor {
    fn put(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = bounded(value);
        } else if masked(field.name()) {
            self.fields.insert(field.name().to_owned(), "***".into());
        } else {
            self.fields.insert(field.name().to_owned(), bounded(value));
        }
    }
}

impl Visit for Visitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.put(field, format!("{value:?}"));
    }
}

impl Record {
    /// The record of `event`, now.
    #[must_use]
    pub fn capture(event: &Event<'_>) -> Self {
        let meta = event.metadata();
        let mut visitor = Visitor::default();
        event.record(&mut visitor);
        // An event without `name:` is called "event <file>:<line>" by tracing.
        let name = if meta.name().starts_with("event ") {
            meta.target().to_owned()
        } else {
            meta.name().to_owned()
        };
        Self {
            time: SystemTime::now(),
            severity: severity(*meta.level()),
            name,
            target: meta.target().to_owned(),
            message: visitor.message,
            fields: visitor.fields,
            trace_id: None,
            span_id: None,
        }
    }
}

/// The entry of one output in the capture: its filter, its queue, its counter of drops.
#[derive(Debug, Clone)]
pub struct Output {
    name: String,
    filter: Arc<RwLock<EnvFilter>>,
    sender: mpsc::Sender<Arc<Record>>,
    dropped: Arc<AtomicU64>,
}

impl Output {
    /// An output `name` taking what `filter` lets through (`EnvFilter` syntax: `info`,
    /// `warn,kartenn_service=debug`…), with a queue of `capacity` records; the receiving
    /// end goes to [`drive`].
    ///
    /// # Errors
    ///
    /// The filter does not parse.
    pub fn new(
        name: impl Into<String>,
        filter: &str,
        capacity: usize,
    ) -> Result<(Self, mpsc::Receiver<Arc<Record>>), tracing_subscriber::filter::ParseError> {
        let filter = EnvFilter::try_new(filter)?;
        let (sender, receiver) = mpsc::channel(capacity);
        Ok((
            Self {
                name: name.into(),
                filter: Arc::new(RwLock::new(filter)),
                sender,
                dropped: Arc::new(AtomicU64::new(0)),
            },
            receiver,
        ))
    }

    /// Name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Records dropped because the queue was full (REQ-012-22, 25).
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Replaces the filter, at run time (REQ-012-27).
    ///
    /// # Errors
    ///
    /// The filter does not parse; the old one stays.
    pub fn set_filter(&self, filter: &str) -> Result<(), tracing_subscriber::filter::ParseError> {
        let filter = EnvFilter::try_new(filter)?;
        *self.filter.write().unwrap_or_else(PoisonError::into_inner) = filter;
        tracing_core::callsite::rebuild_interest_cache();
        Ok(())
    }
}

/// The `tracing` layer that feeds the outputs.
#[derive(Debug, Clone)]
pub struct CaptureLayer {
    outputs: Vec<Output>,
}

impl CaptureLayer {
    /// A layer feeding `outputs`.
    #[must_use]
    pub const fn new(outputs: Vec<Output>) -> Self {
        Self { outputs }
    }
}

impl<S> Layer<S> for CaptureLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();
        let wanted: Vec<&Output> = self
            .outputs
            .iter()
            .filter(|o| {
                let filter = o.filter.read().unwrap_or_else(PoisonError::into_inner);
                Filter::<S>::enabled(&*filter, meta, &ctx)
            })
            .collect();
        if wanted.is_empty() {
            return;
        }
        let record = Arc::new(Record::capture(event));
        for output in wanted {
            if output.sender.try_send(Arc::clone(&record)).is_err() {
                output.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Where an output writes: a batch of records at a time.
pub trait Writer: Send {
    /// Formats and writes `records`.
    fn write(
        &mut self,
        records: &[Arc<Record>],
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

/// Most records taken from a queue at once.
const BATCH: usize = 256;

/// The task of an output: takes its queue by batches and writes them, until every sender
/// is gone (at shutdown, the queue is drained). A write error loses the batch and is
/// counted in the returned total.
pub async fn drive<W: Writer>(mut receiver: mpsc::Receiver<Arc<Record>>, mut writer: W) -> u64 {
    let mut lost = 0;
    let mut batch = Vec::with_capacity(BATCH);
    while receiver.recv_many(&mut batch, BATCH).await > 0 {
        if writer.write(&batch).await.is_err() {
            lost += batch.len() as u64;
        }
        batch.clear();
    }
    lost
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    #[derive(Clone, Default)]
    struct Memory(Arc<Mutex<Vec<Record>>>);

    impl Writer for Memory {
        fn write(
            &mut self,
            records: &[Arc<Record>],
        ) -> impl std::future::Future<Output = std::io::Result<()>> + Send {
            self.0
                .lock()
                .unwrap()
                .extend(records.iter().map(|r| (**r).clone()));
            std::future::ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn each_output_takes_what_its_filter_lets_through() {
        let (warn, warn_rx) = Output::new("warn", "warn", 16).unwrap();
        let (debug, debug_rx) = Output::new("debug", "debug", 16).unwrap();
        let subscriber = tracing_subscriber::registry()
            .with(CaptureLayer::new(vec![warn.clone(), debug.clone()]));
        tracing::subscriber::with_default(subscriber, || {
            tracing::event!(name: "pipeline.changeset.refused", tracing::Level::WARN, rules = 2, "refused");
            tracing::debug!(password = "hunter2", long = %"x".repeat(5000), "details");
        });
        drop((warn, debug));
        let (w, d) = (Memory::default(), Memory::default());
        drive(warn_rx, w.clone()).await;
        drive(debug_rx, d.clone()).await;
        let w = w.0.lock().unwrap();
        let d = d.0.lock().unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(d.len(), 2);
        assert_eq!(w[0].name, "pipeline.changeset.refused");
        assert_eq!(w[0].severity, Severity::Warn);
        assert_eq!(w[0].fields["rules"], "2");
        assert_eq!(d[1].name, module_path!(), "no name: the target");
        assert_eq!(d[1].fields["password"], "***", "REQ-012-28");
        assert!(d[1].fields["long"].len() < 2100 && d[1].fields["long"].ends_with("octets]"));
    }

    #[test]
    fn a_full_queue_drops_and_counts_without_waiting() {
        let (output, _receiver) = Output::new("slow", "info", 2).unwrap();
        let subscriber =
            tracing_subscriber::registry().with(CaptureLayer::new(vec![output.clone()]));
        tracing::subscriber::with_default(subscriber, || {
            for i in 0..5 {
                tracing::info!(i, "event");
            }
        });
        assert_eq!(output.dropped(), 3);
    }

    #[test]
    fn a_filter_changes_at_run_time() {
        let (output, mut receiver) = Output::new("out", "warn", 16).unwrap();
        let subscriber =
            tracing_subscriber::registry().with(CaptureLayer::new(vec![output.clone()]));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("before");
            output.set_filter("info").unwrap();
            tracing::info!("after");
        });
        assert_eq!(receiver.try_recv().unwrap().message, "after");
        assert!(receiver.try_recv().is_err());
        assert!(output.set_filter("not=a=filter").is_err());
    }
}
