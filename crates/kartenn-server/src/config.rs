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
    /// Authentication (spec 005); without it, no one can log in.
    #[serde(default)]
    pub auth: Option<Auth>,
    /// The HTTP API (spec 006); without it, the instance runs the background work alone.
    #[serde(default)]
    pub http: Option<Http>,
}

/// The HTTP API (spec 006, design « Configuration »).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Http {
    /// Address and port to listen on.
    pub listen: std::net::SocketAddr,
    /// The public origin (`https://host[:port]`): CSRF, absolute links, default
    /// `redirect_uri`.
    pub public_origin: Option<String>,
    /// Proxies whose `Forwarded` and `traceparent` are believed, as addresses or CIDR
    /// blocks.
    #[serde(default)]
    pub trusted_proxies: Vec<kartenn_rest::transport::TrustedProxy>,
    /// Largest request body.
    #[serde(default = "max_body_bytes")]
    pub max_body_bytes: usize,
    /// Longest request.
    #[serde(default = "request_timeout_ms")]
    pub request_timeout_ms: u64,
    /// Longest write transaction, in seconds.
    #[serde(default = "write_transaction_timeout_s")]
    pub write_transaction_timeout_s: u64,
    /// Most requests in flight; `0` is twice the size of the pool.
    #[serde(default)]
    pub max_in_flight: usize,
    /// TLS terminated by kartenn; absent, clear HTTP behind a proxy.
    pub tls: Option<Tls>,
    /// Quotas by identity.
    #[serde(default)]
    pub rate: Rates,
}

const fn max_body_bytes() -> usize {
    16 * 1024 * 1024
}

const fn request_timeout_ms() -> u64 {
    30_000
}

const fn write_transaction_timeout_s() -> u64 {
    60
}

/// Certificate and key, PEM, read again on `SIGHUP`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tls {
    /// The certificate chain, leaf first.
    pub cert_file: PathBuf,
    /// The private key (PKCS#8, PKCS#1 or SEC1).
    pub key_file: PathBuf,
}

/// Quotas by identity (GCRA, REQ-006-29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rates {
    /// Reads of an authenticated identity.
    #[serde(default = "read_rate")]
    pub read: Rate,
    /// Writes of an authenticated identity.
    #[serde(default = "write_rate")]
    pub write: Rate,
    /// Requests without identity, by client address.
    #[serde(default = "anonymous_rate")]
    pub anonymous: Rate,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            read: read_rate(),
            write: write_rate(),
            anonymous: anonymous_rate(),
        }
    }
}

/// A sustained rate and the burst above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rate {
    /// Requests per second, sustained.
    pub per_second: u32,
    /// Requests at once.
    pub burst: u32,
}

const fn read_rate() -> Rate {
    Rate {
        per_second: 50,
        burst: 200,
    }
}

const fn write_rate() -> Rate {
    Rate {
        per_second: 10,
        burst: 20,
    }
}

const fn anonymous_rate() -> Rate {
    Rate {
        per_second: 20,
        burst: 20,
    }
}

/// Authentication (spec 005, design « Configuration »): the key ring, the sessions and the
/// OIDC issuers. Their confinement lives in the base (K11).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Auth {
    /// The key ring: one key of 256 bits per line, the first one current.
    pub key_file: PathBuf,
    /// Inactivity that closes a session, in minutes (1 to 60; REQ-005-6).
    #[serde(default = "idle_minutes")]
    pub session_idle_minutes: u64,
    /// Lifetime of a session whatever its activity, in hours (1 to 24; REQ-005-6).
    #[serde(default = "absolute_hours")]
    pub session_absolute_hours: u64,
    /// Where the issuers send the browser back, compared exactly (RFC 9700 §4.1.3).
    pub redirect_uri: Option<String>,
    /// The issuers.
    #[serde(default)]
    pub issuers: Vec<Issuer>,
}

/// An OIDC issuer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issuer {
    /// Local name: lowercase letters, digits and dashes.
    pub id: String,
    /// Issuer identifier (`iss`).
    pub issuer: String,
    /// Client of kartenn at this issuer.
    pub client_id: String,
    /// File holding its secret.
    pub client_secret_file: PathBuf,
    /// Audience of the access tokens of the clients other than browsers.
    pub audience: String,
    /// Claim holding the groups.
    #[serde(default = "groups_claim")]
    pub groups_claim: String,
    /// Algorithms of its signatures, public key only.
    #[serde(default = "algorithms")]
    pub algorithms: Vec<String>,
}

const fn idle_minutes() -> u64 {
    30
}

const fn absolute_hours() -> u64 {
    8
}

fn groups_claim() -> String {
    "groups".into()
}

fn algorithms() -> Vec<String> {
    vec!["RS256".into(), "ES256".into()]
}

