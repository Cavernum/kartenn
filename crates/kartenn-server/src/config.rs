//! Configuration of the server (spec 012, REQ-012-31; D35): a TOML file, then the
//! variables `KARTENN_*` (nesting by `__`). An unknown output, a missing field or a filter
//! that does not parse stops the start; without a `telemetry` section, the technical logs
//! go to the standard output in JSON at `info`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use figment::Figment;
use figment::providers::{Env, Format, Toml};
use serde::Deserialize;

/// Why the configuration is refused.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Unreadable, unknown output kind, missing or unknown field.
    #[error(transparent)]
    Load(#[from] Box<figment::Error>),
    /// A value that parses but cannot be used.
    #[error("{0}")]
    Invalid(String),
}

/// The whole configuration.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Technical logs and OpenTelemetry.
    #[serde(default)]
    pub telemetry: Telemetry,
    /// Audit journal.
    #[serde(default)]
    pub audit: Audit,
}

/// Technical logs and OpenTelemetry (REQ-012-20, 24).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Telemetry {
    /// Outputs, all active at once.
    #[serde(default = "default_outputs")]
    pub outputs: Vec<Output>,
    /// Share of the traces sampled at the root.
    #[serde(default = "one")]
    pub sampling_ratio: f64,
    /// Queue of each output, in records.
    #[serde(default = "queue")]
    pub queue: usize,
    /// Between two readings of the filters changed at run time (REQ-012-27).
    #[serde(default = "override_poll_ms")]
    pub override_poll_ms: u64,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            outputs: default_outputs(),
            sampling_ratio: one(),
            queue: queue(),
            override_poll_ms: override_poll_ms(),
        }
    }
}

const fn override_poll_ms() -> u64 {
    10_000
}

impl Telemetry {
    /// The name of each output, in order: its kind, numbered from the second of a kind
    /// (`file`, `file-2`); the key of its counters and of its overrides.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let mut seen = std::collections::HashMap::new();
        self.outputs
            .iter()
            .map(|o| {
                let n = seen.entry(o.kind()).or_insert(0);
                *n += 1;
                if *n == 1 {
                    o.kind().to_owned()
                } else {
                    format!("{}-{n}", o.kind())
                }
            })
            .collect()
    }
}

fn default_outputs() -> Vec<Output> {
    vec![Output::Stdout {
        format: StreamFormat::Json,
        filter: info(),
    }]
}

const fn one() -> f64 {
    1.0
}

const fn queue() -> usize {
    16_384
}

fn info() -> String {
    "info".into()
}

/// Readable text or JSON lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamFormat {
    /// Text.
    Text,
    /// JSON lines.
    Json,
}

/// How syslog travels (REQ-012-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// Local datagram socket; `address` is its path (`/dev/log`).
    Local,
    /// UDP, `host:port`.
    Udp,
    /// TCP, `host:port`, octet counting.
    Tcp,
    /// TLS, `host:port`, octet counting; `ca_file` holds the roots.
    Tls,
}

/// A technical output.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Output {
    /// Standard output.
    Stdout {
        /// Text or JSON.
        #[serde(default = "json")]
        format: StreamFormat,
        /// `EnvFilter` syntax.
        #[serde(default = "info")]
        filter: String,
    },
    /// File of JSON lines.
    File {
        /// Path.
        path: PathBuf,
        /// Filter.
        #[serde(default = "info")]
        filter: String,
    },
    /// journald.
    Journald {
        /// Socket, `/run/systemd/journal/socket` by default.
        socket: Option<PathBuf>,
        /// Filter.
        #[serde(default = "info")]
        filter: String,
    },
    /// Syslog RFC 5424.
    Syslog {
        /// Transport.
        transport: Transport,
        /// Path or `host:port`.
        address: String,
        /// Name in the certificate (TLS), the host of `address` by default.
        server_name: Option<String>,
        /// PEM roots (TLS).
        ca_file: Option<PathBuf>,
        /// Filter.
        #[serde(default = "info")]
        filter: String,
    },
    /// OpenTelemetry (traces, metrics, logs) in OTLP over gRPC.
    Otlp {
        /// `http://host:4317` or `https://…`; `OTEL_EXPORTER_OTLP_ENDPOINT` by default.
        endpoint: Option<String>,
        /// Filter of the logs.
        #[serde(default = "info")]
        filter: String,
    },
}

const fn json() -> StreamFormat {
    StreamFormat::Json
}

impl Output {
    /// Its filter.
    #[must_use]
    pub fn filter(&self) -> &str {
        match self {
            Self::Stdout { filter, .. }
            | Self::File { filter, .. }
            | Self::Journald { filter, .. }
            | Self::Syslog { filter, .. }
            | Self::Otlp { filter, .. } => filter,
        }
    }

