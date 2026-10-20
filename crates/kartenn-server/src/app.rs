//! Composition of a running instance (spec 012, REQ-012-2, 13, 15, 20, 24, 26, 31, 34):
//! the outputs of the technical logs and OpenTelemetry from the configuration, the audit
//! outputs, the periodic sealing and relays, and a clean stop that drains everything.
//! With `[http]`, the same instance serves the API (spec 006, REQ-006-32): drained first at
//! the stop, its certificate read again on `SIGHUP`.

use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, anyhow};
use kartenn_core::audit::seal::SealKey;
use kartenn_core::audit::{Outcome, SYS_OUTPUT_DISABLED, SYS_STARTED, SYS_STOPPED};
use kartenn_service::audit::{
    AuditJournal, AuditStatus, Instance, RequestContext, SinkHealth, now,
};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::audit_relay::{RelayError, export_lag, relay_once};
use kartenn_store_pg::audit_seal::seal_once;
use kartenn_telemetry::capture::{self, CaptureLayer, drive_until};
use kartenn_telemetry::otel::{Identity, Instruments, Providers};
use kartenn_telemetry::outputs::{self, Format};
use kartenn_telemetry::sinks::{AnySink, FileSink, OtlpSink, SyslogSink};
use sqlx::PgPool;
use sqlx::postgres::PgConnectOptions;
use sqlx::types::Uuid;
use syslog_sender::rustls::pki_types::pem::PemObject;
use syslog_sender::rustls::pki_types::{CertificateDer, ServerName};
use syslog_sender::rustls::{ClientConfig, RootCertStore, crypto};
use syslog_sender::{Facility, Target};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing_subscriber::layer::SubscriberExt;

use crate::config::{Config, Output, Sink, StreamFormat, Transport};

/// Most events per relay pass.
const RELAY_BATCH: i64 = 500;
/// Longest drain of one tick, by output for the relays: full batches follow one another
/// within it, so that the export and the sealing keep up with any rate (one batch per
/// tick capped them at 500 and 666 events per second).
const DRAIN_WITHIN: Duration = Duration::from_secs(1);
/// Longest stop (REQ-012-34).
const STOP_WITHIN: Duration = Duration::from_secs(10);

/// At most one WARN per output in this period, when it drops records or refuses a batch
/// (REQ-012-26).
const WARN_EVERY: Duration = Duration::from_mins(1);

/// The discoveries and keys of the issuers read again in this period (REQ-006-38).
const ISSUERS_EVERY: Duration = Duration::from_mins(5);

/// Allows one WARN per key and [`WARN_EVERY`].
#[derive(Default)]
struct Throttle(std::collections::HashMap<String, std::time::Instant>);

impl Throttle {
    fn allow(&mut self, key: &str) -> bool {
        let now = std::time::Instant::now();
        match self.0.get(key) {
            Some(last) if now.duration_since(*last) < WARN_EVERY => false,
            _ => {
                self.0.insert(key.to_owned(), now);
                true
            }
        }
    }
}

/// Between two comparisons of the clocks (REQ-012-32).
const CLOCK_EVERY: Duration = Duration::from_mins(5);

/// Compares the clock of the instance to PostgreSQL's: metric, and WARN beyond one second.
async fn check_clock(pool: &PgPool, instruments: Option<&Instruments>) {
    let Ok(skew) = kartenn_store_pg::clock::clock_skew(pool, std::time::SystemTime::now).await
    else {
        return;
    };
    if let Some(instruments) = instruments {
        instruments
            .gauges
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clock_skew = skew;
    }
    if skew.abs() > kartenn_store_pg::clock::SKEW_TOLERANCE.as_secs_f64() {
        tracing::warn!(name: "sys.clock.skew", skew_seconds = skew, "the clock of the instance drifts from PostgreSQL's");
    }
}

/// What a run left behind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Technical records lost: dropped by a full output or by a failed write.
    pub lost_technical: u64,
}

fn host_name() -> Option<String> {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            std::fs::read_to_string("/proc/sys/kernel/hostname")
                .ok()
                .map(|h| h.trim().to_owned())
        })
        .filter(|h| !h.is_empty())
}

