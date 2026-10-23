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
    /// The web shell and its plugins (spec 007); without it, nothing is served outside
    /// `/v1` and `/health`.
    #[serde(default)]
    pub web: Option<Web>,
    /// Where the database is (spec 015); without it, its URL is the argument of the
    /// subcommand.
    #[serde(default)]
    pub database: Option<Database>,
    /// The event stream `GET /v1/events` (spec 022).
    #[serde(default)]
    pub events: Events,
    /// Languages (spec 018, REQ-018-12).
    #[serde(default)]
    pub i18n: I18n,
}

/// The event stream (spec 022): on by default with the API.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Events {
    /// Serve `GET /v1/events` (REQ-022-1); off, the route answers 404.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Keep-alive after this silence, 5 to 60 s (REQ-022-10).
    #[serde(default = "heartbeat_s")]
    pub heartbeat_s: u64,
    /// Most objects an event names before it names types (REQ-022-5).
    #[serde(default = "event_objects")]
    pub event_objects: usize,
    /// How long the ring keeps an event, in seconds (REQ-022-7).
    #[serde(default = "ring_seconds")]
    pub ring_seconds: u64,
    /// Most events in the ring.
    #[serde(default = "ring_events")]
    pub ring_events: usize,
    /// Events waiting per connection (REQ-022-21).
    #[serde(default = "buffer")]
    pub buffer: usize,
    /// Open streams per instance (REQ-022-22).
    #[serde(default = "max_streams")]
    pub max_streams: usize,
    /// Open streams per identity.
    #[serde(default = "per_identity")]
    pub per_identity: usize,
    /// The fallback poll of the reader, in milliseconds (REQ-022-20).
    #[serde(default = "poll_ms")]
    pub poll_ms: u64,
    /// `retry:` at the opening, drawn between these milliseconds (REQ-022-11).
    #[serde(default = "retry_min_ms")]
    pub retry_min_ms: u64,
    /// See [`Self::retry_min_ms`].
    #[serde(default = "retry_max_ms")]
    pub retry_max_ms: u64,
    /// `Incremental: ?1` on the stream (RFC 10036).
    #[serde(default)]
    pub incremental: bool,
}

const fn yes() -> bool {
    true
}
const fn heartbeat_s() -> u64 {
    15
}
const fn event_objects() -> usize {
    kartenn_service::events::DEFAULT_BOUND
}
const fn ring_seconds() -> u64 {
    900
}
const fn ring_events() -> usize {
    10_000
}
const fn buffer() -> usize {
    256
}
const fn max_streams() -> usize {
    5_000
}
const fn per_identity() -> usize {
    16
}
const fn poll_ms() -> u64 {
    1_000
}
const fn retry_min_ms() -> u64 {
    1_000
}
const fn retry_max_ms() -> u64 {
    3_000
}

impl Default for Events {
    fn default() -> Self {
        Self {
            enabled: true,
            heartbeat_s: heartbeat_s(),
            event_objects: event_objects(),
            ring_seconds: ring_seconds(),
            ring_events: ring_events(),
            buffer: buffer(),
            max_streams: max_streams(),
            per_identity: per_identity(),
            poll_ms: poll_ms(),
            retry_min_ms: retry_min_ms(),
            retry_max_ms: retry_max_ms(),
            incremental: false,
        }
    }
}

impl Events {
    /// The settings of the hub, `public_origin` that of `[http]`.
    #[must_use]
    pub fn settings(&self, public_origin: Option<String>) -> kartenn_rest::events::hub::Settings {
        kartenn_rest::events::hub::Settings {
            heartbeat: std::time::Duration::from_secs(self.heartbeat_s),
            bound: self.event_objects,
            ring_age: std::time::Duration::from_secs(self.ring_seconds),
            ring_max: self.ring_events,
            buffer: self.buffer,
            max_streams: self.max_streams,
            per_identity: self.per_identity,
            poll: std::time::Duration::from_millis(self.poll_ms),
            retry_open: (self.retry_min_ms, self.retry_max_ms),
            incremental: self.incremental,
            public_origin,
            ..kartenn_rest::events::hub::Settings::default()
        }
    }
}

