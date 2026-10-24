//! Budgets of the event stream (spec 022, REQ-022-26; task 022-17), in process: the hub
//! and its shared reader on the bench database, streams opened by the hub itself (the
//! router adds the authentication of the opening only, measured by `bench-http`):
//!
//! - latency from the commit of a unitary changeset to the writing of its event on every
//!   stream, p95 and p99, at 50 changesets per second;
//! - memory added per idle stream, then the ceiling of streams of an instance;
//! - a storm of resumptions: every stream reopened with its cursor within 10 s, the
//!   unitary changeset under INV-34 meanwhile;
//! - the overhead of the reader (`NOTIFY` included) on the p95 of the unitary changeset;
//! - the cost of the visibility query of a batch, subjects × objects.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use kartenn_auth::keys::KeyRing;
use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_rest::events::cursor::Position;
use kartenn_rest::events::hub::{Hub, Settings, SubjectKey, Watched};
use kartenn_rest::events::reader::Reader;
use kartenn_rest::events::stream::{Body, Start};
use kartenn_service::authz::{Subject, Unrestricted};
use kartenn_service::events::{Filters, Parser};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use tokio::sync::watch;

use crate::props::{Budget, p95, progress, verdict};

/// The sizes of a run (ADR 0032).
#[derive(Debug, Clone, Copy)]
pub struct Sizes {
    /// Streams open during the measure of the latency (REQ-022-26: 2 000).
    pub streams: usize,
    /// Streams open for the memory and the storm (5 000).
    pub ceiling: usize,
    /// Distinct identities among them (200).
    pub identities: usize,
    /// Subjects of the visibility query (500).
    pub subjects: usize,
    /// Objects of the visibility query (1 000).
    pub objects: usize,
    /// Seconds of writes at 50 per second.
    pub seconds: u64,
}

impl Sizes {
    /// The reference sizes of REQ-022-26.
    pub const FULL: Self = Self {
        streams: 2_000,
        ceiling: 5_000,
        identities: 200,
        subjects: 500,
        objects: 1_000,
        seconds: 20,
    };
    /// About a tenth.
    pub const REDUCED: Self = Self {
        streams: 200,
        ceiling: 500,
        identities: 20,
        subjects: 50,
        objects: 1_000,
        seconds: 10,
    };
    /// For the tests.
    pub const SMALL: Self = Self {
        streams: 20,
        ceiling: 50,
        identities: 4,
        subjects: 5,
        objects: 100,
        seconds: 2,
    };
}

/// Writes per second (INV-36).
const RATE: u64 = 50;

/// What the clients saw: for each changeset, when each stream wrote its event.
type Seen = Arc<Mutex<HashMap<Uuid, Vec<Instant>>>>;

fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
        })
        .map_or(0, |pages| pages * 4)
}

async fn person(pool: &PgPool, scope: (&str, Option<Uuid>)) -> anyhow::Result<Uuid> {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'r') RETURNING id",
    )
    .fetch_one(pool)
    .await?;
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, 'r_' || replace($1::text, '-', ''), '{\"fr\": \"reader\"}',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .execute(pool)
    .await?;
    for (_, permission) in kartenn_store_pg::rbac::READ_PERMISSIONS {
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(permission)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'identity', $1, $2, $3, $4,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(identity)
    .bind(role)
    .bind(scope.0)
    .bind(scope.1)
    .execute(pool)
    .await?;
    Ok(identity)
}

async fn apply(store: &PgStore, author: Uuid, operation: Operation) -> anyhow::Result<Uuid> {
    let request = ChangesetRequest {
        author,
        motive: "bench".into(),
        period: None,
        operations: vec![operation],
        simulate: false,
        idempotency_key: None,
        apply_own: true,
    };
    match submit(store, &Unrestricted, &request, MAX_OPERATIONS).await? {
        Outcome::Applied { changeset, .. } => Ok(changeset),
        other => bail!("changeset not applied: {other:?}"),
    }
}

/// The `n`-th address of the network `10.1.0.0/16`, as a unitary operation.
fn address(domain: Uuid, n: u32) -> Operation {
    let [_, b, c, d] = (0x0a01_0000u32 + n).to_be_bytes();
    Operation::CreateAddress {
        local: None,
        domain: Ref::Id(domain),
        address: format!("10.{b}.{c}.{d}"),
        usage: AddressUsage::Host,
    }
}

/// A unitary changeset, timed.
async fn timed(
    store: &PgStore,
    author: Uuid,
    operation: Operation,
) -> anyhow::Result<(Uuid, Duration)> {
    let started = Instant::now();
    let changeset = apply(store, author, operation).await?;
    Ok((changeset, started.elapsed()))
}