/// The syslog target of a transport, TLS with the roots of `ca_file`.
fn target(
    transport: Transport,
    address: &str,
    server_name: Option<&str>,
    ca_file: Option<&Path>,
) -> anyhow::Result<Target> {
    Ok(match transport {
        Transport::Local => Target::UnixDatagram(address.into()),
        Transport::Udp => Target::Udp(address.to_owned()),
        Transport::Tcp => Target::Tcp(address.to_owned()),
        Transport::Tls => {
            let ca_file = ca_file.ok_or_else(|| anyhow!("TLS needs ca_file"))?;
            let pem =
                std::fs::read(ca_file).with_context(|| format!("reading {}", ca_file.display()))?;
            let mut roots = RootCertStore::empty();
            for cert in CertificateDer::pem_slice_iter(&pem) {
                roots.add(cert.context("a certificate of ca_file")?)?;
            }
            let config =
                ClientConfig::builder_with_provider(Arc::new(crypto::ring::default_provider()))
                    .with_safe_default_protocol_versions()?
                    .with_root_certificates(roots)
                    .with_no_client_auth();
            let host = server_name
                .map(str::to_owned)
                .or_else(|| address.rsplit_once(':').map(|(h, _)| h.to_owned()))
                .ok_or_else(|| anyhow!("no server name for {address}"))?;
            Target::Tls {
                address: address.to_owned(),
                server_name: ServerName::try_from(host)?,
                config: Arc::new(config),
            }
        }
    })
}

/// The running technical outputs.
struct Telemetry {
    outputs: Vec<capture::Output>,
    tasks: Vec<JoinHandle<u64>>,
    providers: Option<Providers>,
    instruments: Option<Instruments>,
    stop: watch::Sender<bool>,
}

async fn stopped(mut stop: watch::Receiver<bool>) {
    let _ = stop.wait_for(|s| *s).await;
}

/// Starts the outputs and installs the global subscriber (once per process; a second call,
/// in tests, keeps the first one).
fn telemetry(config: &Config, identity: &Identity) -> anyhow::Result<Telemetry> {
    let (stop, stop_rx) = watch::channel(false);
    let mut outputs = Vec::new();
    let mut tasks = Vec::new();
    let mut otlp = None;
    for (output, name) in config
        .telemetry
        .outputs
        .iter()
        .zip(config.telemetry.names())
    {
        if let Output::Otlp { endpoint, .. } = output
            && otlp.is_none()
        {
            otlp = Some(Providers::otlp(
                identity,
                config.telemetry.sampling_ratio,
                endpoint.as_deref(),
            )?);
        }
        let (handle, receiver) =
            capture::Output::new(name, output.filter(), config.telemetry.queue)?;
        let until = stopped(stop_rx.clone());
        let task = match output {
            Output::Stdout { format, .. } => {
                let format = match format {
                    StreamFormat::Text => Format::Text,
                    StreamFormat::Json => Format::Json,
                };
                tokio::spawn(drive_until(receiver, outputs::Stdout::new(format), until))
            }
            Output::File { path, .. } => {
                tokio::spawn(drive_until(receiver, outputs::File::new(path), until))
            }
            Output::Journald { socket, .. } => {
                let socket = socket
                    .clone()
                    .unwrap_or_else(|| outputs::JOURNALD_SOCKET.into());
                tokio::spawn(drive_until(
                    receiver,
                    outputs::Journald::new(socket)?,
                    until,
                ))
            }
            Output::Syslog {
                transport,
                address,
                server_name,
                ca_file,
                ..
            } => {
                let target = target(
                    *transport,
                    address,
                    server_name.as_deref(),
                    ca_file.as_deref(),
                )?;
                let writer = outputs::Syslog::new(target, Facility::Daemon, host_name());
                tokio::spawn(drive_until(receiver, writer, until))
            }
            Output::Otlp { .. } => {
                let writer = otlp.as_ref().expect("created above").log_writer();
                tokio::spawn(drive_until(receiver, writer, until))
            }
        };
        outputs.push(handle);
        tasks.push(task);
    }
    let trace = otlp.as_ref().map(Providers::trace_layer);
    let instruments = otlp.as_ref().map(|p| {
        let counters = outputs.clone();
        Instruments::new(&p.meter, move || {
            counters
                .iter()
                .map(|o| (o.name().to_owned(), o.dropped()))
                .collect()
        })
    });
    let subscriber = tracing_subscriber::registry()
        .with(CaptureLayer::new(outputs.clone()))
        .with(trace)
        .with(instruments.as_ref().map(Instruments::layer));
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        eprintln!("kartenn: a global subscriber is already installed; keeping it");
    }
    Ok(Telemetry {
        outputs,
        tasks,
        providers: otlp,
        instruments,
        stop,
    })
}

