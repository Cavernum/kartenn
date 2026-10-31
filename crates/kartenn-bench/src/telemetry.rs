//! Cost of the telemetry (spec 012, REQ-012-23, 18; task 012-21): the budgets of the core
//! measured through a running instance (sealing, relays, outputs) in three
//! configurations, each in a process of its own (the `tracing` subscriber is global):
//! - `none`: no technical output (an output filtered to `off`) and no audit output, the
//!   audit still written and sealed;
//! - `reference`: JSON on the standard output, syslog over TCP and OTLP at INFO, traces
//!   sampled at 100 %, the audit exported to syslog;
//! - `blocked`: the reference plus a syslog output whose receiver never reads.
//!
//! The three run in turn for [`ROUNDS`] rounds, the database vacuumed and left to settle
//! before each run; a configuration counts by the median of its p95, and the spread of the
//! `none` runs is the noise of the measure. Reads go through the service, so each is
//! audited (`data.objects.read`). Then the volume of the audit of reads is measured on
//! pages of 100 identifiers.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use kartenn_core::audit::{OBJECTS_READ, Outcome as AuditOutcome};
use kartenn_core::period::Instant as At;
use kartenn_service::audit::{AuditJournal, Instance, RequestContext};
use kartenn_service::authz::Unrestricted;
use kartenn_service::reads::{ReadAudit, read_object};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::filter::{Condition, filter};
use kartenn_store_pg::tree::{children, occupancy};
use kartenn_store_pg::write::PgStore;
use opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::{
    LogsService, LogsServiceServer,
};
use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::collector::metrics::v1::metrics_service_server::{
    MetricsService, MetricsServiceServer,
};
use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::{
    TraceService, TraceServiceServer,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use tokio::io::AsyncReadExt;

use crate::Dataset;
use crate::core_budgets::{new_address, prepare, unitary};
use crate::measures::{Bench, Measure, Selection};
use crate::props::{p95, progress};

const READ: &str = "telemetry.read-object";
const FILTERED_PAGE: &str = "telemetry.filtered-page";
const SUBTREE: &str = "telemetry.subtree";
const UNITARY: &str = "telemetry.unitary-changeset";
const SUSTAINED_RATE: &str = "telemetry.sustained";
const VOLUME: &str = "telemetry.audit-volume";

/// The measures of `bench-telemetry` (ADR 0041): those of the workload run in every child,
/// with the cost of an event and of a span on which the verdict rests; then the volume of
/// the audit of reads.
pub(crate) static BENCH: Bench = Bench {
    name: "telemetry",
    measures: &[
        Measure::new(READ),
        Measure::new(FILTERED_PAGE),
        Measure::new(SUBTREE),
        Measure::new(UNITARY),
        Measure::new(SUSTAINED_RATE),
        Measure::new(VOLUME),
    ],
};

/// Whether a measure of the workload runs: the children have something to measure.
fn workload_runs(selection: &Selection) -> bool {
    [READ, FILTERED_PAGE, SUBTREE, UNITARY, SUSTAINED_RATE]
        .iter()
        .any(|m| selection.runs(m))
}

/// The configurations, in their order of measure within a round.
const CONFIGS: [&str; 3] = ["none", "reference", "blocked"];
/// Rounds of the three configurations.
const ROUNDS: usize = 3;
/// Highest overhead on a p95: with the reference outputs, with a blocked output.
const REFERENCE_MAX: f64 = 0.05;
const BLOCKED_MAX: f64 = 0.10;
/// Writers of the sustained rate, and how long they write.
const WRITERS: usize = 8;
const SUSTAINED: Duration = Duration::from_secs(10);
/// Read requests per second and days of `low` retention of the volume estimate.
const READS_PER_SECOND: f64 = 5.0;
const LOW_DAYS: f64 = 90.0;

/// A collector that acknowledges and forgets.
#[derive(Clone, Copy)]
struct Discard;

#[tonic::async_trait]
impl LogsService for Discard {
    async fn export(
        &self,
        _: tonic::Request<ExportLogsServiceRequest>,
    ) -> Result<tonic::Response<ExportLogsServiceResponse>, tonic::Status> {
        Ok(tonic::Response::new(ExportLogsServiceResponse::default()))
    }
}

#[tonic::async_trait]
impl TraceService for Discard {
    async fn export(
        &self,
        _: tonic::Request<ExportTraceServiceRequest>,
    ) -> Result<tonic::Response<ExportTraceServiceResponse>, tonic::Status> {
        Ok(tonic::Response::new(ExportTraceServiceResponse::default()))
    }
}

#[tonic::async_trait]
impl MetricsService for Discard {
    async fn export(
        &self,
        _: tonic::Request<ExportMetricsServiceRequest>,
    ) -> Result<tonic::Response<ExportMetricsServiceResponse>, tonic::Status> {
        Ok(tonic::Response::new(ExportMetricsServiceResponse::default()))
    }
}

/// A syslog receiver over TCP: reads and forgets, or accepts and never reads.
async fn receiver(reads: bool) -> anyhow::Result<String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?.to_string();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((mut stream, _)) = listener.accept().await {
            if reads {
                tokio::spawn(async move {
                    let mut buf = vec![0; 65_536];
                    while matches!(stream.read(&mut buf).await, Ok(n) if n > 0) {}
                });
            } else {
                held.push(stream);
            }
        }
    });
    Ok(address)
}