fn check_events(events: &Events) -> Result<(), String> {
    if !(5..=60).contains(&events.heartbeat_s) {
        return Err("events.heartbeat_s: between 5 and 60".into());
    }
    if events.event_objects == 0
        || events.ring_seconds == 0
        || events.ring_events == 0
        || events.buffer == 0
        || events.max_streams == 0
        || events.per_identity == 0
        || events.poll_ms == 0
    {
        return Err("events: bounds and intervals must be positive".into());
    }
    if events.retry_min_ms > events.retry_max_ms {
        return Err("events.retry_min_ms: at most retry_max_ms".into());
    }
    Ok(())
}

/// Languages of the instance (spec 018, R9, REQ-018-12).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct I18n {
    /// The default language (BCP 47 tag): required in every `labels` table, served when
    /// no language asked matches, and the language of the pedagogical texts of the
    /// policy; `fr` by default.
    #[serde(default = "default_locale")]
    pub default_locale: String,
}

fn default_locale() -> String {
    kartenn_core::labels::FALLBACK_LOCALE.to_owned()
}

impl Default for I18n {
    fn default() -> Self {
        Self {
            default_locale: default_locale(),
        }
    }
}

impl I18n {
    /// The default language, checked by [`load`].
    ///
    /// # Panics
    /// On a tag that [`load`] would have refused.
    #[must_use]
    pub fn locale(&self) -> kartenn_core::labels::Locale {
        kartenn_core::labels::Locale::parse(&self.default_locale).expect("checked at load")
    }

    /// Makes the default language that of the process (core, modules, API), once at
    /// start.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] when another language was installed before.
    pub fn install(&self) -> Result<(), ConfigError> {
        kartenn_core::labels::set_default_locale(self.locale()).map_err(|current| {
            ConfigError::Invalid(format!(
                "i18n.default_locale: {current} already in force in this process"
            ))
        })
    }
}

/// Where the database is (spec 015, REQ-015-4, design « Configuration de la base »).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Database {
    /// A file holding the URL of the database (`postgres://…`), read at start: the URL,
    /// which may carry a password, then stays off the command line (readable by every
    /// user of the machine) and out of logs and errors.
    pub url_file: Option<PathBuf>,
}

impl Config {
    /// The URL of the database: the `argument` of the subcommand or the content of
    /// `[database] url_file`, exactly one of them.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Invalid`] for neither or both, or a file unreadable or empty; the
    /// message names the file, never its content.
    pub fn database_url(&self, argument: Option<&str>) -> Result<String, ConfigError> {
        let file = self.database.as_ref().and_then(|d| d.url_file.as_deref());
        match (argument, file) {
            (Some(url), None) => Ok(url.to_owned()),
            (None, Some(file)) => {
                let text = std::fs::read_to_string(file).map_err(|e| {
                    ConfigError::Invalid(format!("database.url_file {}: {e}", file.display()))
                })?;
                let url = text.trim_end();
                if url.is_empty() {
                    return Err(ConfigError::Invalid(format!(
                        "database.url_file {} is empty",
                        file.display()
                    )));
                }
                Ok(url.to_owned())
            }
            (Some(_), Some(file)) => Err(ConfigError::Invalid(format!(
                "the database is given twice: as an argument and by database.url_file {}",
                file.display()
            ))),
            (None, None) => Err(ConfigError::Invalid(
                "no database: give its URL as an argument or set database.url_file".into(),
            )),
        }
    }
}

/// The web shell and its plugins (spec 007, design « Configuration »).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Web {
    /// The build of the shell; without it, only the plugins are served (development
    /// behind Vite).
    pub root: Option<PathBuf>,
    /// The plugins, one directory `<name>/<version>/` each; without it, `ui_plugins` is
    /// empty.
    pub plugins_dir: Option<PathBuf>,
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

impl Config {
    /// Longest write transaction (`[http] write_transaction_timeout_s`, 60 s without an
    /// `[http]` section): the `transaction_timeout` of the pool of an instance, on which
    /// the frozen past of the reads rests (spec 006, « Lectures datées et cache »).
    #[must_use]
    pub fn write_transaction_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(
            self.http
                .as_ref()
                .map_or_else(write_transaction_timeout_s, |h| {
                    h.write_transaction_timeout_s
                }),
        )
    }
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
    /// The public client of the native applications (the CLI) at this issuer, published
    /// by `GET /v1/auth/native-clients` (spec 023, T10/A5); absent: not published.
    #[serde(default)]
    pub native_client_id: Option<String>,
    /// The scopes it asks for (`openid` by default).
    #[serde(default = "native_scopes")]
    pub native_scopes: Vec<String>,
    /// Whether the issuer offers it the device flow (RFC 8628).
    #[serde(default)]
    pub native_device: bool,
}

