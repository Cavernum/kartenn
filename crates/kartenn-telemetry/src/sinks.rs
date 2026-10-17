//! Outputs of the audit journal (spec 012, REQ-012-15): a file of JSON lines, and syslog
//! (RFC 5424, TCP or TLS with octet counting). Each receives the stored rows of
//! `audit.event`, in sequence order, and returns once they are written.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;

use kartenn_service::audit::{AuditSink, SinkError};
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::logs::v1::logs_service_client::LogsServiceClient;
use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, KeyValue, any_value};
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::resource::v1::Resource;
use serde_json::Value;
use syslog_sender::{Facility, Message, SdElement, Sender, Severity, Target};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tonic::transport::{Channel, ClientTlsConfig};

fn sink_err(e: impl std::fmt::Display) -> SinkError {
    SinkError(e.to_string())
}

/// JSON lines appended to a file, synchronised to disk after each batch.
#[derive(Debug, Clone)]
pub struct FileSink {
    name: String,
    path: PathBuf,
}

impl FileSink {
    /// An output named `name` writing to `path` (created if needed). The file is opened
    /// for each batch, so that an external rotation is followed.
    #[must_use]
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
        }
    }
}

impl AuditSink for FileSink {
    fn name(&self) -> &str {
        &self.name
    }

    async fn deliver(&self, events: &[Value]) -> Result<(), SinkError> {
        let mut out = Vec::new();
        for event in events {
            serde_json::to_writer(&mut out, event).map_err(sink_err)?;
            out.push(b'\n');
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .map_err(sink_err)?;
        file.write_all(&out).await.map_err(sink_err)?;
        file.sync_data().await.map_err(sink_err)
    }
}

/// Longest audit message: an event is never cut, its JSON would be broken.
const SYSLOG_MAX: usize = 64 * 1024;
/// Largest `sequenceId` (RFC 5424 §7.3.1).
const SEQUENCE_MAX: u32 = 2_147_483_647;

/// Syslog RFC 5424 (012/L19): facility `audit`, `MSGID` the event name, `meta
/// sequenceId` counted from 1 by this sender, the whole event in JSON as MSG.
#[derive(Debug)]
pub struct SyslogSink {
    name: String,
    hostname: Option<String>,
    sender: Mutex<Sender>,
    sequence: AtomicU32,
}

impl SyslogSink {
    /// An output named `name` to `target` (octet counting on streams), naming `hostname`.
    #[must_use]
    pub fn new(name: impl Into<String>, target: Target, hostname: Option<String>) -> Self {
        Self {
            name: name.into(),
            hostname,
            sender: Mutex::new(Sender::new(target)),
            sequence: AtomicU32::new(0),
        }
    }

    fn next_sequence(&self) -> u32 {
        // 1, 2, …, 2147483647, then 1 again (RFC 5424 §7.3.1).
        let n = self.sequence.fetch_add(1, Ordering::Relaxed) % SEQUENCE_MAX;
        n + 1
    }

    fn message(&self, event: &Value) -> Result<Vec<u8>, SinkError> {
        let severity = match event["severity"].as_str() {
            Some("ERROR") => Severity::Error,
            Some("WARN") => Severity::Warning,
            Some("INFO") => Severity::Informational,
            _ => Severity::Debug,
        };
        let at = event["occurred_at"]
            .as_str()
            .and_then(|t| OffsetDateTime::parse(t, &Rfc3339).ok())
            .map(SystemTime::from);
        let mut message = Message::new(Facility::Audit, severity)
            .app_name("kartenn")
            .map_err(sink_err)?
            .procid(&std::process::id().to_string())
            .map_err(sink_err)?;
        if let Some(at) = at {
            message = message.timestamp(at);
        }
        if let Some(host) = &self.hostname {
            message = message.hostname(host).map_err(sink_err)?;
        }
        if let Some(name) = event["name"].as_str() {
            message = message.msgid(name).map_err(sink_err)?;
        }
        message = message
            .structured(SdElement::meta_sequence(self.next_sequence()).map_err(sink_err)?)
            .map_err(sink_err)?
            .text(event.to_string());
        message.to_rfc5424(SYSLOG_MAX).map_err(sink_err)
    }
}

impl AuditSink for SyslogSink {
    fn name(&self) -> &str {
        &self.name
    }

    async fn deliver(&self, events: &[Value]) -> Result<(), SinkError> {
        let messages = events
            .iter()
            .map(|e| self.message(e))
            .collect::<Result<Vec<_>, _>>()?;
        self.sender
            .lock()
            .await
            .send(&messages)
            .await
            .map_err(sink_err)
    }
}

/// OTLP logs over gRPC (REQ-012-15): each batch is one acknowledged `Export` call, never
/// the batching processor of the SDK, which drops records when full. A batch partly
/// rejected by the collector is not delivered.
#[derive(Debug)]
pub struct OtlpSink {
    name: String,
    endpoint: String,
    client: Mutex<Option<LogsServiceClient<Channel>>>,
}

impl OtlpSink {
    /// An output named `name` to `endpoint` (`http://…:4317`, or `https://` with the
    /// system roots); connected at the first batch, and again after an error.
    #[must_use]
    pub fn new(name: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            endpoint: endpoint.into(),
            client: Mutex::new(None),
        }
    }