/// Opens a stream for `identity` on `hub`, resumed after `cursor`, and reads it in a task
/// that notes when each changeset reached it; returns the task and the last cursor seen.
fn stream(
    hub: &Arc<Hub>,
    identity: Uuid,
    cursor: Option<i64>,
    seen: &Seen,
    ready: Option<Arc<tokio::sync::Barrier>>,
) -> anyhow::Result<tokio::task::JoinHandle<String>> {
    let opened = hub
        .open(
            SubjectKey::of(&Subject::from(identity)),
            Filters::default(),
            Watched::default(),
            cursor.map(Ok),
        )
        .map_err(|e| anyhow::anyhow!("a stream refused: {e:?}"))?;
    let position = Position {
        floor: cursor.unwrap_or(opened.floor),
        last: cursor.unwrap_or(opened.floor),
    };
    let hub = hub.clone();
    let seen = seen.clone();
    let task = tokio::spawn(async move {
        let replay = hub
            .replay(&opened.conn, &opened.replay, opened.floor)
            .await
            .unwrap_or_default();
        let mut body = Body::new(
            hub.clone(),
            opened.conn,
            opened.rx,
            Start {
                position,
                reset: opened.reset.map(|r| (r, opened.floor)),
                replay,
                expires: None,
            },
            None,
        );
        let mut parser = Parser::default();
        let mut ready = ready;
        while let Some(block) = body.next().await {
            let at = Instant::now();
            for e in parser.feed(std::str::from_utf8(&block).unwrap_or_default()) {
                if e.event == "ready"
                    && let Some(barrier) = ready.take()
                {
                    barrier.wait().await;
                }
                if e.event == "changes"
                    && let Some(c) = serde_json::from_str::<serde_json::Value>(&e.data)
                        .ok()
                        .and_then(|d| d["changeset"].as_str().and_then(|s| s.parse().ok()))
                {
                    seen.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .entry(c)
                        .or_default()
                        .push(at);
                }
            }
        }
        parser.last_id
    });
    Ok(task)
}

fn percentile(mut samples: Vec<Duration>, per: usize) -> Duration {
    samples.sort();
    let i = (samples.len() * per).div_ceil(100).saturating_sub(1);
    samples.get(i).copied().unwrap_or_default()
}