fn native_scopes() -> Vec<String> {
    vec!["openid".into()]
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
        check_native(issuer)?;
    }
    Ok(())
}

/// The public client of the native applications at `issuer` (spec 023, T10/A5): another
/// client than the confidential one of the BFF, scopes of RFC 6749 §3.3 among which
/// `openid`; `native_scopes` and `native_device` only beside `native_client_id`.
fn check_native(issuer: &Issuer) -> Result<(), String> {
    let id = &issuer.id;
    let Some(client) = &issuer.native_client_id else {
        if issuer.native_scopes != native_scopes() || issuer.native_device {
            return Err(format!(
                "auth.issuers.{id}: native_scopes and native_device need native_client_id"
            ));
        }
        return Ok(());
    };
    if client.is_empty() || *client == issuer.client_id {
        return Err(format!(
            "auth.issuers.{id}.native_client_id: a public client, other than client_id"
        ));
    }
    // scope-token = 1*( %x21 / %x23-5B / %x5D-7E ) (RFC 6749 §3.3).
    let token = |s: &String| {
        !s.is_empty()
            && s.bytes()
                .all(|b| matches!(b, 0x21 | 0x23..=0x5B | 0x5D..=0x7E))
    };
    if !issuer.native_scopes.iter().all(token)
        || !issuer.native_scopes.iter().any(|s| s == "openid")
    {
        return Err(format!(
            "auth.issuers.{id}.native_scopes: scope tokens (RFC 6749 §3.3), openid among them"
        ));
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
    if kartenn_core::labels::Locale::parse(&config.i18n.default_locale).is_none() {
        return invalid(format!(
            "i18n.default_locale: {:?} is not a language tag (BCP 47)",
            config.i18n.default_locale
        ));
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
    check_events(&config.events).map_err(ConfigError::Invalid)?;
    if let Some(root) = config.web.as_ref().and_then(|web| web.root.as_ref()) {
        kartenn_rest::web::Shell::open(root).map_err(|e| ConfigError::Invalid(e.to_string()))?;
    }
    if let Some(dir) = config.web.as_ref().and_then(|web| web.plugins_dir.as_ref())
        && let Err(e) = std::fs::read_dir(dir)
    {
        return invalid(format!("web.plugins_dir: {}: {e}", dir.display()));
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

    /// The operator reference (`docs/exploitation/configuration.md`, spec 015 task 8) names
    /// every field of this file, and its complete example is a valid configuration (parsed
    /// alone: the files it names do not exist here).
    #[test]
    fn the_operator_reference_covers_every_field() {
        let doc = include_str!("../../../docs/exploitation/configuration.md");
        let missing: Vec<&str> = include_str!("config.rs")
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub "))
            .filter(|rest| {
                !["fn ", "struct ", "enum ", "const ", "use ", "mod ", "type "]
                    .iter()
                    .any(|item| rest.starts_with(item))
            })
            .filter_map(|rest| rest.split_once(':').map(|(name, _)| name.trim()))
            // A field as `name`, a section as `[name]`.
            .filter(|name| {
                !doc.contains(&format!("`{name}`")) && !doc.contains(&format!("`[{name}]`"))
            })
            .collect();
        assert!(
            missing.is_empty(),
            "fields absent from the reference: {missing:?}"
        );
        let example = doc
            .split("```toml\n")
            .nth(1)
            .and_then(|rest| rest.split("```").next())
            .expect("the reference has a TOML example");
        let config: Config = Figment::from(Toml::string(example)).extract().unwrap();
        assert!(config.http.is_some() && config.auth.is_some() && config.web.is_some());
        assert_eq!(config.telemetry.outputs.len(), 3);
    }

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
            (
                format!("{AUTH}native_client_id = \"kartenn\""),
                "other than client_id",
            ),
            (
                format!("{AUTH}native_device = true"),
                "need native_client_id",
            ),
            (
                format!("{AUTH}native_client_id = \"cli\"\nnative_scopes = [\"profile\"]"),
                "openid among them",
            ),
            (
                format!("{AUTH}native_client_id = \"cli\"\nnative_scopes = [\"openid\", \"a b\"]"),
                "scope tokens",
            ),
        ];
        for (text, reason) in cases {
            let error = from_toml(&text).unwrap_err().to_string();
            assert!(error.contains(reason), "{reason}: {error}");
        }
    }

    /// The public client of the native applications (spec 023, task 023-6): absent by
    /// default; `openid` alone and no device flow unless said.
    #[test]
    fn an_issuer_may_publish_a_native_client() {
        let auth = from_toml(AUTH).unwrap().auth.unwrap();
        assert_eq!(auth.issuers[0].native_client_id, None);
        let text = format!("{AUTH}native_client_id = \"kartenn-cli\"");
        let issuer = &from_toml(&text).unwrap().auth.unwrap().issuers[0];
        assert_eq!(issuer.native_client_id.as_deref(), Some("kartenn-cli"));
        assert_eq!(
            (issuer.native_scopes.as_slice(), issuer.native_device),
            (&["openid".to_owned()][..], false)
        );
        let text = format!(
            "{AUTH}native_client_id = \"kartenn-cli\"\n\
             native_scopes = [\"openid\", \"offline_access\"]\nnative_device = true"
        );
        let issuer = &from_toml(&text).unwrap().auth.unwrap().issuers[0];
        assert_eq!(issuer.native_scopes, ["openid", "offline_access"]);
        assert!(issuer.native_device);
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

    /// REQ-018-12: `[i18n] default_locale`, `fr` by default, a BCP 47 tag.
    #[test]
    fn the_default_locale_is_configured() {
        assert_eq!(from_toml("").unwrap().i18n.locale().as_str(), "fr");
        let config = from_toml("[i18n]\ndefault_locale = \"en-GB\"").unwrap();
        assert_eq!(config.i18n.locale().as_str(), "en-gb");
        config.i18n.install().unwrap();
        assert_eq!(kartenn_core::labels::default_locale().as_str(), "en-gb");
        assert!(config.i18n.install().is_ok());
        let error = from_toml("[i18n]\ndefault_locale = \"en_GB\"")
            .unwrap_err()
            .to_string();
        assert!(error.contains("i18n.default_locale"), "{error}");
        assert!(from_toml("[i18n]\nlocale = \"en\"").is_err());
    }

    #[test]
    fn a_faulty_web_section_stops_the_start_with_its_reason() {
        let root = std::env::temp_dir().join(format!("kartenn-web-config-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let section = format!("[web]\nroot = \"{}\"", root.display());
        let error = from_toml(&section).unwrap_err().to_string();
        assert!(error.contains("index.html"), "{error}");
        std::fs::write(root.join("index.html"), "<html><script>x</script></html>").unwrap();
        let error = from_toml(&section).unwrap_err().to_string();
        assert!(error.contains("importmap"), "{error}");
        std::fs::write(
            root.join("index.html"),
            "<script type=\"importmap\">{\"imports\":{}}</script>",
        )
        .unwrap();
        let config = from_toml(&section).unwrap();
        assert_eq!(config.web.unwrap().root.as_deref(), Some(root.as_path()));
        let error = from_toml(&section.replace("root", "rooot"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("rooot"), "{error}");
    }

    #[test]
    fn the_database_comes_from_the_argument_or_its_file_once() {
        const URL: &str = "postgres://kartenn:s3cret@db.invalid/kartenn";
        let dir = std::env::temp_dir().join(format!("kartenn-db-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("url");
        let section = format!("[database]\nurl_file = \"{}\"", file.display());
        // Unknown field, as everywhere.
        let error = from_toml("[database]\nurl = \"x\"")
            .unwrap_err()
            .to_string();
        assert!(error.contains("url"), "{error}");
        // The argument alone.
        assert_eq!(from_toml("").unwrap().database_url(Some(URL)).unwrap(), URL);
        // The file alone, its final newline dropped.
        std::fs::write(&file, format!("{URL}\n")).unwrap();
        let config = from_toml(&section).unwrap();
        assert_eq!(config.database_url(None).unwrap(), URL);
        // Neither, both, an empty file, an unreadable one: refused, the URL never shown.
        let refusals = [
            from_toml("").unwrap().database_url(None).unwrap_err(),
            config.database_url(Some(URL)).unwrap_err(),
            {
                std::fs::write(&file, " \n").unwrap();
                config.database_url(None).unwrap_err()
            },
            {
                std::fs::write(&file, URL).unwrap();
                from_toml(&section.replace("url\"", "absent\""))
                    .unwrap()
                    .database_url(None)
                    .unwrap_err()
            },
        ];
        for error in refusals {
            let error = error.to_string();
            assert!(!error.contains("s3cret"), "{error}");
            assert!(error.contains("database"), "{error}");
        }
    }
}