    async fn connect(&self) -> Result<LogsServiceClient<Channel>, SinkError> {
        let mut endpoint = Channel::from_shared(self.endpoint.clone()).map_err(sink_err)?;
        if self.endpoint.starts_with("https://") {
            endpoint = endpoint
                .tls_config(ClientTlsConfig::new().with_native_roots())
                .map_err(sink_err)?;
        }
        Ok(LogsServiceClient::new(
            endpoint.connect().await.map_err(sink_err)?,
        ))
    }
}

fn nanos(value: &Value) -> u64 {
    value
        .as_str()
        .and_then(|t| OffsetDateTime::parse(t, &Rfc3339).ok())
        .and_then(|t| u64::try_from(t.unix_timestamp_nanos()).ok())
        .unwrap_or(0)
}

fn text_value(text: impl Into<String>) -> AnyValue {
    AnyValue {
        value: Some(any_value::Value::StringValue(text.into())),
    }
}

/// The OTLP log record of an audit event: its time, severity (table of REQ-012-4), name,
/// the event in JSON as body.
#[must_use]
pub fn otlp_record(event: &Value) -> LogRecord {
    let (number, text) = match event["severity"].as_str() {
        Some("ERROR") => (17, "ERROR"),
        Some("WARN") => (13, "WARN"),
        Some("INFO") => (9, "INFO"),
        Some("DEBUG") => (5, "DEBUG"),
        _ => (1, "TRACE"),
    };
    let attribute = |key: &str, value: Option<AnyValue>| KeyValue {
        key: key.to_owned(),
        value,
        ..KeyValue::default()
    };
    LogRecord {
        time_unix_nano: nanos(&event["occurred_at"]),
        observed_time_unix_nano: nanos(&event["recorded_at"]),
        severity_number: number,
        severity_text: text.to_owned(),
        event_name: event["name"].as_str().unwrap_or_default().to_owned(),
        body: Some(text_value(event.to_string())),
        attributes: vec![
            attribute(
                "kartenn.audit.seq",
                Some(AnyValue {
                    value: Some(any_value::Value::IntValue(
                        event["seq"].as_i64().unwrap_or(0),
                    )),
                }),
            ),
            attribute(
                "kartenn.audit.criticality",
                Some(text_value(
                    event["criticality"].as_str().unwrap_or_default(),
                )),
            ),
        ],
        ..LogRecord::default()
    }
}

impl AuditSink for OtlpSink {
    fn name(&self) -> &str {
        &self.name
    }

    async fn deliver(&self, events: &[Value]) -> Result<(), SinkError> {
        let request = ExportLogsServiceRequest {
            resource_logs: vec![ResourceLogs {
                resource: Some(Resource {
                    attributes: vec![KeyValue {
                        key: "service.name".into(),
                        value: Some(text_value("kartenn")),
                        ..KeyValue::default()
                    }],
                    ..Resource::default()
                }),
                scope_logs: vec![ScopeLogs {
                    scope: Some(InstrumentationScope {
                        name: "kartenn.audit".into(),
                        ..InstrumentationScope::default()
                    }),
                    log_records: events.iter().map(otlp_record).collect(),
                    ..ScopeLogs::default()
                }],
                ..ResourceLogs::default()
            }],
        };
        let mut client = self.client.lock().await;
        if client.is_none() {
            *client = Some(self.connect().await?);
        }
        let Some(connected) = client.as_mut() else {
            return Err(SinkError("not connected".into()));
        };
        match connected.export(request).await {
            Ok(response) => match response.into_inner().partial_success {
                Some(partial) if partial.rejected_log_records > 0 => Err(SinkError(format!(
                    "{} records rejected: {}",
                    partial.rejected_log_records, partial.error_message
                ))),
                _ => Ok(()),
            },
            Err(status) => {
                *client = None;
                Err(sink_err(status))
            }
        }
    }
}

/// Any output of the audit, for a configured list ([`AuditSink`] is not object safe).
#[derive(Debug)]
pub enum AnySink {
    /// [`FileSink`].
    File(FileSink),
    /// [`SyslogSink`].
    Syslog(SyslogSink),
    /// [`OtlpSink`].
    Otlp(OtlpSink),
}

impl AuditSink for AnySink {
    fn name(&self) -> &str {
        match self {
            Self::File(s) => s.name(),
            Self::Syslog(s) => s.name(),
            Self::Otlp(s) => s.name(),
        }
    }

    async fn deliver(&self, events: &[Value]) -> Result<(), SinkError> {
        match self {
            Self::File(s) => s.deliver(events).await,
            Self::Syslog(s) => s.deliver(events).await,
            Self::Otlp(s) => s.deliver(events).await,
        }
    }
}