fn sinks(config: &Config) -> anyhow::Result<Vec<(AnySink, SinkHealth)>> {
    config
        .audit
        .sinks
        .iter()
        .map(|sink| {
            let sink = match sink {
                Sink::File { name, path } => AnySink::File(FileSink::new(name, path)),
                Sink::Syslog {
                    name,
                    transport,
                    address,
                    server_name,
                    ca_file,
                } => AnySink::Syslog(SyslogSink::new(
                    name,
                    target(
                        *transport,
                        address,
                        server_name.as_deref(),
                        ca_file.as_deref(),
                    )?,
                    host_name(),
                )),
                Sink::Otlp { name, endpoint } => AnySink::Otlp(OtlpSink::new(name, endpoint)),
            };
            Ok((sink, SinkHealth::default()))
        })
        .collect()
}

/// One relay pass over every audit output: health events and lag gauges updated.
/// Records `sys.output.disabled` for each audit output that received events before and is
/// no longer configured (REQ-012-26): removing an export is an audited change.
async fn disabled_sinks(
    pool: &PgPool,
    journal: &PgAuditJournal,
    config: &Config,
) -> anyhow::Result<()> {
    let known: Vec<String> = sqlx::query_scalar(
        "SELECT substr(pass, length('audit.relay/') + 1) FROM pass_floor
         WHERE pass LIKE 'audit.relay/%' ORDER BY pass",
    )
    .fetch_all(pool)
    .await?;
    for name in known
        .iter()
        .filter(|n| !config.audit.sinks.iter().any(|s| s.name() == n.as_str()))
    {
        let event = RequestContext::system()
            .event(&SYS_OUTPUT_DISABLED, Outcome::Success)?
            .detail("output", name.as_str())?;
        journal
            .record(&event)
            .await
            .map_err(|e| anyhow!("recording a disabled output: {}", e.0))?;
        tracing::warn!(name: "sys.output.disabled", output = %name, "an audit output is no longer configured");
    }
    Ok(())
}

/// A WARN for each technical output that dropped records since the last call (at most one
/// per [`WARN_EVERY`], REQ-012-26).
fn warn_drops(outputs: &[capture::Output], seen: &mut std::collections::HashMap<String, u64>) {
    for output in outputs {
        let now = output.dropped();
        let before = seen.insert(output.name().to_owned(), now).unwrap_or(0);
        if now > before {
            tracing::warn!(
                name: "sys.output.dropped",
                output = output.name(),
                dropped = now - before,
                "a technical output dropped records: its queue was full"
            );
        }
    }
}

/// Seals full batches one after another, within [`DRAIN_WITHIN`].
async fn seal_all(pool: &PgPool, key: Option<&SealKey>, instance: &Instance) {
    let started = std::time::Instant::now();
    loop {
        match seal_once(pool, key, instance).await {
            Ok(Some(sealed))
                if sealed.count
                    == usize::try_from(kartenn_store_pg::audit_seal::BATCH_MAX)
                        .unwrap_or(usize::MAX)
                    && started.elapsed() < DRAIN_WITHIN => {}
            Ok(_) => break,
            Err(e) => {
                tracing::error!(name: "audit.seal.failed", error = %e.0, "sealing failed");
                break;
            }
        }
    }
}