/// Whether `url` is `https`, or `http` on the loopback for development (RFC 8252 §7.3),
/// without fragment (RFC 6749 §3.1.2).
fn secure_url(url: &str) -> bool {
    let loopback = ["http://127.0.0.1", "http://[::1]", "http://localhost"]
        .iter()
        .any(|base| {
            url.strip_prefix(base)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([':', '/']))
        });
    (url.len() > "https://".len() && url.starts_with("https://") || loopback) && !url.contains('#')
}

fn check_auth(auth: &Auth) -> Result<(), String> {
    if !(1..=60).contains(&auth.session_idle_minutes) {
        return Err("auth.session_idle_minutes: from 1 to 60".into());
    }
    if !(1..=24).contains(&auth.session_absolute_hours) {
        return Err("auth.session_absolute_hours: from 1 to 24".into());
    }
    if auth.session_idle_minutes > auth.session_absolute_hours * 60 {
        return Err("auth.session_idle_minutes exceeds the absolute duration".into());
    }
    match &auth.redirect_uri {
        None if !auth.issuers.is_empty() => {
            return Err("auth.redirect_uri: required with issuers".into());
        }
        Some(uri) if !secure_url(uri) => {
            return Err("auth.redirect_uri: https, without fragment".into());
        }
        _ => {}
    }
    let (mut ids, mut urls) = (
        std::collections::BTreeSet::new(),
        std::collections::BTreeSet::new(),
    );
    for issuer in &auth.issuers {
        let id = &issuer.id;
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(format!("auth.issuers: {id:?} is not a local name"));
        }
        if !ids.insert(id) {
            return Err(format!("auth.issuers: two issuers are named {id}"));
        }
        if !urls.insert(issuer.issuer.trim_end_matches('/')) {
            return Err(format!(
                "auth.issuers.{id}: another issuer has this identifier"
            ));
        }
        if !secure_url(&issuer.issuer) || issuer.issuer.contains('?') {
            // OpenID Connect Discovery 1.0 §3: https, no query, no fragment.
            return Err(format!("auth.issuers.{id}.issuer: https, without query"));
        }
        if issuer.client_id.is_empty()
            || issuer.audience.is_empty()
            || issuer.groups_claim.is_empty()
        {
            return Err(format!(
                "auth.issuers.{id}: client_id, audience and groups_claim are required"
            ));
        }
        if issuer.algorithms.is_empty() {
            return Err(format!("auth.issuers.{id}.algorithms: at least one"));
        }
        if let Some(refused) = issuer
            .algorithms
            .iter()
            .find(|a| kartenn_auth::issuer::accepted_algorithm(a).is_none())
        {
            return Err(format!(
                "auth.issuers.{id}.algorithms: {refused} refused (public key signatures only)"
            ));
        }
    }
    Ok(())
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
    if let Some(auth) = &config.auth {
        check_auth(auth).map_err(ConfigError::Invalid)?;
    }
    if let Some(http) = &config.http {
        check_http(http).map_err(ConfigError::Invalid)?;
    }
    Ok(())
}

