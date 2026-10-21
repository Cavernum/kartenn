//! OpenTelemetry: one export per event, logs carrying the trace of their span, incoming
//! contexts followed only for trusted callers, instruments with their units (spec 012,
//! REQ-012-6, 7, 24, 25; task 012-15).

use kartenn_telemetry::capture::{CaptureLayer, Output, drive};
use kartenn_telemetry::otel::{Identity, Instruments, Providers, incoming};
use opentelemetry::trace::TraceContextExt;
use opentelemetry_sdk::logs::InMemoryLogExporter;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use tracing_subscriber::layer::SubscriberExt;

fn identity() -> Identity {
    Identity {
        version: "1.2.3",
        instance: "instance-1".into(),
    }
}

#[tokio::test]
async fn an_event_is_exported_once_with_the_trace_of_its_span() {
    let spans = InMemorySpanExporter::default();
    let logs = InMemoryLogExporter::default();
    let metrics = InMemoryMetricExporter::default();
    let providers = Providers::with_exporters(
        &identity(),
        1.0,
        false,
        spans.clone(),
        logs.clone(),
        metrics,
    );
    let (output, mut technical) = Output::new("stdout", "info", 16).unwrap();
    let (otlp, otlp_rx) = Output::new("otlp", "info", 16).unwrap();
    let subscriber = tracing_subscriber::registry()
        .with(providers.trace_layer())
        .with(CaptureLayer::new(vec![output, otlp]));
    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!(
            "submit",
            token = "canary-span-token",
            db = "postgres://kartenn:canary-span-password@db/kartenn"
        );
        let _entered = span.enter();
        tracing::event!(
            name: "pipeline.changeset.applied",
            tracing::Level::INFO,
            authorization = "Basic Y2FuYXJ5LWxvZw==",
            "applied"
        );
        // The export crates are kept out of the export (telemetry loop).
        tracing::info!(target: "hyper::client", "connection");
    });
    assert_eq!(drive(otlp_rx, providers.log_writer()).await, 0);
    providers.tracer.force_flush().unwrap();
    providers.logger.force_flush().unwrap();

    let finished = spans.get_finished_spans().unwrap();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].name, "submit");
    assert!(
        finished[0].events.is_empty(),
        "not exported twice, as a span event"
    );
    let resource_service = finished[0].span_context.trace_id();

    let emitted = logs.get_emitted_logs().unwrap();
    assert_eq!(emitted.len(), 1, "the hyper event is not exported");
    let record = &emitted[0].record;
    assert_eq!(record.event_name(), Some("pipeline.changeset.applied"));
    assert_eq!(record.trace_context().unwrap().trace_id, resource_service);
    assert_eq!(
        record.severity_number(),
        Some(opentelemetry::logs::Severity::Info)
    );

    // Secrets masked in the exported log and span (REQ-012-28).
    let exported = format!("{:?}{:?}", record, finished[0].attributes);
    for canary in [
        "Y2FuYXJ5LWxvZw",
        "canary-span-token",
        "canary-span-password",
    ] {
        assert!(!exported.contains(canary), "{canary} leaked: {exported}");
    }
    assert!(exported.contains("postgres://kartenn:***@db/kartenn"));

    // The technical record carries the same trace.
    let captured = technical.try_recv().unwrap();
    assert_eq!(
        captured.trace_id.as_deref(),
        Some(resource_service.to_string().as_str())
    );
    let resource = emitted[0].resource.as_ref();
    let attribute = |key: &str| {
        resource
            .iter()
            .find(|(k, _)| k.as_str() == key)
            .map(|(_, v)| v.to_string())
    };
    assert_eq!(attribute("service.name").as_deref(), Some("kartenn"));
    assert_eq!(attribute("service.version").as_deref(), Some("1.2.3"));
    assert_eq!(
        attribute("service.instance.id").as_deref(),
        Some("instance-1")
    );
}

#[test]
fn an_incoming_context_is_followed_only_for_trusted_callers() {
    let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let trusted = incoming(Some(header), true);
    let parent = trusted.parent.unwrap();
    assert_eq!(
        parent.span().span_context().trace_id().to_string(),
        "4bf92f3577b34da6a3ce929d0e0e4736"
    );
    let untrusted = incoming(Some(header), false);
    assert!(
        untrusted.parent.is_none(),
        "a forged sampled flag forces nothing"
    );
    assert_eq!(
        untrusted.link.unwrap().trace_id().to_string(),
        "4bf92f3577b34da6a3ce929d0e0e4736"
    );
    let invalid = incoming(Some("00-zz-00f067aa0ba902b7-01"), true);
    assert!(
        invalid.parent.is_none() && invalid.link.is_none(),
        "ignored"
    );
}

#[tokio::test]
async fn the_instruments_carry_their_units() {
    let metrics = InMemoryMetricExporter::default();
    let providers = Providers::with_exporters(
        &identity(),
        1.0,
        false,
        InMemorySpanExporter::default(),
        InMemoryLogExporter::default(),
        metrics.clone(),
    );
    let instruments = Instruments::new(&providers.meter, || vec![("syslog".into(), 3)]);
    instruments
        .changesets
        .add(1, &[opentelemetry::KeyValue::new("state", "applied")]);
    instruments.pipeline_duration.record(0.012, &[]);
    instruments.http_duration.record(0.003, &[]);
    instruments.audit_failures.add(0, &[]);
    {
        let mut gauges = instruments.gauges.lock().unwrap();
        gauges.export_lag.insert("siem".into(), 4.0);
        gauges.unsealed = 2;
        gauges.clock_skew = 0.2;
    }
    providers.meter.force_flush().unwrap();
    let finished = metrics.get_finished_metrics().unwrap();
    let units: std::collections::BTreeMap<String, String> = finished
        .iter()
        .flat_map(opentelemetry_sdk::metrics::data::ResourceMetrics::scope_metrics)
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .map(|m| (m.name().to_owned(), m.unit().to_owned()))
        .collect();
    for (name, unit) in [
        ("http.server.request.duration", "s"),
        ("kartenn.changeset.count", "{changeset}"),
        ("kartenn.pipeline.duration", "s"),
        ("kartenn.log.dropped", "{record}"),
        ("kartenn.audit.export.lag", "s"),
        ("kartenn.audit.unsealed", "{event}"),
        ("kartenn.audit.write.failures", "{event}"),
        ("kartenn.clock.skew", "s"),
    ] {
        assert_eq!(units.get(name).map(String::as_str), Some(unit), "{name}");
    }
}