/// The configuration of `run`.
fn config(run: &str, syslog: &str, blocked: &str, otlp: &str) -> String {
    let reference = format!(
        r#"{{ kind = "stdout", format = "json", filter = "info" }},
          {{ kind = "syslog", transport = "tcp", address = "{syslog}", filter = "info" }},
          {{ kind = "otlp", endpoint = "http://{otlp}", filter = "info" }},"#
    );
    let (outputs, sinks) = match run {
        // An output is required: one that lets nothing through.
        "none" => (
            r#"{ kind = "stdout", filter = "off" }"#.to_owned(),
            String::new(),
        ),
        "reference" => (
            reference,
            format!(
                r#"{{ kind = "syslog", name = "siem", transport = "tcp", address = "{syslog}" }}"#
            ),
        ),
        _ => (
            format!(
                r#"{reference}
          {{ kind = "syslog", transport = "tcp", address = "{blocked}", filter = "info" }},"#
            ),
            format!(
                r#"{{ kind = "syslog", name = "siem", transport = "tcp", address = "{syslog}" }}"#
            ),
        ),
    };
    format!(
        "[telemetry]\nsampling_ratio = 1.0\noutputs = [{outputs}]\n[audit]\nsinks = [{sinks}]\n"
    )
}

/// Prepares the database, measures each configuration in a child process, compares them
/// and measures the volume of the audit of reads, as far as `selection` runs; returns the
/// report and whether every overhead holds.
///
/// # Errors
///
/// Database failures, or a child that fails.
pub async fn run(
    database: &str,
    data: &Dataset,
    seed: u64,
    small: bool,
    selection: &Selection,
) -> anyhow::Result<(String, bool)> {
    let prepared = prepare(database, data, seed).await?;
    let mut report = prepared.report;
    let mut ok = true;
    if workload_runs(selection) {
        ok = configurations(
            database,
            data,
            seed,
            small,
            selection,
            &prepared.pool,
            &mut report,
        )
        .await?;
    }
    if selection.runs(VOLUME) {
        volume(&prepared.pool, &mut report).await?;
    }
    Ok((report, ok))
}