    /// Its name, for metrics and events.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Stdout { .. } => "stdout",
            Self::File { .. } => "file",
            Self::Journald { .. } => "journald",
            Self::Syslog { .. } => "syslog",
            Self::Otlp { .. } => "otlp",
        }
    }
}

/// Audit journal (REQ-012-13 to 16).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
    /// Outputs of the audit.
    #[serde(default)]
    pub sinks: Vec<Sink>,
    /// PKCS#8 PEM key signing the seals (optional, 012/L23).
    pub signing_key_file: Option<PathBuf>,
    /// Between two sealings.
    #[serde(default = "seal_ms")]
    pub seal_interval_ms: u64,
    /// Between two relay passes.
    #[serde(default = "relay_ms")]
    pub relay_interval_ms: u64,
    /// Retention by criticality, in days.
    #[serde(default)]
    pub retention_days: RetentionDays,
    /// Most identifiers a read event records, the others counted; `0` records the
    /// criteria alone (012/L25: the lever on the volume of the audit of reads).
    #[serde(default = "read_ids_max")]
    pub read_ids_max: usize,
}

const fn read_ids_max() -> usize {
    kartenn_service::reads::DEFAULT_READ_IDS_MAX
}

impl Audit {
    /// What a read event records, for the reads of the service.
    #[must_use]
    pub const fn read_audit(&self) -> kartenn_service::reads::ReadAudit {
        kartenn_service::reads::ReadAudit {
            ids_max: self.read_ids_max,
        }
    }
}

impl Default for Audit {
    fn default() -> Self {
        Self {
            sinks: Vec::new(),
            signing_key_file: None,
            seal_interval_ms: seal_ms(),
            relay_interval_ms: relay_ms(),
            retention_days: RetentionDays::default(),
            read_ids_max: read_ids_max(),
        }
    }
}

const fn seal_ms() -> u64 {
    15_000
}

const fn relay_ms() -> u64 {
    1_000
}

/// Retention by criticality, in days (012/L22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionDays {
    /// Ordinary reads.
    pub low: u64,
    /// Writes, operation.
    pub normal: u64,
    /// Refusals, administration, sensitive reads.
    pub high: u64,
}

impl Default for RetentionDays {
    fn default() -> Self {
        Self {
            low: 90,
            normal: 365,
            high: 1095,
        }
    }
}

impl RetentionDays {
    /// As durations.
    #[must_use]
    pub fn retention(&self) -> kartenn_store_pg::audit_purge::Retention {
        let day = Duration::from_hours(24);
        kartenn_store_pg::audit_purge::Retention {
            low: day * u32::try_from(self.low).unwrap_or(u32::MAX),
            normal: day * u32::try_from(self.normal).unwrap_or(u32::MAX),
            high: day * u32::try_from(self.high).unwrap_or(u32::MAX),
        }
    }
}

/// An output of the audit.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Sink {
    /// File of JSON lines.
    File {
        /// Name, the key of its deliveries: never change it.
        name: String,
        /// Path.
        path: PathBuf,
    },
    /// Syslog RFC 5424.
    Syslog {
        /// Name.
        name: String,
        /// Transport (not `udp`: no guarantee).
        transport: Transport,
        /// Path or `host:port`.
        address: String,
        /// Name in the certificate.
        server_name: Option<String>,
        /// PEM roots.
        ca_file: Option<PathBuf>,
    },
    /// OTLP logs over gRPC.
    Otlp {
        /// Name.
        name: String,
        /// `http://host:4317` or `https://…`.
        endpoint: String,
    },
}

impl Sink {
    /// Its name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::File { name, .. } | Self::Syslog { name, .. } | Self::Otlp { name, .. } => name,
        }
    }
}

/// Loads `file` (if any) then the `KARTENN_*` variables, and checks the result.
///
/// # Errors
///
/// [`ConfigError`], naming the faulty setting.
pub fn load(file: Option<&Path>) -> Result<Config, ConfigError> {
    let mut figment = Figment::new();
    if let Some(file) = file {
        figment = figment.merge(Toml::file_exact(file));
    }
    let config: Config = figment
        .merge(Env::prefixed("KARTENN_").split("__"))
        .extract()
        .map_err(Box::new)?;
    check(&config)?;
    Ok(config)
}

/// Loads a configuration from TOML text (tests, examples).
///
/// # Errors
///
/// As [`load`].
pub fn from_toml(text: &str) -> Result<Config, ConfigError> {
    let config: Config = Figment::from(Toml::string(text))
        .extract()
        .map_err(Box::new)?;
    check(&config)?;
    Ok(config)
}