async fn relay_all(
    pool: &PgPool,
    journal: &PgAuditJournal,
    sinks: &mut [(AnySink, SinkHealth)],
    instruments: Option<&Instruments>,
    throttle: &mut Throttle,
    status: &AuditStatus,
) {
    for (sink, health) in sinks.iter_mut() {
        let started = std::time::Instant::now();
        let mut result = relay_once(pool, sink, RELAY_BATCH).await;
        while matches!(result, Ok(n) if n == usize::try_from(RELAY_BATCH).unwrap_or(usize::MAX))
            && started.elapsed() < DRAIN_WITHIN
        {
            result = relay_once(pool, sink, RELAY_BATCH).await;
        }
        if let Err(RelayError::Sink { sink, error }) = &result
            && throttle.allow(sink)
        {
            tracing::warn!(name: "audit.relay.failed", output = %sink, error = %error, "an audit output refused a batch");
        }
        let ok = !matches!(result, Err(RelayError::Sink { .. }));
        if let Some(kind) = health.observe(ok, now()) {
            let event = RequestContext::system()
                .event(kind, Outcome::Success)
                .and_then(|e| e.detail("output", kartenn_service::audit::AuditSink::name(sink)));
            if let Ok(event) = event
                && let Err(e) = journal.record(&event).await
            {
                tracing::error!(name: "audit.write.failed", error = %e.0, "an audit event could not be recorded");
            }
        }
        if let Some(instruments) = instruments
            && let Ok(lag) = export_lag(pool, kartenn_service::audit::AuditSink::name(sink)).await
        {
            let mut gauges = instruments
                .gauges
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            gauges.export_lag.insert(
                kartenn_service::audit::AuditSink::name(sink).to_owned(),
                lag.map_or(0.0, |d| d.as_secs_f64()),
            );
        }
    }
    status.update(sinks.iter().map(|(_, health)| health));
}

/// The filters of the technical outputs, as configured, before any change at run time.
fn filters(config: &Config) -> capture::FilterOverrides {
    capture::FilterOverrides::new(
        config.telemetry.names().into_iter().zip(
            config
                .telemetry
                .outputs
                .iter()
                .map(|o| o.filter().to_owned()),
        ),
    )
}

/// The key signing the seals, when configured.
fn seal_key(config: &Config) -> anyhow::Result<Option<SealKey>> {
    let Some(path) = &config.audit.signing_key_file else {
        return Ok(None);
    };
    Ok(Some(SealKey::from_pkcs8_pem(
        &std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
    )?))
}

/// The connection options of the pool of an instance: every transaction ends within
/// `bound` (`transaction_timeout`), so no write can appear at a system time older than
/// `bound`, the frozen past of the reads (spec 006, « Lectures datées et cache »). The
/// one-off commands (`migrate`, `audit purge`) are not bounded.
#[must_use]
pub fn bounded(options: PgConnectOptions, bound: Duration) -> PgConnectOptions {
    options.options([("transaction_timeout", format!("{}ms", bound.as_millis()))])
}