/// Each configuration measured in a child process, round after round, then compared.
async fn configurations(
    database: &str,
    data: &Dataset,
    seed: u64,
    small: bool,
    selection: &Selection,
    pool: &PgPool,
    report: &mut String,
) -> anyhow::Result<bool> {
    let counts = tokio::task::block_in_place(|| counts(database, data))?;
    let syslog = receiver(true).await?;
    let blocked = receiver(false).await?;
    let probe = std::net::TcpListener::bind("127.0.0.1:0")?;
    let otlp = probe.local_addr()?;
    drop(probe);
    tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(LogsServiceServer::new(Discard))
            .add_service(TraceServiceServer::new(Discard))
            .add_service(MetricsServiceServer::new(Discard))
            .serve(otlp),
    );
    let dir = std::env::temp_dir().join(format!("kartenn-bench-telemetry-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    // Results by configuration, one per round.
    let mut results: Vec<Vec<Vec<(String, u128)>>> = vec![Vec::new(); CONFIGS.len()];
    for round in 0..ROUNDS {
        for (c, name) in CONFIGS.iter().enumerate() {
            let config_path = dir.join(format!("{name}.toml"));
            let result_path = dir.join(format!("{round}-{name}.tsv"));
            std::fs::write(
                &config_path,
                config(name, &syslog, &blocked, &otlp.to_string()),
            )?;
            // The writes of the previous run vacuumed, its background work finished.
            sqlx::raw_sql("VACUUM ANALYZE").execute(pool).await?;
            tokio::time::sleep(Duration::from_secs(2)).await;
            let mut command = std::process::Command::new(std::env::current_exe()?);
            command
                .arg("telemetry-run")
                .arg(database)
                .arg(seed.to_string())
                .arg(&config_path)
                .arg(&result_path);
            if small {
                command.arg("small");
            }
            if let Some(only) = selection.only() {
                command.arg("--only").arg(only);
            }
            // The JSON lines of the standard output are written, then dropped.
            command.stdout(std::process::Stdio::null());
            let status = tokio::task::spawn_blocking(move || command.status()).await??;
            if !status.success() {
                bail!("the {name} run failed: {status}");
            }
            results[c].push(parse(&std::fs::read_to_string(&result_path)?)?);
            progress(&format!("round {round}, {name}: done"));
        }
    }
    std::fs::remove_dir_all(&dir)?;
    // The operations of the measures that ran.
    let counts: Vec<_> = counts
        .into_iter()
        .filter(|(name, _, _)| results[0][0].iter().any(|(n, _)| n == name))
        .collect();
    compare(report, &results, &counts)
}

/// `name\tnanoseconds` lines.
fn parse(text: &str) -> anyhow::Result<Vec<(String, u128)>> {
    text.lines()
        .map(|l| {
            let (name, value) = l.split_once('\t').context("a result line")?;
            Ok((name.to_owned(), value.parse()?))
        })
        .collect()
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        f64::midpoint(values[n / 2 - 1], values[n / 2])
    }
}

