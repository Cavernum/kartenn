//! The writers of the technical logs (spec 012, REQ-012-3, 4, 5, 20, 21, 29): standard
//! output (readable text or JSON lines), a file of JSON lines, journald (native protocol)
//! and syslog (RFC 5424). Every one takes the same [`Record`] and writes the same severity
//! (one table, REQ-012-4), name and time.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use kartenn_core::audit::rfc3339;
use kartenn_core::period::Instant;
use serde_json::{Map, Value, json};
use syslog_sender::{Facility, Message, SdElement, Sender, Severity as SyslogSeverity, Target};
use tokio::io::AsyncWriteExt;

use crate::capture::{Record, Writer};

/// Longest syslog message of a technical log (design: 8 KiB, cut at its end).
const SYSLOG_MAX: usize = 8 * 1024;

/// RFC 3339 in UTC, six fractional digits (REQ-012-3).
#[must_use]
pub fn timestamp(time: SystemTime) -> String {
    let micros = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_micros()).unwrap_or(i64::MAX));
    rfc3339(Instant(micros))
}

/// The JSON line of a record (REQ-012-5: `event.name`).
#[must_use]
pub fn json(record: &Record) -> Value {
    let fields: Map<String, Value> = record
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    json!({
        "timestamp": timestamp(record.time),
        "severity": record.severity.as_str(),
        "severity_number": record.severity.otel(),
        "event.name": record.name,
        "target": record.target,
        "message": record.message,
        "fields": fields,
        "trace_id": record.trace_id,
        "span_id": record.span_id,
    })
}

/// Control characters (line feeds included) written as `\u{…}`, so that a value cannot
/// forge a line of its own (CWE-117, REQ-012-29).
#[must_use]
pub fn neutralised(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control() {
            out.push_str(&c.escape_unicode().to_string());
        } else {
            out.push(c);
        }
    }
    out
}

/// The readable line of a record: time, severity, name, message, fields.
#[must_use]
pub fn text(record: &Record) -> String {
    let mut line = format!(
        "{} {:5} {}: {}",
        timestamp(record.time),
        record.severity.as_str(),
        record.name,
        neutralised(&record.message)
    );
    for (k, v) in &record.fields {
        let _ = write!(line, " {k}={}", neutralised(v));
    }
    line
}

/// Form of a stream output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// [`text`].
    Text,
    /// [`json`], one line per record.
    Json,
}

fn lines(records: &[Arc<Record>], format: Format) -> Vec<u8> {
    let mut out = Vec::new();
    for r in records {
        match format {
            Format::Text => out.extend_from_slice(text(r).as_bytes()),
            Format::Json => out.extend_from_slice(json(r).to_string().as_bytes()),
        }
        out.push(b'\n');
    }
    out
}

/// The standard output.
#[derive(Debug, Clone, Copy)]
pub struct Stdout {
    format: Format,
}

impl Stdout {
    /// In `format`.
    #[must_use]
    pub const fn new(format: Format) -> Self {
        Self { format }
    }
}

impl Writer for Stdout {
    async fn write(&mut self, records: &[Arc<Record>]) -> std::io::Result<()> {
        let mut out = tokio::io::stdout();
        out.write_all(&lines(records, self.format)).await?;
        out.flush().await
    }
}

/// A file of JSON lines, opened for each batch so that an external rotation is followed.
#[derive(Debug, Clone)]
pub struct File {
    path: PathBuf,
}

impl File {
    /// Appending to `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Writer for File {
    async fn write(&mut self, records: &[Arc<Record>]) -> std::io::Result<()> {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        file.write_all(&lines(records, Format::Json)).await?;
        // tokio writes on a blocking thread: without this, the write may still be pending
        // when the file is dropped, and a reader finds the file short.
        file.flush().await
    }
}

/// journald, by its native protocol (datagrams of `NAME=value` fields).
#[cfg(unix)]
#[derive(Debug)]
pub struct Journald {
    socket: tokio::net::UnixDatagram,
    path: PathBuf,
}

/// The socket of journald.
#[cfg(unix)]
pub const JOURNALD_SOCKET: &str = "/run/systemd/journal/socket";

#[cfg(unix)]
impl Journald {
    /// To the journald socket at `path` ([`JOURNALD_SOCKET`] in production).
    ///
    /// # Errors
    ///
    /// The local socket cannot be created.
    pub fn new(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        Ok(Self {
            socket: tokio::net::UnixDatagram::unbound()?,
            path: path.into(),
        })
    }