/// Runs an instance until `shutdown`: technical outputs, audit sealing and relays; at
/// the end, a last sealing and relay, `sys.instance.stopped`, and the outputs drained.
///
/// # Errors
///
/// A configuration that cannot be put to work (an output that cannot start, an unreadable
/// key), or a database that refuses the start event.
#[allow(clippy::too_many_lines)] // the start, the main loop and the stop, in order
pub async fn run(
    pool: PgPool,
    config: &Config,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<Report> {
    let id = Uuid::now_v7();
    let instance = Instance {
        version: env!("CARGO_PKG_VERSION"),
        id,
    };
    let identity = Identity {
        version: instance.version,
        instance: id.to_string(),
    };
    let telemetry = telemetry(config, &identity)?;
    kartenn_telemetry::panic::install();
    // The modules of the build, before anything is recorded: a conflict stops the start.
    let modules = crate::modules::registry()?;
    let instruments = telemetry.instruments.clone();
    let key = seal_key(config)?;
    // The files of `[auth]`, before anything is recorded: a missing one stops the start.
    let auth = config.auth.as_ref().map(crate::auth::load).transpose()?;
    let mut sinks = sinks(config)?;
    let journal = PgAuditJournal::new(pool.clone(), instance.clone());
    let started = RequestContext::system()
        .event(&SYS_STARTED, Outcome::Success)?
        .detail("version", instance.version)?;
    journal
        .record(&started)
        .await
        .map_err(|e| anyhow!("recording the start: {}", e.0))?;
    tracing::info!(name: "sys.instance.started", version = instance.version, instance = %id, "kartenn started");
    if let Some(auth) = &auth {
        tracing::info!(issuers = auth.issuers.len(), "authentication configured");
    } else {
        tracing::warn!("no [auth] section: no one can log in");
    }
    disabled_sinks(&pool, &journal, config).await?;
    let audit_status = AuditStatus::default();
    let mut api = match &config.http {
        Some(http) => Some(
            Api::start(
                http,
                &pool,
                &audit_status,
                auth.as_ref(),
                &journal,
                config.audit.read_audit(),
                config.telemetry.names().into(),
                config.web.as_ref(),
                modules.clone(),
            )
            .await?,
        ),
        None => None,
    };

    let store = kartenn_store_pg::write::PgStore::new(pool.clone())
        .with_instance(instance.clone())
        .with_modules(modules.clone());
    let mut filters = filters(config);
    let mut reread =
        tokio::time::interval(Duration::from_millis(config.telemetry.override_poll_ms));
    let mut clock = tokio::time::interval(CLOCK_EVERY);
    let mut quotas = tokio::time::interval(Duration::from_mins(1));
    let mut seal = tokio::time::interval(Duration::from_millis(config.audit.seal_interval_ms));
    let mut relay = tokio::time::interval(Duration::from_millis(config.audit.relay_interval_ms));
    // The background tasks (REQ-006-38); the public flags checked again at each start, so
    // at the start of a version whose registries changed.
    if let Err(e) = kartenn_store_pg::jobs::due_now(&pool, "public-drift").await {
        tracing::warn!(name: "job.claim_failed", error = %e.0, "the check of the public flags could not be scheduled");
    }
    let (jobs_stop, jobs_stopped) = watch::channel(false);
    let jobs = tokio::spawn(crate::jobs::Scheduler::new(pool.clone(), id).run(
        crate::jobs::Tasks {
            store: store.clone(),
            // No module provides a source of expected values yet.
            sources: kartenn_service::expected::Sources::none(),
        },
        jobs_stopped,
    ));
    let mut issuers =
        tokio::time::interval_at(tokio::time::Instant::now() + ISSUERS_EVERY, ISSUERS_EVERY);
    let mut drops = tokio::time::interval(WARN_EVERY);
    let mut dropped_seen = std::collections::HashMap::new();
    let mut throttle = Throttle::default();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            () = hangup(api.as_mut()) => {
                if let Some(api) = &api {
                    api.reload();
                }
            }
            _ = seal.tick() => seal_all(&pool, key.as_ref(), &instance).await,
            _ = relay.tick() => {
                relay_all(&pool, &journal, &mut sinks, instruments.as_ref(), &mut throttle, &audit_status).await;
            }
            _ = drops.tick() => warn_drops(&telemetry.outputs, &mut dropped_seen),
            _ = clock.tick() => check_clock(&pool, instruments.as_ref()).await,
            _ = quotas.tick() => Api::record_refusals(api.as_ref(), &journal).await,
            _ = issuers.tick() => Api::reload_issuers(api.as_ref()),
            _ = reread.tick() => {
                if let Ok(list) = kartenn_service::observability::Overrides::overrides(&store).await {
                    let pairs: Vec<(String, String)> =
                        list.into_iter().map(|o| (o.output, o.filter)).collect();
                    for output in filters.apply(&telemetry.outputs, &pairs) {
                        tracing::info!(name: "sys.log_filter.applied", output = %output, "filter of an output changed");
                    }
                }
            }
        }
    }

    // The background tasks and the API first: nothing writes while the journal is closed.
    let _ = jobs_stop.send(true);
    let _ = jobs.await;
    if let Some(api) = api {
        api.stop(&journal).await;
    }
    let lost_so_far = dropped(&telemetry);
    let finish = async {
        let stopped = RequestContext::system()
            .event(&SYS_STOPPED, Outcome::Success)
            .and_then(|e| e.detail("lost_technical_events", lost_so_far));
        if let Ok(event) = stopped {
            let _ = journal.record(&event).await;
        }
        let _ = seal_once(&pool, key.as_ref(), &instance).await;
        relay_all(
            &pool,
            &journal,
            &mut sinks,
            instruments.as_ref(),
            &mut throttle,
            &audit_status,
        )
        .await;
    };
    Ok(stop(telemetry, finish).await)
}