/// Compares the configurations on each budget; `results[config][round]`. The verdict
/// rests on the modelled overhead (events and spans of an operation times their measured
/// cost, over the p95 without output); the end-to-end difference is reported with its
/// noise.
#[allow(clippy::cast_precision_loss, clippy::too_many_lines)] // durations of seconds; one report
fn compare(
    report: &mut String,
    results: &[Vec<Vec<(String, u128)>>],
    counts: &[(String, usize, usize)],
) -> anyhow::Result<bool> {
    let values = |config: usize, name: &str| -> Vec<f64> {
        results[config]
            .iter()
            .map(|run| {
                run.iter()
                    .find(|(n, _)| n == name)
                    .map_or(f64::NAN, |(_, v)| *v as f64)
            })
            .collect()
    };
    let mut ok = true;
    writeln!(
        report,
        "p95, median of {ROUNDS} runs          {:>10} {:>10} {:>10} {:>8} {:>8} {:>7}",
        "none", "reference", "blocked", "ref.", "blocked", "noise"
    )?;
    for (name, _) in results[0][0]
        .iter()
        .filter(|(n, _)| n != "lost" && !n.starts_with("cost per"))
    {
        let none = values(0, name);
        let baseline = median(none.clone());
        let reference = median(values(1, name));
        let blocked = median(values(2, name));
        let over_reference = reference / baseline - 1.0;
        let over_blocked = blocked / baseline - 1.0;
        let spread = none.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - none.iter().copied().fold(f64::INFINITY, f64::min);
        let noise = spread / baseline;
        let informative = name.ends_with("(mean)");
        let pass = over_reference <= REFERENCE_MAX && over_blocked <= BLOCKED_MAX;
        let ms = |nanos: f64| format!("{:.2} ms", nanos / 1e6);
        writeln!(
            report,
            "{} {:<33} {:>10} {:>10} {:>10} {:>7.1}% {:>7.1}% {:>6.1}%{}",
            if informative || noise > REFERENCE_MAX {
                "INFO"
            } else if pass {
                "PASS"
            } else {
                "FAIL"
            },
            name,
            ms(baseline),
            ms(reference),
            ms(blocked),
            over_reference * 100.0,
            over_blocked * 100.0,
            noise * 100.0,
            if noise > REFERENCE_MAX {
                " (noise above the limit)"
            } else {
                ""
            }
        )?;
    }
    for (c, name) in CONFIGS.iter().enumerate() {
        writeln!(
            report,
            "INFO {name}: technical records lost per run {:?}",
            values(c, "lost")
        )?;
    }
    // The modelled overhead, on which the verdict rests.
    let cost = |config: usize, what: &str| median(values(config, what));
    writeln!(
        report,
        "INFO cost per event: none {:.0} ns, reference {:.0} ns, blocked {:.0} ns; \
         per span: none {:.0} ns, reference {:.0} ns, blocked {:.0} ns",
        cost(0, "cost per event"),
        cost(1, "cost per event"),
        cost(2, "cost per event"),
        cost(0, "cost per span"),
        cost(1, "cost per span"),
        cost(2, "cost per span"),
    )?;
    writeln!(
        report,
        "modelled overhead on the p95              events  spans     ref.  blocked"
    )?;
    for (name, events, spans) in counts {
        let baseline = median(values(0, name));
        let extra = |config: usize| {
            (*events as f64) * (cost(config, "cost per event") - cost(0, "cost per event"))
                + (*spans as f64) * (cost(config, "cost per span") - cost(0, "cost per span"))
        };
        let over_reference = extra(1) / baseline;
        let over_blocked = extra(2) / baseline;
        let pass = over_reference <= REFERENCE_MAX && over_blocked <= BLOCKED_MAX;
        ok &= pass;
        writeln!(
            report,
            "{} {:<38} {:>6} {:>6} {:>7.2}% {:>7.2}%",
            if pass { "PASS" } else { "FAIL" },
            name,
            events,
            spans,
            over_reference * 100.0,
            over_blocked * 100.0,
        )?;
    }
    Ok(ok)
}

/// Counts the events and the spans.
#[derive(Default)]
struct Count {
    events: AtomicUsize,
    spans: AtomicUsize,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for &'static Count {
    fn on_event(&self, _: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        self.events.fetch_add(1, Ordering::Relaxed);
    }

    fn on_new_span(
        &self,
        _: &tracing::span::Attributes<'_>,
        _: &tracing::span::Id,
        _: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.spans.fetch_add(1, Ordering::Relaxed);
    }
}

/// The events and spans one operation of each budget emits, counted on a thread of its
/// own (the subscriber is local to it); by budget name.
fn counts(database: &str, data: &Dataset) -> anyhow::Result<Vec<(String, usize, usize)>> {
    use tracing_subscriber::layer::SubscriberExt as _;
    let count: &'static Count = Box::leak(Box::default());
    let _guard = tracing::subscriber::set_default(tracing_subscriber::registry().with(count));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let pool = PgPool::connect(database).await?;
        let author: Uuid = sqlx::query_scalar("SELECT id FROM person WHERE display_name = 'bench'")
            .fetch_one(&pool)
            .await?;
        let rack: Uuid =
            sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'custom.rack'")
                .fetch_one(&pool)
                .await?;
        let store = PgStore::new(pool.clone());
        let journal = PgAuditJournal::new(pool.clone(), Instance::new("bench"));
        let context = RequestContext::person(author);
        let now = OffsetDateTime::now_utc();
        let at = At(i64::try_from(now.unix_timestamp_nanos() / 1_000)?);
        let past = At(1_696_118_400_000_000 + 100 * 86_400_000_000);
        let object = data.addresses[0].id;
        let prefix = &data.prefixes[0];
        let network = data
            .prefixes
            .iter()
            .find(|p| p.nature == crate::Nature::Network)
            .map(|p| (p.id, data.domains[p.domain].id))
            .context("a network")?;
        let mut out = Vec::new();
        let mut take = |name: &str| {
            out.push((
                name.to_owned(),
                count.events.swap(0, Ordering::Relaxed),
                count.spans.swap(0, Ordering::Relaxed),
            ));
        };
        take("");
        read_object(
            &store,
            &Unrestricted,
            &journal,
            &context,
            object,
            at,
            None,
            ReadAudit::default(),
        )
        .await?;
        take("read an object (audited)");
        read_object(
            &store,
            &Unrestricted,
            &journal,
            &context,
            object,
            at,
            Some(past),
            ReadAudit::default(),
        )
        .await?;
        take("read an object as known (audited)");
        let condition = Condition::Between(Some("0".into()), Some("100000".into()));
        filter(&pool, rack, data.domains[0].id, &condition, now, None, 100).await?;
        take("filtered page of 100");
        children(&pool, prefix.id, now).await?;
        occupancy(&pool, prefix.id, now).await?;
        take("subtree and occupancy");
        let operation = new_address(&pool, network).await?;
        let address = (
            count.events.swap(0, Ordering::Relaxed),
            count.spans.swap(0, Ordering::Relaxed),
        );
        unitary(&store, author, operation).await?;
        take("unitary changeset");
        let (events, spans) = (out.last().map_or(0, |o| o.1), out.last().map_or(0, |o| o.2));
        out.push((
            "sustained, time per changeset".into(),
            events + address.0,
            spans + address.1,
        ));
        out.remove(0);
        anyhow::Ok(out)
    })
}

