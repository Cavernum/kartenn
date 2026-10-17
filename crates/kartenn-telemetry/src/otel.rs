//! OpenTelemetry (spec 012, REQ-012-6, 7, 24, 25; 012/L9): providers of traces, logs and
//! metrics, exported in OTLP over gRPC; the `tracing` layers that feed them without
//! exporting an event twice; the incoming trace context, followed only for trusted
//! callers; the instruments of REQ-012-25.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use opentelemetry::propagation::TextMapPropagator;
use opentelemetry::trace::{SpanContext, TraceContextExt, TracerProvider as _};
use opentelemetry::{Context, KeyValue};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::{LogExporter, SdkLoggerProvider};
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::metrics::exporter::PushMetricExporter;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider, SpanExporter};
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::{EnvFilter, filter_fn};
use tracing_subscriber::registry::LookupSpan;

/// Crates whose events and spans never reach the OpenTelemetry exporters: they carry the
/// export itself, and would feed it back (telemetry loop).
const QUIET: [&str; 7] = [
    "hyper",
    "h2",
    "tonic",
    "tower",
    "reqwest",
    "opentelemetry",
    "opentelemetry_sdk",
];

fn quiet(target: &str) -> bool {
    QUIET
        .iter()
        .any(|q| target == *q || target.starts_with(&format!("{q}::")))
}

/// Why OpenTelemetry cannot start.
#[derive(Debug, thiserror::Error)]
#[error("OpenTelemetry: {0}")]
pub struct OtelError(String);

/// What names this process in every signal (REQ-012-24).
#[derive(Debug, Clone)]
pub struct Identity {
    /// `service.version`.
    pub version: &'static str,
    /// `service.instance.id`.
    pub instance: String,
}

/// The resource of every signal: `service.name` (`kartenn` unless `OTEL_SERVICE_NAME`
/// says otherwise), `service.version`, `service.instance.id`, and `OTEL_RESOURCE_ATTRIBUTES`.
#[must_use]
pub fn resource(identity: &Identity) -> Resource {
    let mut builder = Resource::builder();
    if std::env::var_os("OTEL_SERVICE_NAME").is_none() {
        builder = builder.with_service_name("kartenn");
    }
    builder
        .with_attributes([
            KeyValue::new("service.version", identity.version),
            KeyValue::new("service.instance.id", identity.instance.clone()),
        ])
        .build()
}

/// The three providers.
#[derive(Debug, Clone)]
pub struct Providers {
    /// Traces.
    pub tracer: SdkTracerProvider,
    /// Logs.
    pub logger: SdkLoggerProvider,
    /// Metrics.
    pub meter: SdkMeterProvider,
}

impl Providers {
    /// Exporters given by the caller (tests use the in-memory ones); `batch` for the
    /// batching processors of production, otherwise every record is exported at once.
    /// Traces are sampled by `ParentBased(TraceIdRatio(sampling_ratio))`.
    pub fn with_exporters(
        identity: &Identity,
        sampling_ratio: f64,
        batch: bool,
        spans: impl SpanExporter + 'static,
        logs: impl LogExporter + 'static,
        metrics: impl PushMetricExporter,
    ) -> Self {
        let resource = resource(identity);
        let sampler = Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(sampling_ratio)));
        let tracer = SdkTracerProvider::builder()
            .with_resource(resource.clone())
            .with_sampler(sampler);
        let tracer = if batch {
            tracer.with_batch_exporter(spans)
        } else {
            tracer.with_simple_exporter(spans)
        }
        .build();
        let logger = SdkLoggerProvider::builder().with_resource(resource.clone());
        let logger = if batch {
            logger.with_batch_exporter(logs)
        } else {
            logger.with_simple_exporter(logs)
        }
        .build();
        let meter = SdkMeterProvider::builder()
            .with_resource(resource)
            .with_periodic_exporter(metrics)
            .build();
        Self {
            tracer,
            logger,
            meter,
        }
    }

    /// OTLP over gRPC to `OTEL_EXPORTER_OTLP_ENDPOINT` (by default
    /// `http://localhost:4317`), with the batching processors. Needs a tokio runtime.
    ///
    /// # Errors
    ///
    /// An exporter cannot be built.
    pub fn otlp(identity: &Identity, sampling_ratio: f64) -> Result<Self, OtelError> {
        let err = |e: opentelemetry_otlp::ExporterBuildError| OtelError(e.to_string());
        let spans = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .build()
            .map_err(err)?;
        let logs = opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .build()
            .map_err(err)?;
        let metrics = opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .build()
            .map_err(err)?;
        Ok(Self::with_exporters(
            identity,
            sampling_ratio,
            true,
            spans,
            logs,
            metrics,
        ))
    }

    /// The layer of traces: spans only, so that an event inside a span is not also
    /// exported as a span event (it goes through [`Providers::log_layer`]).
    pub fn trace_layer<S>(&self) -> impl Layer<S> + use<S>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        tracing_opentelemetry::layer()
            .with_tracer(self.tracer.tracer("kartenn"))
            .with_filter(filter_fn(|meta| meta.is_span() && !quiet(meta.target())))
    }

    /// The layer of logs, through `filter` (`EnvFilter` syntax), the crates of the export
    /// itself excluded.
    ///
    /// # Errors
    ///
    /// The filter does not parse.
    pub fn log_layer<S>(
        &self,
        filter: &str,
    ) -> Result<impl Layer<S> + use<S>, tracing_subscriber::filter::ParseError>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let filter = QUIET
            .iter()
            .fold(filter.to_owned(), |acc, q| acc + "," + q + "=off");
        let filter = EnvFilter::try_new(filter)?;
        Ok(OpenTelemetryTracingBridge::new(&self.logger).with_filter(filter))
    }

    /// Flushes and stops the three providers (REQ-012-34).
    ///
    /// # Errors
    ///
    /// The first provider that fails.
    pub fn shutdown(&self) -> Result<(), OtelError> {
        let err = |e: opentelemetry_sdk::error::OTelSdkError| OtelError(e.to_string());
        self.tracer.shutdown().map_err(err)?;
        self.logger.shutdown().map_err(err)?;
        self.meter.shutdown().map_err(err)
    }
}