/// Purges the idle keys of the quotas and records their refusals aggregated per minute
/// (REQ-006-29), and the failed authentications aggregated (REQ-012-19), of every window
/// when `all` (at shutdown).
async fn record_refusals(
    journal: &PgAuditJournal,
    quotas: &kartenn_rest::quota::Quotas,
    authn: Option<&ServerAuthn>,
    all: bool,
) {
    let at = kartenn_service::audit::now();
    let mut events = quotas.tick(at, all);
    if let Some(authn) = authn {
        events.extend(authn.tick(at, all));
    }
    for event in events {
        if let Err(e) = journal.record(&event).await {
            tracing::error!(name: "audit.record_failed", error = %e, "rate refusals not recorded");
        }
    }
}

/// The authentication of the API.
type ServerAuthn = kartenn_rest::authn::Authn<
    kartenn_store_pg::write::PgStore,
    PgAuditJournal,
    kartenn_auth::tokens::HttpKeys,
>;

/// The authentication of `auth` (spec 005): failures limited to 20 per minute and
/// source (REQ-005-13), aggregated beyond 10 per minute (REQ-012-19), the state of the
/// identities kept 5 s (K10).
fn authn(
    auth: &crate::auth::Loaded,
    pool: &PgPool,
    journal: &PgAuditJournal,
) -> anyhow::Result<ServerAuthn> {
    let client = kartenn_auth::http::HttpClient::new().context("loading the roots of trust")?;
    Ok(kartenn_rest::authn::Authn {
        identities: kartenn_store_pg::write::PgStore::new(pool.clone()),
        journal: journal.clone(),
        ring: auth.ring.clone(),
        oidc: kartenn_auth::oidc::Oidc::new(
            kartenn_auth::tokens::TokenVerifier::new(
                auth.issuers.clone(),
                kartenn_auth::tokens::HttpKeys(client.clone()),
            ),
            client,
            auth.ring.clone(),
            // Required with issuers (configuration); without, no login starts.
            auth.redirect_uri.clone().unwrap_or_default(),
        ),
        states: kartenn_service::authn::StateCache::new(Duration::from_secs(5)),
        limiter: kartenn_auth::secrets::FailureLimiter::new(20, Duration::from_mins(1)),
        aggregator: kartenn_service::aggregate::FailureAggregator::new(10, Duration::from_mins(1)),
        idle: auth.idle,
        absolute: auth.absolute,
    })
}

/// What `[web]` serves (spec 007): the shell, the plugins and the task that reads them
/// again.
type Served = (
    Option<Arc<kartenn_rest::web::Web>>,
    Option<Arc<kartenn_rest::web::Plugins>>,
    Option<JoinHandle<()>>,
);

/// Opens the shell and the plugins of `[web]`, and reads the plugins again every 10 s.
fn serve_web(
    web: Option<&crate::config::Web>,
    modules: &kartenn_service::module::ModuleRegistry,
) -> anyhow::Result<Served> {
    let shell = web
        .and_then(|web| web.root.as_deref())
        .map(kartenn_rest::web::Shell::open)
        .transpose()?;
    let plugins = web
        .and_then(|web| web.plugins_dir.as_deref())
        .map(|dir| {
            kartenn_rest::web::Plugins::open(dir, kartenn_rest::capabilities::module_names(modules))
                .map(Arc::new)
                .with_context(|| format!("web.plugins_dir: {}", dir.display()))
        })
        .transpose()?;
    let watcher = plugins
        .as_ref()
        .map(|plugins| plugins.watch(kartenn_rest::web::plugins::RELOAD_EVERY));
    let web = web.map(|_| Arc::new(kartenn_rest::web::Web::new(shell, plugins.clone())));
    Ok((web, plugins, watcher))
}