/// The size of the audit tables, indexes included.
async fn audit_size(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT coalesce(sum(pg_total_relation_size(c.oid)), 0)::bigint
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'audit' AND c.relkind = 'r'",
    )
    .fetch_one(pool)
    .await?)
}

/// The size of the GIN indexes on the targets (search by object, REQ-012-17).
async fn target_index_size(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT coalesce(sum(pg_relation_size(i.indexrelid)), 0)::bigint
         FROM pg_index i JOIN pg_class c ON c.oid = i.indexrelid
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'audit' AND c.relkind = 'i'
           AND pg_get_indexdef(i.indexrelid) LIKE '%USING gin (target_ids)%'",
    )
    .fetch_one(pool)
    .await?)
}

/// Octets per audited read of a page of 100 objects, sealed, and the volume it gives.
#[allow(clippy::cast_precision_loss)] // sizes far below 2^52
async fn volume(pool: &PgPool, report: &mut String) -> anyhow::Result<()> {
    const EVENTS: usize = 2_000;
    let instance = Instance::new("bench");
    kartenn_store_pg::audit_seal::seal_once(pool, None, &instance).await?;
    sqlx::raw_sql("VACUUM ANALYZE audit.event")
        .execute(pool)
        .await?;
    let before = audit_size(pool).await?;
    let index_before = target_index_size(pool).await?;
    let journal = PgAuditJournal::new(pool.clone(), instance.clone());
    let person = RequestContext::person(Uuid::now_v7());
    for _ in 0..EVENTS {
        let ids: Vec<Uuid> = (0..100).map(|_| Uuid::now_v7()).collect();
        let event = person
            .event(&OBJECTS_READ, AuditOutcome::Success)?
            .target("address", ids)
            .detail(
                "criteria",
                serde_json::json!({ "domain": Uuid::now_v7(), "filter": "custom.rack between 1 and 2", "page": 1 }),
            )?;
        journal.record(&event).await?;
    }
    while kartenn_store_pg::audit_seal::seal_once(pool, None, &instance)
        .await?
        .is_some()
    {}
    let per_event = (audit_size(pool).await? - before) as f64 / EVENTS as f64;
    let index = (target_index_size(pool).await? - index_before) as f64 / EVENTS as f64;
    let per_day = per_event * READS_PER_SECOND * 86_400.0 / f64::from(1 << 30);
    writeln!(
        report,
        "INFO audit of reads: {per_event:.0} octets per page of 100 objects (sealed, indexes \
         included), {index:.0} of them in the index by object; at {READS_PER_SECOND} pages/s, \
         {per_day:.2} GiB/day, {:.0} GiB over {LOW_DAYS} days",
        per_day * LOW_DAYS
    )?;
    Ok(())
}

