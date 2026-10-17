//! Composition of a running instance (spec 012, REQ-012-2, 13, 15, 20, 24, 26, 31, 34):
//! the outputs of the technical logs and OpenTelemetry from the configuration, the audit
//! outputs, the periodic sealing and relays, and a clean stop that drains everything.
//! The HTTP API (006) will be served by the same instance.

use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, anyhow};
use kartenn_core::audit::seal::SealKey;
use kartenn_core::audit::{Outcome, SYS_STARTED, SYS_STOPPED};
use kartenn_service::audit::{AuditJournal, Instance, RequestContext, SinkHealth, now};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::audit_relay::{RelayError, export_lag, relay_once};
use kartenn_store_pg::audit_seal::seal_once;
use kartenn_telemetry::capture::{self, CaptureLayer, drive_until};
use kartenn_telemetry::otel::{Identity, Instruments, Providers};
use kartenn_telemetry::outputs::{self, Format};
use kartenn_telemetry::sinks::{AnySink, FileSink, OtlpSink, SyslogSink};
use sqlx::PgPool;
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
/// Longest stop (REQ-012-34).
const STOP_WITHIN: Duration = Duration::from_secs(10);

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
    let subscriber = tracing_subscriber::registry()
        .with(CaptureLayer::new(outputs.clone()))
        .with(trace);
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        eprintln!("kartenn: a global subscriber is already installed; keeping it");
    }
    Ok(Telemetry {
        outputs,
        tasks,
        providers: otlp,
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
async fn relay_all(
    pool: &PgPool,
    journal: &PgAuditJournal,
    sinks: &mut [(AnySink, SinkHealth)],
    instruments: Option<&Instruments>,
) {
    for (sink, health) in sinks.iter_mut() {
        let result = relay_once(pool, sink, RELAY_BATCH).await;
        if let Err(RelayError::Sink { sink, error }) = &result {
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
}

/// Runs an instance until `shutdown`: technical outputs, audit sealing and relays; at
/// the end, a last sealing and relay, `sys.instance.stopped`, and the outputs drained.
///
/// # Errors
///
/// A configuration that cannot be put to work (an output that cannot start, an unreadable
/// key), or a database that refuses the start event.
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
    let counters = telemetry.outputs.clone();
    let instruments = telemetry.providers.as_ref().map(|p| {
        Instruments::new(&p.meter, move || {
            counters
                .iter()
                .map(|o| (o.name().to_owned(), o.dropped()))
                .collect()
        })
    });
    let key = match &config.audit.signing_key_file {
        Some(path) => Some(SealKey::from_pkcs8_pem(
            &std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?,
        )?),
        None => None,
    };
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

    let store = kartenn_store_pg::write::PgStore::new(pool.clone()).with_instance(instance.clone());
    let mut filters = capture::FilterOverrides::new(
        config.telemetry.names().into_iter().zip(
            config
                .telemetry
                .outputs
                .iter()
                .map(|o| o.filter().to_owned()),
        ),
    );
    let mut reread =
        tokio::time::interval(Duration::from_millis(config.telemetry.override_poll_ms));
    let mut clock = tokio::time::interval(CLOCK_EVERY);
    let mut seal = tokio::time::interval(Duration::from_millis(config.audit.seal_interval_ms));
    let mut relay = tokio::time::interval(Duration::from_millis(config.audit.relay_interval_ms));
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            _ = seal.tick() => {
                if let Err(e) = seal_once(&pool, key.as_ref(), &instance).await {
                    tracing::error!(name: "audit.seal.failed", error = %e.0, "sealing failed");
                }
            }
            _ = relay.tick() => relay_all(&pool, &journal, &mut sinks, instruments.as_ref()).await,
            _ = clock.tick() => check_clock(&pool, instruments.as_ref()).await,
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

    let lost_so_far = dropped(&telemetry);
    let finish = async {
        let stopped = RequestContext::system()
            .event(&SYS_STOPPED, Outcome::Success)
            .and_then(|e| e.detail("lost_technical_events", lost_so_far));
        if let Ok(event) = stopped {
            let _ = journal.record(&event).await;
        }
        let _ = seal_once(&pool, key.as_ref(), &instance).await;
        relay_all(&pool, &journal, &mut sinks, instruments.as_ref()).await;
    };
    Ok(stop(telemetry, finish).await)
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
