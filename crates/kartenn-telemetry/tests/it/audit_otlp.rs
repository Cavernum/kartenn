//! The OTLP output of the audit journal: one acknowledged export per batch, a rejection
//! leaves the events for the next attempt (spec 012, REQ-012-15; task 012-15).

use std::sync::{Arc, Mutex};

use kartenn_core::audit::{OBJECTS_READ, Outcome};
use kartenn_service::audit::{AuditJournal, Instance, RequestContext};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::audit_relay::{RelayError, relay_once};
use kartenn_telemetry::sinks::OtlpSink;
use opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::{
    LogsService, LogsServiceServer,
};
use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsPartialSuccess, ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::common::v1::any_value;
use sqlx::PgPool;

#[derive(Clone, Default)]
struct Collector {
    received: Arc<Mutex<Vec<ExportLogsServiceRequest>>>,
    reject: Arc<Mutex<bool>>,
}

#[tonic::async_trait]
impl LogsService for Collector {
    async fn export(
        &self,
        request: tonic::Request<ExportLogsServiceRequest>,
    ) -> Result<tonic::Response<ExportLogsServiceResponse>, tonic::Status> {
        let request = request.into_inner();
        let count = request.resource_logs[0].scope_logs[0].log_records.len();
        if *self.reject.lock().unwrap() {
            return Ok(tonic::Response::new(ExportLogsServiceResponse {
                partial_success: Some(ExportLogsPartialSuccess {
                    rejected_log_records: i64::try_from(count).unwrap(),
                    error_message: "quota".into(),
                }),
            }));
        }
        self.received.lock().unwrap().push(request);
        Ok(tonic::Response::new(ExportLogsServiceResponse::default()))
    }
}

async fn record(pool: &PgPool, n: usize) {
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("test"));
    for _ in 0..n {
        let event = RequestContext::system()
            .event(&OBJECTS_READ, Outcome::Success)
            .unwrap();
        journal.record(&event).await.unwrap();
    }
}

#[kartenn_testing::pg_test]
async fn audit_events_reach_an_otlp_collector_acknowledged(pool: PgPool) {
    let collector = Collector::default();
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(LogsServiceServer::new(collector.clone()))
            .serve(address),
    );
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let sink = OtlpSink::new("otlp", format!("http://{address}"));

    record(&pool, 2).await;
    assert_eq!(relay_once(&pool, &sink, 500).await.unwrap(), 2);
    {
        let received = collector.received.lock().unwrap();
        let records = &received[0].resource_logs[0].scope_logs[0].log_records;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].event_name, "data.objects.read");
        assert_eq!(records[0].severity_number, 9);
        assert!(records[0].time_unix_nano > 0);
        // A structured body, so that a backend indexes each field of the event.
        let Some(any_value::Value::KvlistValue(body)) =
            records[0].body.as_ref().and_then(|b| b.value.as_ref())
        else {
            panic!("a map as body: {:?}", records[0].body);
        };
        let field = |key: &str| body.values.iter().find(|kv| kv.key == key);
        assert!(matches!(
            field("name").and_then(|kv| kv.value.as_ref()?.value.as_ref()),
            Some(any_value::Value::StringValue(n)) if n == "data.objects.read"
        ));
        assert!(matches!(
            field("actor_kind").and_then(|kv| kv.value.as_ref()?.value.as_ref()),
            Some(any_value::Value::StringValue(k)) if k == "system"
        ));
        assert!(matches!(
            field("details").and_then(|kv| kv.value.as_ref()?.value.as_ref()),
            Some(any_value::Value::KvlistValue(_))
        ));
    }

    // Rejected by the collector: not delivered, offered again.
    *collector.reject.lock().unwrap() = true;
    record(&pool, 1).await;
    assert!(matches!(
        relay_once(&pool, &sink, 500).await,
        Err(RelayError::Sink { .. })
    ));
    *collector.reject.lock().unwrap() = false;
    assert_eq!(relay_once(&pool, &sink, 500).await.unwrap(), 1);
    assert_eq!(collector.received.lock().unwrap().len(), 2);
    server.abort();
}