/// The trace context of an incoming request (REQ-012-7).
#[derive(Debug, Clone, Default)]
pub struct Incoming {
    /// The parent to follow, for a trusted caller.
    pub parent: Option<Context>,
    /// For another caller: the context received, to link a new root to, never to follow
    /// (its `sampled` flag cannot force sampling, W3C Trace Context §7.2).
    pub link: Option<SpanContext>,
}

/// Reads a `traceparent` header: an invalid one is ignored (W3C Trace Context §3.2.2.3).
/// Baggage is never read (W3C Baggage §4.1).
#[must_use]
pub fn incoming(traceparent: Option<&str>, trusted: bool) -> Incoming {
    let Some(header) = traceparent else {
        return Incoming::default();
    };
    let carrier: HashMap<String, String> =
        HashMap::from([("traceparent".to_owned(), header.to_owned())]);
    let context = TraceContextPropagator::new().extract(&carrier);
    let span = context.span().span_context().clone();
    if !span.is_valid() {
        return Incoming::default();
    }
    if trusted {
        Incoming {
            parent: Some(context),
            link: None,
        }
    } else {
        Incoming {
            parent: None,
            link: Some(span),
        }
    }
}

/// The W3C identifiers of the current trace, if any (for the technical records).
#[must_use]
pub fn current_ids() -> Option<(String, String)> {
    let context = Context::current();
    let span = context.span();
    let span = span.span_context();
    span.is_valid()
        .then(|| (span.trace_id().to_string(), span.span_id().to_string()))
}

/// Values of the gauges that kartenn computes outside the metric callbacks (REQ-012-25).
#[derive(Debug, Default)]
pub struct Gauges {
    /// Age in seconds of the oldest event not delivered, per audit output.
    pub export_lag: HashMap<String, f64>,
    /// Events not sealed yet.
    pub unsealed: u64,
    /// Clock of the instance minus clock of PostgreSQL, in seconds.
    pub clock_skew: f64,
}

/// The instruments of REQ-012-25.
#[derive(Debug, Clone)]
pub struct Instruments {
    /// `http.server.request.duration` (s), recorded by the API (006).
    pub http_duration: opentelemetry::metrics::Histogram<f64>,
    /// `kartenn.changeset.count`, by `state`.
    pub changesets: opentelemetry::metrics::Counter<u64>,
    /// `kartenn.pipeline.duration` (s).
    pub pipeline_duration: opentelemetry::metrics::Histogram<f64>,
    /// `kartenn.audit.write.failures`.
    pub audit_failures: opentelemetry::metrics::Counter<u64>,
    /// Shared values of the gauges.
    pub gauges: Arc<Mutex<Gauges>>,
}

impl Instruments {
    /// Instruments on `provider`; `dropped` gives the records dropped per technical output
    /// (`kartenn.log.dropped`).
    pub fn new(
        provider: &SdkMeterProvider,
        dropped: impl Fn() -> Vec<(String, u64)> + Send + Sync + 'static,
    ) -> Self {
        use opentelemetry::metrics::MeterProvider as _;
        let meter = provider.meter("kartenn");
        let gauges = Arc::new(Mutex::new(Gauges::default()));
        meter
            .u64_observable_counter("kartenn.log.dropped")
            .with_unit("{record}")
            .with_description("Technical records dropped by a full output")
            .with_callback(move |observer| {
                for (output, n) in dropped() {
                    observer.observe(n, &[KeyValue::new("output", output)]);
                }
            })
            .build();
        let lag = Arc::clone(&gauges);
        meter
            .f64_observable_gauge("kartenn.audit.export.lag")
            .with_unit("s")
            .with_description("Age of the oldest audit event an output has not received")
            .with_callback(move |observer| {
                let gauges = lag.lock().unwrap_or_else(PoisonError::into_inner);
                for (sink, seconds) in &gauges.export_lag {
                    observer.observe(*seconds, &[KeyValue::new("sink", sink.clone())]);
                }
            })
            .build();
        let unsealed = Arc::clone(&gauges);
        meter
            .u64_observable_gauge("kartenn.audit.unsealed")
            .with_unit("{event}")
            .with_description("Audit events not sealed yet")
            .with_callback(move |observer| {
                let n = unsealed
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .unsealed;
                observer.observe(n, &[]);
            })
            .build();
        let skew = Arc::clone(&gauges);
        meter
            .f64_observable_gauge("kartenn.clock.skew")
            .with_unit("s")
            .with_description("Clock of the instance minus clock of PostgreSQL")
            .with_callback(move |observer| {
                let s = skew
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clock_skew;
                observer.observe(s, &[]);
            })
            .build();
        Self {
            http_duration: meter
                .f64_histogram("http.server.request.duration")
                .with_unit("s")
                .build(),
            changesets: meter
                .u64_counter("kartenn.changeset.count")
                .with_unit("{changeset}")
                .build(),
            pipeline_duration: meter
                .f64_histogram("kartenn.pipeline.duration")
                .with_unit("s")
                .build(),
            audit_failures: meter
                .u64_counter("kartenn.audit.write.failures")
                .with_unit("{event}")
                .build(),
            gauges,
        }
    }
}
