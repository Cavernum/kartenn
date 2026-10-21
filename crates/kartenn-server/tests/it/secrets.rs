//! Canary secrets through a running instance: none reaches an output, audit, logs or
//! traces, whatever the transport (spec 012, REQ-012-28; task 012-20). The panic report
//! is covered by `kartenn-telemetry/tests/it/panic.rs`, the connection error of the binary
//! by `redact_credentials`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use kartenn_core::audit::{EVENTS, OBJECTS_READ, Outcome};
use kartenn_service::audit::{AuditJournal, Instance, RequestContext};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::{
    LogsService, LogsServiceServer,
};
use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::{
    TraceService, TraceServiceServer,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use sqlx::PgPool;
use tokio::io::AsyncReadExt;

/// `kartenn:canary-basic` in Base64.
const BASIC: &str = "Basic a2FydGVubjpjYW5hcnktYmFzaWM=";
const TOKEN: &str = "canary-token-7f3a";
const DSN: &str = "postgres://kartenn:canary-dsn-pass@db:5432/kartenn";
/// What must appear nowhere.
const CANARIES: [&str; 4] = [
    "a2FydGVubjpjYW5hcnktYmFzaWM",
    "canary-basic",
    "canary-token-7f3a",
    "canary-dsn-pass",
];

/// What each output received, as text.
type Received = Arc<Mutex<Vec<String>>>;

#[derive(Clone, Default)]
struct Collector {
    received: Received,
}

#[tonic::async_trait]
impl LogsService for Collector {
    async fn export(
        &self,
        request: tonic::Request<ExportLogsServiceRequest>,
    ) -> Result<tonic::Response<ExportLogsServiceResponse>, tonic::Status> {
        self.received
            .lock()
            .unwrap()
            .push(format!("{:?}", request.into_inner()));
        Ok(tonic::Response::new(ExportLogsServiceResponse::default()))
    }
}

#[tonic::async_trait]
impl TraceService for Collector {
    async fn export(
        &self,
        request: tonic::Request<ExportTraceServiceRequest>,
    ) -> Result<tonic::Response<ExportTraceServiceResponse>, tonic::Status> {
        self.received
            .lock()
            .unwrap()
            .push(format!("{:?}", request.into_inner()));
        Ok(tonic::Response::new(ExportTraceServiceResponse::default()))
    }
}

/// A TCP listener keeping every octet of every connection.
async fn syslog_server() -> (String, Received) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let received = Received::default();
    let sink = Arc::clone(&received);
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let sink = Arc::clone(&sink);
            tokio::spawn(async move {
                let mut buf = vec![0; 65_536];
                while let Ok(n) = stream.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    sink.lock()
                        .unwrap()
                        .push(String::from_utf8_lossy(&buf[..n]).into_owned());
                }
            });
        }
    });
    (address, received)
}

fn leaks(what: &str, text: &str) {
    for canary in CANARIES {
        assert!(!text.contains(canary), "{canary} reached {what}: {text}");
    }
}

#[test]
fn no_audit_detail_is_named_like_a_secret() {
    for kind in EVENTS {
        for detail in kind.details {
            assert_ne!(
                kartenn_telemetry::capture::mask(detail, "x"),
                "***",
                "{}: the detail {detail} would carry a secret",
                kind.name
            );
        }
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn canary_secrets_reach_no_output(pool: PgPool) {
    let dir = std::env::temp_dir().join(format!("kartenn-secrets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let technical = dir.join("technical.jsonl");
    let audit = dir.join("audit.jsonl");
    let journal_path = dir.join("journal.sock");
    let journal = tokio::net::UnixDatagram::bind(&journal_path).unwrap();
    let journald = Received::default();
    tokio::spawn({
        let journald = Arc::clone(&journald);
        async move {
            let mut buf = vec![0; 65_536];
            while let Ok(n) = journal.recv(&mut buf).await {
                journald
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).into_owned());
            }
        }
    });
    let (syslog_address, syslog) = syslog_server().await;
    let (audit_syslog_address, audit_syslog) = syslog_server().await;
    let collector = Collector::default();
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let otlp_address = probe.local_addr().unwrap();
    drop(probe);
    tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(LogsServiceServer::new(collector.clone()))
            .add_service(TraceServiceServer::new(collector.clone()))
            .serve(otlp_address),
    );

    let config = kartenn_server::config::from_toml(&format!(
        r#"
        [telemetry]
        outputs = [
          {{ kind = "file", path = "{technical}", filter = "info" }},
          {{ kind = "journald", socket = "{journal}", filter = "info" }},
          {{ kind = "syslog", transport = "tcp", address = "{syslog_address}", filter = "info" }},
          {{ kind = "otlp", endpoint = "http://{otlp_address}", filter = "info" }},
        ]
        [audit]
        sinks = [
          {{ kind = "file", name = "archive", path = "{audit}" }},
          {{ kind = "syslog", name = "siem", transport = "tcp", address = "{audit_syslog_address}" }},
          {{ kind = "otlp", name = "collector", endpoint = "http://{otlp_address}" }},
        ]
        seal_interval_ms = 100
        relay_interval_ms = 100
        "#,
        technical = technical.display(),
        journal = journal_path.display(),
        audit = audit.display(),
    ))
    .unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let run = tokio::spawn({
        let pool = pool.clone();
        async move {
            kartenn_server::app::run(pool, &config, async {
                let _ = stopped.await;
            })
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;

    // The canaries, the way code would carry them: headers and tokens as fields, a
    // connection string in a message, an error and a span.
    {
        let span = tracing::info_span!("test.secrets.request", api_token = TOKEN, dsn = DSN);
        let _entered = span.enter();
        tracing::info!(
            name: "test.secrets.request",
            authorization = BASIC,
            token = TOKEN,
            "request received"
        );
        tracing::warn!(name: "test.secrets.connect", "cannot reach {DSN}");
        tracing::error!(
            name: "test.secrets.error",
            error = %format!("connection refused: {DSN}"),
            "the store is unreachable"
        );
    }
    let event = RequestContext::system()
        .event(&OBJECTS_READ, Outcome::Success)
        .unwrap();
    PgAuditJournal::new(pool.clone(), Instance::new("test"))
        .record(&event)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    stop.send(()).unwrap();
    let report = run.await.unwrap().unwrap();
    assert_eq!(report.lost_technical, 0);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let outputs = [
        (
            "the technical file",
            std::fs::read_to_string(&technical).unwrap(),
        ),
        ("journald", journald.lock().unwrap().concat()),
        ("syslog", syslog.lock().unwrap().concat()),
        ("the audit file", std::fs::read_to_string(&audit).unwrap()),
        ("the audit syslog", audit_syslog.lock().unwrap().concat()),
        (
            "the OTLP collector",
            collector.received.lock().unwrap().concat(),
        ),
    ];
    for (what, text) in &outputs {
        // Each output did receive something: the check is not vacuous.
        assert!(!text.is_empty(), "{what} received nothing");
        leaks(what, text);
    }
    // The technical outputs and the collector saw the canary events, masked.
    for (what, text) in [&outputs[0], &outputs[1], &outputs[2], &outputs[5]] {
        assert!(text.contains("test.secrets.request"), "{what}: {text}");
        assert!(
            text.contains("postgres://kartenn:***@db:5432/kartenn"),
            "{what}: {text}"
        );
    }
    assert!(
        outputs[5].1.contains("data.objects.read"),
        "the audit reached the collector"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