/// One measured run, in this process: the instance of `config_path` runs while the
/// workload is measured; the p95 of each budget, the sustained time per changeset and the
/// technical records lost go to `result_path`.
///
/// # Errors
///
/// Database, configuration or pipeline failures.
pub async fn measure(
    database: &str,
    data: &Dataset,
    seed: u64,
    config_path: &Path,
    result_path: &Path,
    selection: &Selection,
) -> anyhow::Result<()> {
    let pool = PgPool::connect(database)
        .await
        .with_context(|| format!("connecting to {database}"))?;
    let config = kartenn_server::config::from_toml(&std::fs::read_to_string(config_path)?)?;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let instance = tokio::spawn({
        let pool = pool.clone();
        async move {
            kartenn_server::app::run(pool, &config, async {
                let _ = stopped.await;
            })
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    let results = workload(&pool, data, seed, selection).await;
    let costs = costs();
    let _ = stop.send(());
    let report = instance.await??;
    let mut out = String::new();
    for (name, duration) in results?.into_iter().chain(costs) {
        writeln!(out, "{name}\t{}", duration.as_nanos())?;
    }
    writeln!(out, "lost\t{}", report.lost_technical)?;
    std::fs::write(result_path, out)?;
    Ok(())
}

/// Events and spans per batch of the cost measure; below the queue of an output (16 384),
/// so that the measure sees records queued, not dropped.
const COST_BATCH: u32 = 1_000;

/// The cost, on the thread of a request, of an event and of a span under the installed
/// configuration: capture, filters and queues for an event, the span of OpenTelemetry for
/// a span (formatting and writing run in the tasks of the outputs). The best of 10
/// batches, each followed by a pause that lets the outputs drain.
fn costs() -> Vec<(String, Duration)> {
    let mut event = Duration::MAX;
    let mut span = Duration::MAX;
    for _ in 0..10 {
        let t = Instant::now();
        for i in 0..COST_BATCH {
            tracing::info!(
                name: "bench.cost.event",
                state = "applied",
                changeset = "0192b5c4-7f00-7000-8000-000000000000",
                operations = i,
                violations = 0,
                duration_ms = 1.5,
                "changeset applied"
            );
        }
        event = event.min(t.elapsed() / COST_BATCH);
        std::thread::sleep(Duration::from_millis(100));
        let t = Instant::now();
        for _ in 0..COST_BATCH {
            tracing::info_span!(
                "bench.cost.span",
                db.system.name = "postgresql",
                db.operation.name = "SELECT",
                db.query.summary = "SELECT object"
            )
            .in_scope(|| {});
        }
        span = span.min(t.elapsed() / COST_BATCH);
        std::thread::sleep(Duration::from_millis(100));
    }
    vec![
        ("cost per event".into(), event),
        ("cost per span".into(), span),
    ]
}

/// The p95 of `samples` under `name`, and their mean under `name (mean)`.
fn measured(results: &mut Vec<(String, Duration)>, name: &str, samples: Vec<Duration>) {
    let mean = samples.iter().sum::<Duration>() / u32::try_from(samples.len().max(1)).unwrap_or(1);
    results.push((name.to_owned(), p95(samples)));
    results.push((format!("{name} (mean)"), mean));
}

/// The budgets of the core `selection` runs, read through the service (audited).
#[allow(clippy::too_many_lines)] // one linear scenario, step by step
async fn workload(
    pool: &PgPool,
    data: &Dataset,
    seed: u64,
    selection: &Selection,
) -> anyhow::Result<Vec<(String, Duration)>> {
    let author: Uuid = sqlx::query_scalar("SELECT id FROM person WHERE display_name = 'bench'")
        .fetch_one(pool)
        .await?;
    let rack: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'custom.rack'")
            .fetch_one(pool)
            .await?;
    let store = PgStore::new(pool.clone());
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("bench"));
    let context = RequestContext::person(author);
    // The same draws in every run.
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(2));
    let mut results = Vec::new();
    let now = OffsetDateTime::now_utc();
    let at = At(i64::try_from(now.unix_timestamp_nanos() / 1_000)?);
    // 2023-10-01T00:00Z, the start of the history, plus 100 days.
    let past = At(1_696_118_400_000_000 + 100 * 86_400_000_000);
    let draw = |rng: &mut ChaCha8Rng| {
        if rng.random_range(0..4) == 0 {
            data.prefixes[rng.random_range(0..data.prefixes.len())].id
        } else {
            data.addresses[rng.random_range(0..data.addresses.len())].id
        }
    };
    if selection.runs(READ) {
        for _ in 0..20 {
            read_object(
                &store,
                &Unrestricted,
                &journal,
                &context,
                draw(&mut rng),
                at,
                None,
                ReadAudit::default(),
            )
            .await?;
        }
        let (mut current, mut known) = (Vec::new(), Vec::new());
        for _ in 0..1_000 {
            let t = Instant::now();
            read_object(
                &store,
                &Unrestricted,
                &journal,
                &context,
                draw(&mut rng),
                at,
                None,
                ReadAudit::default(),
            )
            .await?
            .context("a loaded object is missing")?;
            current.push(t.elapsed());
            let t = Instant::now();
            read_object(
                &store,
                &Unrestricted,
                &journal,
                &context,
                draw(&mut rng),
                at,
                Some(past),
                ReadAudit::default(),
            )
            .await?
            .context("a past version is missing")?;
            known.push(t.elapsed());
        }
        measured(&mut results, "read an object (audited)", current);
        measured(&mut results, "read an object as known (audited)", known);
    }

    if selection.runs(FILTERED_PAGE) {
        let mut samples = Vec::new();
        for _ in 0..500 {
            let domain = data.domains[rng.random_range(0..data.domains.len())].id;
            let low: i64 = rng.random_range(0..900_000);
            let condition =
                Condition::Between(Some(low.to_string()), Some((low + 100_000).to_string()));
            let t = Instant::now();
            filter(pool, rack, domain, &condition, now, None, 100).await?;
            samples.push(t.elapsed());
        }
        measured(&mut results, "filtered page of 100", samples);
    }

    if selection.runs(SUBTREE) {
        let mut samples = Vec::new();
        for _ in 0..1_000 {
            let p = &data.prefixes[rng.random_range(0..data.prefixes.len())];
            let t = Instant::now();
            children(pool, p.id, now).await?;
            occupancy(pool, p.id, now)
                .await?
                .context("a loaded prefix is missing")?;
            samples.push(t.elapsed());
        }
        measured(&mut results, "subtree and occupancy", samples);
    }

    let networks: Vec<(Uuid, Uuid)> = data
        .prefixes
        .iter()
        .filter(|p| p.nature == crate::Nature::Network)
        .map(|p| (p.id, data.domains[p.domain].id))
        .collect();
    if selection.runs(UNITARY) {
        let mut samples = Vec::new();
        for _ in 0..500 {
            let network = networks[rng.random_range(0..networks.len())];
            let operation = new_address(pool, network).await?;
            samples.push(unitary(&store, author, operation).await?);
        }
        measured(&mut results, "unitary changeset", samples);
    }

    if selection.runs(SUSTAINED_RATE) {
        let done = Arc::new(AtomicUsize::new(0));
        let mut writers = Vec::new();
        let begun = Instant::now();
        for w in 0..WRITERS {
            let (pool, store, done) = (pool.clone(), store.clone(), done.clone());
            let mine: Vec<(Uuid, Uuid)> = networks
                .iter()
                .enumerate()
                .filter(|(i, _)| i % WRITERS == w)
                .map(|(_, n)| *n)
                .collect();
            writers.push(tokio::spawn(async move {
                let mut i = 0;
                while begun.elapsed() < SUSTAINED {
                    let operation = new_address(&pool, mine[i % mine.len()]).await?;
                    unitary(&store, author, operation).await?;
                    done.fetch_add(1, Ordering::Relaxed);
                    i += 1;
                }
                anyhow::Ok(())
            }));
        }
        for w in writers {
            w.await??;
        }
        let count = u32::try_from(done.load(Ordering::Relaxed).max(1))?;
        results.push((
            "sustained, time per changeset".into(),
            begun.elapsed() / count,
        ));
    }
    Ok(results)
}
