//! Outputs of the audit journal (spec 012, REQ-012-15): a file of JSON lines, and syslog
//! (RFC 5424, TCP or TLS with octet counting). Each receives the stored rows of
//! `audit.event`, in sequence order, and returns once they are written.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;

use kartenn_service::audit::{AuditSink, SinkError};
use serde_json::Value;
use syslog_sender::{Facility, Message, SdElement, Sender, Severity, Target};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

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