    /// The datagram of a record: `MESSAGE`, `PRIORITY` (table of REQ-012-4),
    /// `SYSLOG_IDENTIFIER`, `MESSAGE_ID` (128 bits derived from the name), the name and
    /// the fields under `KARTENN_*`.
    #[must_use]
    pub fn datagram(record: &Record) -> Vec<u8> {
        let mut out = Vec::new();
        let mut field = |name: &str, value: &str| {
            if value.contains('\n') {
                // Binary form: name, newline, length (64 bits, little endian), value.
                out.extend_from_slice(name.as_bytes());
                out.push(b'\n');
                out.extend_from_slice(&(value.len() as u64).to_le_bytes());
                out.extend_from_slice(value.as_bytes());
                out.push(b'\n');
            } else {
                out.extend_from_slice(name.as_bytes());
                out.push(b'=');
                out.extend_from_slice(value.as_bytes());
                out.push(b'\n');
            }
        };
        field("MESSAGE", &record.message);
        field("PRIORITY", &record.severity.journald().to_string());
        field("SYSLOG_IDENTIFIER", "kartenn");
        field("MESSAGE_ID", &message_id(record.name));
        field("KARTENN_EVENT_NAME", record.name);
        field("KARTENN_TARGET", record.target);
        if let Some(trace) = &record.trace_id {
            field("TRACE_ID", trace);
        }
        for (k, v) in &record.fields {
            field(&format!("KARTENN_{}", journald_name(k)), v);
        }
        out
    }
}

/// A journald field name: uppercase letters, digits and `_`, not starting with `_`.
fn journald_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    if out.starts_with('_') {
        out.insert(0, 'X');
    }
    out
}

/// A stable 128-bit identifier of an event name, in hexadecimal (journald `MESSAGE_ID`).
fn message_id(name: &str) -> String {
    use sha2::{Digest, Sha256};
    kartenn_core::audit::seal::hex(&Sha256::digest(name.as_bytes())[..16])
}

#[cfg(unix)]
impl Writer for Journald {
    async fn write(&mut self, records: &[Arc<Record>]) -> std::io::Result<()> {
        for r in records {
            self.socket.send_to(&Self::datagram(r), &self.path).await?;
        }
        Ok(())
    }
}

/// Syslog RFC 5424 (012/L19): `MSGID` the event name (cut to 32 octets), `meta
/// sequenceId`, the record in JSON as MSG.
#[derive(Debug)]
pub struct Syslog {
    sender: Sender,
    hostname: Option<String>,
    facility: Facility,
    sequence: u32,
}

impl Syslog {
    /// To `target`, as `facility`, naming `hostname`.
    #[must_use]
    pub fn new(target: Target, facility: Facility, hostname: Option<String>) -> Self {
        Self {
            sender: Sender::new(target),
            hostname,
            facility,
            sequence: 0,
        }
    }

    /// The message of a record.
    ///
    /// # Errors
    ///
    /// A field the RFC refuses (an empty host name…).
    pub fn message(&mut self, record: &Record) -> Result<Vec<u8>, syslog_sender::Error> {
        let severity = match record.severity.syslog() {
            3 => SyslogSeverity::Error,
            4 => SyslogSeverity::Warning,
            6 => SyslogSeverity::Informational,
            _ => SyslogSeverity::Debug,
        };
        self.sequence = self.sequence % 2_147_483_647 + 1;
        let mut msgid: String = record.name.chars().filter(char::is_ascii_graphic).collect();
        msgid.truncate(32);
        let mut message = Message::new(self.facility, severity)
            .timestamp(record.time)
            .app_name("kartenn")?
            .procid(&std::process::id().to_string())?
            .structured(SdElement::meta_sequence(self.sequence)?)?
            .text(json(record).to_string());
        if !msgid.is_empty() && msgid != "-" {
            message = message.msgid(&msgid)?;
        }
        if let Some(host) = &self.hostname {
            message = message.hostname(host)?;
        }
        message.to_rfc5424(SYSLOG_MAX)
    }
}

impl Writer for Syslog {
    async fn write(&mut self, records: &[Arc<Record>]) -> std::io::Result<()> {
        let mut messages = Vec::with_capacity(records.len());
        for r in records {
            messages.push(
                self.message(r)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
            );
        }
        self.sender.send(&messages).await
    }
}