fn check_http(http: &Http) -> Result<(), String> {
    if http.max_body_bytes == 0
        || http.request_timeout_ms == 0
        || http.write_transaction_timeout_s == 0
    {
        return Err("http: limits and timeouts must be positive".into());
    }
    if let Some(origin) = &http.public_origin
        && (!secure_url(origin)
            || origin
                .split_once("://")
                .is_some_and(|(_, host)| host.contains('/')))
    {
        return Err(format!(
            "http.public_origin: an origin in https (or http on loopback), without path: {origin}"
        ));
    }
    for (name, rate) in [
        ("read", http.rate.read),
        ("write", http.rate.write),
        ("anonymous", http.rate.anonymous),
    ] {
        if rate.per_second == 0 || rate.burst == 0 {
            return Err(format!(
                "http.rate.{name}: per_second and burst must be positive"
            ));
        }
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

    const AUTH: &str = r#"
        [auth]
        key_file = "/run/secrets/kartenn-auth.keys"
        redirect_uri = "https://kartenn.example.invalid/auth/callback"
        [[auth.issuers]]
        id = "internal"
        issuer = "https://idp.example.invalid/realms/main"
        client_id = "kartenn"
        client_secret_file = "/run/secrets/oidc-internal"
        audience = "kartenn"
    "#;

    #[test]
    fn an_auth_section_takes_the_defaults_of_the_design() {
        assert_eq!(from_toml("").unwrap().auth, None);
        let auth = from_toml(AUTH).unwrap().auth.unwrap();
        assert_eq!(
            (auth.session_idle_minutes, auth.session_absolute_hours),
            (30, 8)
        );
        assert_eq!(auth.issuers[0].groups_claim, "groups");
        assert_eq!(auth.issuers[0].algorithms, ["RS256", "ES256"]);
        let local = AUTH
            .replace("https://kartenn.example.invalid", "http://127.0.0.1:8080")
            .replace("https://idp.example.invalid", "http://localhost:8180");
        assert!(from_toml(&local).is_ok(), "the loopback, for development");
    }

    #[test]
    fn a_faulty_auth_section_stops_the_start_with_its_reason() {
        let second = r#"
            [[auth.issuers]]
            id = "internal"
            issuer = "https://other.example.invalid"
            client_id = "kartenn"
            client_secret_file = "/x"
            audience = "kartenn"
        "#;
        let cases = [
            (AUTH.replace("key_file", "keyfile"), "keyfile"),
            (
                AUTH.replace("[auth]", "[auth]\nsession_idle_minutes = 0"),
                "session_idle_minutes",
            ),
            (
                AUTH.replace("[auth]", "[auth]\nsession_idle_minutes = 90"),
                "session_idle_minutes",
            ),
            (
                AUTH.replace("[auth]", "[auth]\nsession_absolute_hours = 48"),
                "session_absolute_hours",
            ),
            (
                AUTH.replace(
                    "redirect_uri = \"https://kartenn.example.invalid/auth/callback\"",
                    "",
                ),
                "redirect_uri: required",
            ),
            (
                AUTH.replace(
                    "https://kartenn.example.invalid",
                    "http://kartenn.example.invalid",
                ),
                "redirect_uri",
            ),
            (AUTH.replace("callback", "callback#x"), "fragment"),
            (
                AUTH.replace("https://idp.example.invalid", "http://idp.example.invalid"),
                "issuer: https",
            ),
            (
                AUTH.replace("realms/main", "realms/main?x=1"),
                "without query",
            ),
            (
                AUTH.replace("id = \"internal\"", "id = \"Internal IdP\""),
                "local name",
            ),
            (format!("{AUTH}{second}"), "two issuers"),
            (
                format!(
                    "{AUTH}{}",
                    second
                        .replace("\"internal\"", "\"partner\"")
                        .replace("other.example.invalid", "idp.example.invalid/realms/main/")
                ),
                "another issuer",
            ),
            (
                AUTH.replace("audience = \"kartenn\"", "audience = \"\""),
                "audience",
            ),
            (format!("{AUTH}algorithms = []"), "at least one"),
            (
                format!("{AUTH}algorithms = [\"RS256\", \"HS256\"]"),
                "HS256 refused",
            ),
            (format!("{AUTH}algorithms = [\"none\"]"), "none refused"),
        ];
        for (text, reason) in cases {
            let error = from_toml(&text).unwrap_err().to_string();
            assert!(error.contains(reason), "{reason}: {error}");
        }
    }

    const HTTP: &str = r#"
        [http]
        listen = "127.0.0.1:8443"
        public_origin = "https://kartenn.example"
        [http.tls]
        cert_file = "/etc/kartenn/tls.crt"
        key_file = "/etc/kartenn/tls.key"
    "#;

    #[test]
    fn an_http_section_takes_the_defaults_of_the_design() {
        assert_eq!(from_toml("").unwrap().http, None);
        let http = from_toml(HTTP).unwrap().http.unwrap();
        assert_eq!(http.listen, "127.0.0.1:8443".parse().unwrap());
        assert_eq!(http.max_body_bytes, 16_777_216);
        assert_eq!(http.request_timeout_ms, 30_000);
        assert_eq!(http.write_transaction_timeout_s, 60);
        assert_eq!(http.max_in_flight, 0);
        assert_eq!(
            http.rate.write,
            Rate {
                per_second: 10,
                burst: 20
            }
        );
        assert!(http.trusted_proxies.is_empty());
        assert_eq!(
            http.tls.unwrap().key_file,
            PathBuf::from("/etc/kartenn/tls.key")
        );
    }

    #[test]
    fn a_faulty_http_section_stops_the_start_with_its_reason() {
        let cases = [
            (HTTP.replace("listen", "address"), "address"),
            (HTTP.replace("cert_file", "certificate"), "certificate"),
            (
                HTTP.replace("[http.tls]", "[http.tls]\nca_file = \"/x\""),
                "ca_file",
            ),
            (
                HTTP.replace("8443\"", "8443\"\nmax_body_bytes = 0"),
                "positive",
            ),
            (
                HTTP.replace("https://kartenn.example", "http://kartenn.example"),
                "public_origin",
            ),
            (
                HTTP.replace("kartenn.example", "kartenn.example/api"),
                "public_origin",
            ),
            (
                format!("{HTTP}\n[http.rate]\nread = {{ per_second = 0, burst = 1 }}"),
                "http.rate.read",
            ),
            (
                HTTP.replace("8443\"", "8443\"\ntrusted_proxies = [\"10.0.0.1/8\"]"),
                "10.0.0.1/8",
            ),
        ];
        for (text, reason) in cases {
            let error = from_toml(&text).unwrap_err().to_string();
            assert!(error.contains(reason), "{reason}: {error}");
        }
    }
}
