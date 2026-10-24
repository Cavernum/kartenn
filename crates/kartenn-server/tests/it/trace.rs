//! A changeset through the real service and store: one trace with its database spans, its
//! technical log in the same trace, the changeset counted (spec 012, REQ-012-5, 6, 24,
//! 25).

use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit};
use kartenn_store_pg::write::PgStore;
use kartenn_telemetry::capture::{CaptureLayer, Output, drive};
use kartenn_telemetry::otel::{Identity, Instruments, Providers};
use opentelemetry_sdk::logs::InMemoryLogExporter;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
use opentelemetry_sdk::trace::InMemorySpanExporter;
use sqlx::PgPool;
use sqlx::types::Uuid;
use tracing_subscriber::layer::SubscriberExt;

#[kartenn_testing::pg_test]
async fn a_changeset_is_one_trace_with_its_database_spans(pool: PgPool) {
    let spans = InMemorySpanExporter::default();
    let logs = InMemoryLogExporter::default();
    let metrics = InMemoryMetricExporter::default();
    let providers = Providers::with_exporters(
        &Identity {
            version: "0.0.0",
            instance: "test".into(),
        },
        1.0,
        false,
        spans.clone(),
        logs.clone(),
        metrics.clone(),
    );
    let instruments = Instruments::new(&providers.meter, Vec::new);
    let (otlp, otlp_rx) = Output::new("otlp", "info", 64).unwrap();
    let subscriber = tracing_subscriber::registry()
        .with(CaptureLayer::new(vec![otlp]))
        .with(providers.trace_layer())
        .with(instruments.layer());
    let guard = tracing::subscriber::set_default(subscriber);

    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let request = ChangesetRequest {
        author,
        motive: "m".into(),
        period: None,
        operations: vec![Operation::CreateDomain {
            local: None,
            slug: "lab".into(),
        }],
        simulate: false,
        idempotency_key: None,
        apply_own: true,
    };
    let outcome = submit(
        &PgStore::new(pool.clone()),
        &Unrestricted,
        &request,
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    drop(guard);
    assert_eq!(drive(otlp_rx, providers.log_writer()).await, 0);
    providers.tracer.force_flush().unwrap();
    providers.logger.force_flush().unwrap();
    providers.meter.force_flush().unwrap();

    // One trace: the changeset, and under it the database spans.
    let finished = spans.get_finished_spans().unwrap();
    let root = finished
        .iter()
        .find(|s| s.name == "pipeline.changeset")
        .expect("the span of the changeset");
    let trace = root.span_context.trace_id();
    let db: Vec<&str> = finished
        .iter()
        .filter(|s| {
            s.attributes
                .iter()
                .any(|kv| kv.key.as_str() == "db.system.name")
        })
        .map(|s| {
            assert_eq!(s.span_context.trace_id(), trace, "{} in the trace", s.name);
            s.name.as_ref()
        })
        .collect();
    assert!(
        db.contains(&"BEGIN") && db.contains(&"COMMIT"),
        "database spans: {db:?}"
    );

    // The log of the outcome, in the same trace.
    let emitted = logs.get_emitted_logs().unwrap();
    let applied = emitted
        .iter()
        .find(|l| l.record.event_name() == Some("pipeline.changeset.applied"))
        .expect("the outcome is logged");
    assert_eq!(applied.record.trace_context().unwrap().trace_id, trace);
    let body = format!("{:?}", applied.record);
    assert!(!body.contains("\"m\""), "the motive is not logged: {body}");

    // Counted by state, its duration recorded.
    let finished = metrics.get_finished_metrics().unwrap();
    let metric = |name: &str| {
        finished
            .iter()
            .flat_map(opentelemetry_sdk::metrics::data::ResourceMetrics::scope_metrics)
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
            .find(|m| m.name() == name)
            .map(opentelemetry_sdk::metrics::data::Metric::data)
    };
    let Some(AggregatedMetrics::U64(MetricData::Sum(count))) = metric("kartenn.changeset.count")
    else {
        panic!("kartenn.changeset.count is recorded");
    };
    let point = count.data_points().next().unwrap();
    assert_eq!(point.value(), 1);
    assert!(
        point
            .attributes()
            .any(|kv| kv.key.as_str() == "state" && kv.value.as_str() == "applied")
    );
    assert!(matches!(
        metric("kartenn.pipeline.duration"),
        Some(AggregatedMetrics::F64(MetricData::Histogram(_)))
    ));
}