fn check(config: &Config) -> Result<(), ConfigError> {
    let invalid = |m: String| Err(ConfigError::Invalid(m));
    if !(0.0..=1.0).contains(&config.telemetry.sampling_ratio) {
        return invalid("telemetry.sampling_ratio must lie between 0 and 1".into());
    }
    if config.telemetry.outputs.is_empty() {
        return invalid("telemetry.outputs: at least one output".into());
    }
    for output in &config.telemetry.outputs {
        if let Err(e) = tracing_subscriber::EnvFilter::try_new(output.filter()) {
            return invalid(format!("filter of the {} output: {e}", output.kind()));
        }
        if let Output::Syslog {
            transport: Transport::Tls,
            ca_file: None,
            ..
        } = output
        {
            return invalid("a syslog output over TLS needs ca_file".into());
        }
    }
    let mut names = std::collections::BTreeSet::new();
    for sink in &config.audit.sinks {
        if !names.insert(sink.name()) {
            return invalid(format!("two audit outputs are named {}", sink.name()));
        }
        match sink {
            Sink::Syslog {
                transport: Transport::Udp,
                ..
            } => return invalid("an audit output cannot use UDP (no guarantee)".into()),
            Sink::Syslog {
                transport: Transport::Tls,
                ca_file: None,
                ..
            } => return invalid("a syslog audit output over TLS needs ca_file".into()),
            _ => {}
        }
    }
    if config.audit.seal_interval_ms == 0 || config.audit.relay_interval_ms == 0 {
        return invalid("audit intervals must be positive".into());
    }
    let days = &config.audit.retention_days;
    if days.low == 0 || days.normal == 0 || days.high == 0 {
        // A retention of zero days would purge every sealed and exported event.
        return invalid("audit.retention_days: at least one day for each criticality".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_telemetry_logs_go_to_stdout_in_json() {
        let config = from_toml("").unwrap();
        assert_eq!(
            config.telemetry.outputs,
            vec![Output::Stdout {
                format: StreamFormat::Json,
                filter: "info".into()
            }]
        );
        assert!(config.audit.sinks.is_empty());
        assert_eq!(config.audit.retention_days.low, 90);
    }

    #[test]
    fn several_outputs_at_once() {
        let config = from_toml(
            r#"
            [telemetry]
            outputs = [
              { kind = "stdout", format = "text", filter = "warn" },
              { kind = "syslog", transport = "tcp", address = "127.0.0.1:5514" },
              { kind = "otlp" },
            ]
            [audit]
            sinks = [{ kind = "otlp", name = "collector", endpoint = "http://127.0.0.1:4317" }]
            "#,
        )
        .unwrap();
        assert_eq!(config.telemetry.outputs.len(), 3);
        assert_eq!(config.audit.sinks[0].name(), "collector");
    }

    #[test]
    fn a_faulty_output_stops_the_start_with_its_reason() {
        let cases = [
            (r#"telemetry.outputs = [{ kind = "graylog" }]"#, "graylog"),
            (
                r#"telemetry.outputs = [{ kind = "syslog", transport = "tcp" }]"#,
                "address",
            ),
            (
                r#"telemetry.outputs = [{ kind = "stdout", filter = "x=y=z" }]"#,
                "filter",
            ),
            (
                r#"telemetry.outputs = [{ kind = "syslog", transport = "tls", address = "h:6514" }]"#,
                "ca_file",
            ),
            (
                r#"audit.sinks = [{ kind = "syslog", name = "s", transport = "udp", address = "h:514" }]"#,
                "UDP",
            ),
            (
                r#"audit.sinks = [{ kind = "file", name = "a", path = "/x" }, { kind = "file", name = "a", path = "/y" }]"#,
                "two audit outputs",
            ),
            ("telemetry.sampling_ratio = 2.0", "sampling_ratio"),
            (
                "audit.retention_days = { low = 0, normal = 365, high = 1095 }",
                "retention_days",
            ),
            ("unknown = 1", "unknown"),
        ];
        for (text, reason) in cases {
            let error = from_toml(text).unwrap_err().to_string();
            assert!(error.contains(reason), "{text}: {error}");
        }
    }

    #[test]
    fn the_identifiers_of_a_read_event_are_bounded_by_configuration() {
        assert_eq!(from_toml("").unwrap().audit.read_audit().ids_max, 100);
        let config = from_toml("audit.read_ids_max = 0").unwrap();
        assert_eq!(config.audit.read_audit().ids_max, 0, "criteria alone");
        let config =
            from_toml("audit.retention_days = { low = 30, normal = 365, high = 1095 }").unwrap();
        assert_eq!(config.audit.retention_days.low, 30);
    }
}