/// The API being served.
struct Api {
    stop: tokio::sync::oneshot::Sender<()>,
    server: JoinHandle<()>,
    certificates: Option<Arc<crate::http::Certificates>>,
    /// The quotas, purged by the main loop, which records their refusals.
    quotas: Arc<kartenn_rest::quota::Quotas>,
    /// The authentication, whose aggregated failures the main loop records; none without
    /// `[auth]`.
    authn: Option<Arc<ServerAuthn>>,
    /// The plugins of the shell, read again on `SIGHUP` and by `watcher` (spec 007).
    plugins: Option<Arc<kartenn_rest::web::Plugins>>,
    watcher: Option<JoinHandle<()>>,
    #[cfg(unix)]
    hangup: Option<tokio::signal::unix::Signal>,
}

impl Api {
    /// Binds the address, reads the certificate, and serves in the background.
    #[allow(clippy::too_many_arguments)] // the parts of the instance, built by `run`
    async fn start(
        http: &crate::config::Http,
        pool: &PgPool,
        audit: &AuditStatus,
        auth: Option<&crate::auth::Loaded>,
        journal: &PgAuditJournal,
        read_audit: kartenn_service::reads::ReadAudit,
        outputs: Arc<[String]>,
        web: Option<&crate::config::Web>,
        modules: Arc<kartenn_service::module::ModuleRegistry>,
    ) -> anyhow::Result<Self> {
        let (web, plugins, watcher) = serve_web(web, &modules)?;
        let authn = auth
            .map(|auth| authn(auth, pool, journal).map(Arc::new))
            .transpose()?;
        let authenticator: Arc<dyn kartenn_rest::authn::Authenticator> = match &authn {
            Some(authn) => authn.clone(),
            None => Arc::new(kartenn_rest::authn::Nobody),
        };
        let certificates = http
            .tls
            .as_ref()
            .map(|tls| crate::http::Certificates::load(tls).map(Arc::new))
            .transpose()?;
        let acceptor = certificates
            .as_ref()
            .map(crate::http::Certificates::acceptor)
            .transpose()?;
        #[cfg(unix)]
        let hangup = match (&certificates, &plugins) {
            (None, None) => None,
            _ => Some(
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
                    .context("installing the SIGHUP handler")?,
            ),
        };
        let rate = |r: crate::config::Rate| {
            kartenn_rest::quota::Rate::new(r.per_second, r.burst)
                .context("http.rate: per_second and burst must be positive")
        };
        let quotas = Arc::new(kartenn_rest::quota::Quotas::new(
            kartenn_rest::quota::Rates {
                read: rate(http.rate.read)?,
                write: rate(http.rate.write)?,
                anonymous: rate(http.rate.anonymous)?,
            },
        ));
        let listener = tokio::net::TcpListener::bind(http.listen)
            .await
            .with_context(|| format!("listening on {}", http.listen))?;
        let write_transaction_timeout = Duration::from_secs(http.write_transaction_timeout_s);
        let objects = kartenn_rest::objects::Api {
            pool: pool.clone(),
            // Without `[auth]` no request reaches the objects; a fresh ring will do.
            ring: match auth {
                Some(auth) => auth.ring.clone(),
                None => kartenn_auth::keys::KeyRing::parse(
                    &kartenn_auth::keys::KeyRing::generate_line("k1"),
                )
                .context("drawing a key ring")?,
            },
            bound: write_transaction_timeout,
            journal: journal.clone(),
            audit: read_audit,
            // No module provides a source of expected values yet.
            sources: kartenn_service::expected::Sources::none(),
            outputs,
            modules,
        };
        let app = kartenn_rest::router_with_web(
            kartenn_rest::v1(objects),
            Arc::new(kartenn_rest::health::PgReadiness::new(
                pool.clone(),
                audit.clone(),
            )),
            kartenn_rest::transport::Settings {
                trusted_proxies: http.trusted_proxies.clone(),
                max_body_bytes: http.max_body_bytes,
                request_timeout: Duration::from_millis(http.request_timeout_ms),
                max_in_flight: match http.max_in_flight {
                    0 => 2 * pool.options().get_max_connections() as usize,
                    n => n,
                },
                public_origin: http.public_origin.clone(),
                write_transaction_timeout,
            },
            quotas.clone(),
            authenticator,
            web,
        );
        tracing::info!(name: "http.listening", address = %http.listen, tls = acceptor.is_some(), "serving the API");
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(crate::http::serve(listener, app, acceptor, async {
            let _ = stopped.await;
        }));
        Ok(Self {
            stop,
            server,
            certificates,
            quotas,
            authn,
            plugins,
            watcher,
            #[cfg(unix)]
            hangup,
        })
    }