/// Measures the budgets; returns the report and whether every one holds.
///
/// # Errors
///
/// Database or pipeline failures.
#[allow(clippy::too_many_lines)] // one linear scenario, step by step
pub async fn run(database: &str, sizes: Sizes) -> anyhow::Result<(String, bool)> {
    let pool = PgPool::connect(database)
        .await
        .with_context(|| format!("connecting to {database}"))?;
    kartenn_store_pg::MIGRATOR.run(&pool).await?;
    let store = PgStore::new(pool.clone());
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'bench') RETURNING id",
    )
    .fetch_one(&pool)
    .await?;
    apply(
        &store,
        author,
        Operation::CreateDomain {
            local: Some(1),
            slug: "bench".into(),
        },
    )
    .await?;
    let domain: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = 'bench'")
        .fetch_one(&pool)
        .await?;
    apply(
        &store,
        author,
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(domain),
            prefix: "10.0.0.0/8".into(),
            nature: Nature::Container,
            point_to_point: false,
            anycast: false,
        },
    )
    .await?;
    let container: Uuid = sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '10.0.0.0/8'")
        .fetch_one(&pool)
        .await?;
    // The network of the addresses written.
    apply(
        &store,
        author,
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(domain),
            prefix: "10.1.0.0/16".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
    )
    .await?;
    // The subscribers: a third read everywhere, a third the domain, a third the subtree.
    let mut identities = Vec::new();
    for i in 0..sizes.identities.max(sizes.subjects) {
        let scope = match i % 3 {
            0 => ("global", None),
            1 => ("domain", Some(domain)),
            _ => ("prefix", Some(container)),
        };
        identities.push(person(&pool, scope).await?);
    }
    let mut report = String::new();
    let mut budgets = Vec::new();
    let mut n: u32 = 0;
    let mut next = || {
        n += 1;
        address(domain, n)
    };

    for _ in 0..20 {
        timed(&store, author, next()).await?;
    }
    // The hub.
    let ring = KeyRing::parse(&KeyRing::generate_line("k1"))?;
    let hub = Hub::new(
        Settings {
            max_streams: sizes.ceiling + 10,
            per_identity: sizes.ceiling,
            ..Settings::default()
        },
        pool.clone(),
        ring.clone(),
        Arc::default(),
    );
    // The overhead of the reader (NOTIFY included): rounds without then with a reader
    // listening and reading, no stream open, so that the drift of a shared machine
    // weighs on both alike.
    let (mut without, mut with) = (Vec::new(), Vec::new());
    for _ in 0..5 {
        for _ in 0..60 {
            without.push(timed(&store, author, next()).await?.1);
        }
        let reader = Reader::start(hub.clone(), pool.clone()).await?;
        let (stop, stopped) = watch::channel(false);
        let task = tokio::spawn(reader.run(stopped));
        tokio::time::sleep(Duration::from_millis(200)).await;
        for _ in 0..60 {
            with.push(timed(&store, author, next()).await?.1);
        }
        let _ = stop.send(true);
        let _ = task.await;
    }
    let baseline = p95(without);
    let with_reader = p95(with);
    progress(&format!(
        "unitary p95 {baseline:?} without, {with_reader:?} with the reader"
    ));
    let reader = Reader::start(hub.clone(), pool.clone()).await?;
    let counters = reader.counters.clone();
    let (stop, stopped) = watch::channel(false);
    let reader_task = tokio::spawn(reader.run(stopped.clone()));
    let checks = tokio::spawn(hub.clone().watch(stopped));

    // Latency at 50 writes per second with `streams` streams.
    let seen: Seen = Arc::default();
    let barrier = Arc::new(tokio::sync::Barrier::new(sizes.streams + 1));
    let mut tasks = Vec::new();
    for i in 0..sizes.streams {
        let identity = identities[i % sizes.identities];
        tasks.push(stream(&hub, identity, None, &seen, Some(barrier.clone()))?);
    }
    barrier.wait().await;
    let mut commits: HashMap<Uuid, Instant> = HashMap::new();
    let mut loaded = Vec::new();
    let passes_before = counters.passes.load(std::sync::atomic::Ordering::Relaxed);
    let queries_before = counters.queries.load(std::sync::atomic::Ordering::Relaxed);
    let mut tick = tokio::time::interval(Duration::from_millis(1_000 / RATE));
    let started = Instant::now();
    for _ in 0..sizes.seconds * RATE {
        tick.tick().await;
        let (changeset, took) = timed(&store, author, next()).await?;
        commits.insert(changeset, Instant::now());
        loaded.push(took);
    }
    tokio::time::sleep(Duration::from_secs(2)).await;
    let wrote = started.elapsed();
    let passes = counters.passes.load(std::sync::atomic::Ordering::Relaxed) - passes_before;
    let queries = counters.queries.load(std::sync::atomic::Ordering::Relaxed) - queries_before;
    let mut lags = Vec::new();
    let mut missing = 0usize;
    {
        let seen = seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (changeset, committed) in &commits {
            let got = seen.get(changeset).map_or(0, Vec::len);
            missing += sizes.streams.saturating_sub(got);
            for at in seen.get(changeset).into_iter().flatten() {
                lags.push(at.saturating_duration_since(*committed));
            }
        }
    }
    writeln!(
        report,
        "INFO {} streams ({} identities), {} changesets in {wrote:?}: {} deliveries, {missing} missing; reader: {passes} passes, {queries} queries",
        sizes.streams,
        sizes.identities,
        commits.len(),
        lags.len()
    )?;
    let note = format!("{} streams, {RATE} changesets/s", sizes.streams);
    budgets.push(Budget {
        name: "commit to event written, p95",
        limit: Duration::from_millis(500),
        measured: percentile(lags.clone(), 95),
        note: note.clone(),
    });
    budgets.push(Budget {
        name: "commit to event written, p99",
        limit: Duration::from_secs(1),
        measured: percentile(lags, 99),
        note,
    });
    budgets.push(Budget {
        name: "events lost on a stream",
        limit: Duration::ZERO,
        measured: Duration::from_millis(missing as u64),
        note: "count, as milliseconds".into(),
    });
    let loaded = p95(loaded);
    writeln!(
        report,
        "INFO unitary changeset p95: {baseline:?} without the reader, {with_reader:?} with it, \
         {loaded:?} with it and {} streams read in the same process",
        sizes.streams
    )?;
    for t in tasks.drain(..) {
        t.abort();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Memory per idle stream, up to the ceiling of the instance.
    let before = rss_kib();
    let quiet: Seen = Arc::default();
    let barrier = Arc::new(tokio::sync::Barrier::new(sizes.ceiling + 1));
    let mut idle = Vec::new();
    for i in 0..sizes.ceiling {
        let identity = identities[i % sizes.identities];
        idle.push((
            stream(&hub, identity, None, &quiet, Some(barrier.clone()))?,
            identity,
        ));
    }
    barrier.wait().await;
    let after = rss_kib();
    let per = (after.saturating_sub(before) * 1024) / sizes.ceiling as u64;
    writeln!(
        report,
        "INFO {} idle streams: {before} KiB then {after} KiB, {} open",
        sizes.ceiling,
        hub.open_streams()
    )?;
    budgets.push(Budget {
        name: "memory per idle stream",
        limit: Duration::from_millis(64 * 1024),
        measured: Duration::from_millis(per),
        note: "octets, as milliseconds".into(),
    });

    // The storm: every stream reopened with its cursor within 10 s, writes meanwhile.
    let floor = hub.floor();
    for (t, _) in &idle {
        t.abort();
    }
    let identities_of: Vec<Uuid> = idle.iter().map(|(_, i)| *i).collect();
    drop(idle);
    for _ in 0..20 {
        timed(&store, author, next()).await?;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let resumed_seen: Seen = Arc::default();
    let barrier = Arc::new(tokio::sync::Barrier::new(sizes.ceiling + 1));
    let started = Instant::now();
    let writer = {
        let store = store.clone();
        let mut ops: Vec<Operation> = (0..RATE * 10).map(|_| next()).collect();
        tokio::spawn(async move {
            let mut took = Vec::new();
            let mut tick = tokio::time::interval(Duration::from_millis(1_000 / RATE));
            for op in ops.drain(..) {
                tick.tick().await;
                if let Ok((_, t)) = timed(&store, author, op).await {
                    took.push(t);
                }
            }
            took
        })
    };
    let mut reopened = Vec::new();
    for identity in identities_of {
        reopened.push(stream(
            &hub,
            identity,
            Some(floor),
            &resumed_seen,
            Some(barrier.clone()),
        )?);
        // Spread over the first seconds, as the `retry:` of the stop spreads them.
        tokio::task::yield_now().await;
    }
    barrier.wait().await;
    let resumed = started.elapsed();
    let took = writer.await.unwrap_or_default();
    writeln!(
        report,
        "INFO storm: {} resumptions served in {resumed:?}; {} writes meanwhile",
        sizes.ceiling,
        took.len()
    )?;
    budgets.push(Budget {
        name: "resumption of every stream of an instance",
        limit: Duration::from_secs(10),
        measured: resumed,
        note: format!("{} streams with their cursor", sizes.ceiling),
    });
    budgets.push(Budget {
        name: "unitary changeset during the storm, p95 (INV-34)",
        limit: Duration::from_millis(200),
        measured: p95(took),
        note: format!("{RATE} changesets/s"),
    });
    for t in reopened {
        t.abort();
    }

    // The visibility query of a batch: subjects × objects, none reading everywhere.
    let authorizer = PgAuthorizer::new(pool.clone());
    let mut scopes = Vec::new();
    for (i, identity) in identities
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 3 != 0)
        .take(sizes.subjects)
    {
        let scope = authorizer
            .read_scope(&Subject::from(*identity), time::OffsetDateTime::now_utc())
            .await?;
        if let Some(json) = scope.to_json() {
            scopes.push((i32::try_from(i)?, json));
        }
    }
    let objects: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM address ORDER BY id DESC LIMIT $1")
        .bind(i64::try_from(sizes.objects)?)
        .fetch_all(&pool)
        .await?;
    let mut conn = pool.acquire().await?;
    kartenn_store_pg::events::visible(&mut conn, &scopes, &objects).await?;
    let mut costs = Vec::new();
    for _ in 0..10 {
        let t = Instant::now();
        let pairs = kartenn_store_pg::events::visible(&mut conn, &scopes, &objects).await?;
        costs.push(t.elapsed());
        if pairs.is_empty() {
            bail!("the visibility query found nothing visible");
        }
    }
    budgets.push(Budget {
        name: "visibility of a batch, p95",
        limit: Duration::from_millis(250),
        measured: p95(costs),
        note: format!(
            "{} subjects (domain or subtree, asked once per distinct scope) × {} objects; limit: half the latency budget",
            scopes.len(),
            objects.len()
        ),
    });
    drop(conn);

    let _ = stop.send(true);
    let _ = reader_task.await;
    checks.abort();
    let reference = baseline;
    budgets.push(Budget {
        name: "overhead of the reader on the unitary changeset, p95",
        limit: reference.mul_f64(0.05).max(Duration::from_millis(1)),
        measured: with_reader.saturating_sub(reference),
        note: "NOTIFY included, 5 rounds of 60 draws each way; limit: 5 % of the baseline (floor 1 ms)".into(),
    });
    let (lines, ok) = verdict(&budgets)?;
    report.push_str(&lines);
    Ok((report, ok))
}