    /// Reads the certificate again, a faulty one leaving the current one in place; reads
    /// the plugins again in the background.
    fn reload(&self) {
        if let Some(plugins) = self.plugins.clone() {
            tokio::task::spawn_blocking(move || plugins.reload());
        }
        if let Some(certificates) = &self.certificates {
            match certificates.reload() {
                Ok(()) => tracing::info!(name: "http.tls.reloaded", "certificate read again"),
                Err(e) => {
                    tracing::error!(name: "http.tls.reload_failed", error = format!("{e:#}"), "certificate refused, the current one stays");
                }
            }
        }
    }

    /// Records the refusals of the windows over, and purges the states and failures of the
    /// authentication past their time (REQ-006-38), if the API is served.
    async fn record_refusals(api: Option<&Self>, journal: &PgAuditJournal) {
        if let Some(api) = api {
            record_refusals(journal, &api.quotas, api.authn.as_deref(), false).await;
            if let Some(authn) = &api.authn {
                authn.purge();
            }
        }
    }

    /// Reads again the discoveries and keys of the issuers in the background, so that the
    /// main loop does not wait on them (REQ-006-38).
    fn reload_issuers(api: Option<&Self>) {
        if let Some(authn) = api.and_then(|api| api.authn.clone()) {
            tokio::spawn(async move { authn.reload().await });
        }
    }

    /// Stops accepting, then waits for the requests in flight (bounded by
    /// [`crate::http::DRAIN_WITHIN`]), and records the refusals of the quotas still
    /// counted.
    async fn stop(self, journal: &PgAuditJournal) {
        if let Some(watcher) = &self.watcher {
            watcher.abort();
        }
        let _ = self.stop.send(());
        let _ = self.server.await;
        record_refusals(journal, &self.quotas, self.authn.as_deref(), true).await;
    }
}

/// Resolves at each `SIGHUP` when the API terminates TLS; never otherwise.
async fn hangup(api: Option<&mut Api>) {
    #[cfg(unix)]
    if let Some(signal) = api.and_then(|api| api.hangup.as_mut()) {
        signal.recv().await;
        return;
    }
    #[cfg(not(unix))]
    let _ = api;
    std::future::pending::<()>().await;
}

fn dropped(telemetry: &Telemetry) -> u64 {
    telemetry.outputs.iter().map(capture::Output::dropped).sum()
}

/// Stop (REQ-012-34): `finish` (the stop event sealed and exported), then the technical
/// outputs drained and OpenTelemetry flushed, each within [`STOP_WITHIN`].
async fn stop(telemetry: Telemetry, finish: impl Future<Output = ()>) -> Report {
    tracing::info!(name: "sys.instance.stopping", "kartenn stopping");
    let _ = tokio::time::timeout(STOP_WITHIN, finish).await;
    let mut lost = dropped(&telemetry);
    let _ = telemetry.stop.send(true);
    for task in telemetry.tasks {
        if let Ok(Ok(n)) = tokio::time::timeout(STOP_WITHIN, task).await {
            lost += n;
        }
    }
    if let Some(providers) = telemetry.providers {
        // The shutdown blocks until the last export; the exporters run on this runtime,
        // which must stay free meanwhile.
        let _ = tokio::time::timeout(
            STOP_WITHIN,
            tokio::task::spawn_blocking(move || providers.shutdown()),
        )
        .await;
    }
    Report {
        lost_technical: lost,
    }
}
