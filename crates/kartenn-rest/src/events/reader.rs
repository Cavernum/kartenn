//! The shared reader of an instance (spec 022, REQ-022-20; ADR 0034): one task that
//! follows the outbox above its floor, woken by `NOTIFY kartenn_outbox` on a connection
//! of its own, or by its fallback poll; each pass reads at most a batch, types its
//! objects in one query, and publishes it to the hub. Its queries do not depend on the
//! number of connections (INV-69).

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use kartenn_service::StoreError;
use kartenn_service::events::{Body, Notice, OutboxRow, translate};
use kartenn_store_pg::events::{Floor, administrative, kinds, raise, read, recent, sweep};
use kartenn_store_pg::outbox::{CHANNEL, Event};
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::sync::{Notify, watch};

use super::hub::Hub;

/// The sweep of the window of the ring (design « Plancher »).
const SWEEP_EVERY: Duration = Duration::from_hours(1);
/// Longest wait between two attempts to listen again.
const LISTEN_BACKOFF_MAX: Duration = Duration::from_secs(30);

/// What the reader counts, for the tests and the metrics.
#[derive(Debug, Default)]
pub struct Counters {
    /// Passes run.
    pub passes: AtomicU64,
    /// Queries sent to the base by the reader (its own, not the dispatcher's).
    pub queries: AtomicU64,
    /// Notifications received.
    pub notified: AtomicU64,
}

/// The notices of `events`, their objects typed by one query.
async fn notices(hub: &Hub, pool: &PgPool, events: &[Event]) -> Result<Vec<Notice>, StoreError> {
    let mut ids: Vec<sqlx::types::Uuid> = Vec::new();
    for e in events {
        let data = &e.payload["data"];
        ids.extend(
            data["objects"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().and_then(|s| s.parse::<sqlx::types::Uuid>().ok())),
        );
        if let Some(subject) = e.payload["subject"].as_str().and_then(|s| s.parse().ok()) {
            ids.push(subject);
        }
    }
    ids.sort_unstable();
    ids.dedup();
    let typed: HashMap<sqlx::types::Uuid, String> = if ids.is_empty() {
        HashMap::new()
    } else {
        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| StoreError(e.to_string()))?;
        kinds(&mut conn, &ids).await?
    };
    // The objects without type: an administrative changeset among them has the
    // connections checked at once (REQ-022-18).
    let untyped: Vec<sqlx::types::Uuid> = ids
        .iter()
        .filter(|id| !typed.contains_key(id))
        .copied()
        .collect();
    if untyped.contains(&kartenn_service::rbac::RBAC_OBJECT) {
        hub.administration_changed();
    } else if !untyped.is_empty() {
        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| StoreError(e.to_string()))?;
        if administrative(&mut conn, &untyped).await? {
            hub.administration_changed();
        }
    }
    let kind_of = |id| typed.get(&id).cloned();
    Ok(events
        .iter()
        .flat_map(|e| {
            translate(
                OutboxRow {
                    seq: e.seq,
                    changeset: e.changeset,
                    domains: &e.domains,
                    order_keys: &e.order_keys,
                    payload: &e.payload,
                },
                &kind_of,
            )
        })
        .map(|notice| with_revision_of_build(hub, notice))
        .collect())
}

/// A `labels` notice carries the revision of the dictionary, `<stored>-<compiled>`, as
/// `GET /v1/labels` serves it (spec 018, R22, R23): the outbox knows the stored one, the
/// build its fingerprint.
fn with_revision_of_build(hub: &Hub, mut notice: Notice) -> Notice {
    if let Body::Labels { revision } = &mut notice.body
        && !revision.contains('-')
    {
        let digest = crate::labels::catalogue(hub.modules());
        *revision = format!("{revision}-{}", digest.digest());
    }
    notice
}

/// The reader: its floor and the numbers it read above it.
#[derive(Debug)]
pub struct Reader {
    hub: Arc<Hub>,
    pool: PgPool,
    floor: Floor,
    seen: BTreeSet<i64>,
    /// The numbers of the window of the ring, for the sweep.
    window: BTreeSet<i64>,
    swept: Instant,
    /// What it counted.
    pub counters: Arc<Counters>,
}

impl Reader {
    /// Loads the ring from the base (the last [`super::hub::Settings::ring_age`], at most
    /// `ring_max` events) and sets the floor.
    ///
    /// # Errors
    ///
    /// Database errors.
    pub async fn start(hub: Arc<Hub>, pool: PgPool) -> Result<Self, StoreError> {
        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| StoreError(e.to_string()))?;
        let (events, floor) = recent(
            &mut conn,
            i64::try_from(hub.settings.ring_age.as_secs()).unwrap_or(i64::MAX),
            i64::try_from(hub.settings.ring_max).unwrap_or(i64::MAX),
        )
        .await?;
        drop(conn);
        let seen: BTreeSet<i64> = events.iter().map(|e| e.seq).collect();
        hub.load(notices(&hub, &pool, &events).await?, floor.floor);
        Ok(Self {
            hub,
            pool,
            window: seen.clone(),
            seen,
            floor,
            swept: Instant::now(),
            counters: Arc::default(),
        })
    }

    /// One pass: the floor raised, a batch read and published, or the floor alone.
    /// Returns the number of events read.
    ///
    /// # Errors
    ///
    /// Database errors; the next pass tries again.
    pub async fn pass(&mut self) -> Result<usize, StoreError> {
        let started = Instant::now();
        self.counters.passes.fetch_add(1, Ordering::Relaxed);
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| StoreError(e.to_string()))?;
        raise(&mut conn, &mut self.floor, &mut self.seen).await?;
        let mut events = read(
            &mut conn,
            self.floor.floor,
            &self.seen,
            self.hub.settings.batch,
        )
        .await?;
        self.counters.queries.fetch_add(3, Ordering::Relaxed);
        if self.swept.elapsed() >= SWEEP_EVERY {
            self.swept = Instant::now();
            if let Some(low) = self.window.first().copied() {
                let late = sweep(&mut conn, low - 1, self.floor.floor, &self.window).await?;
                self.counters.queries.fetch_add(1, Ordering::Relaxed);
                events.extend(late);
                events.sort_by_key(|e| e.seq);
                events.dedup_by_key(|e| e.seq);
            }
        }
        drop(conn);
        let read_count = events.len();
        let span = tracing::debug_span!(
            "events.reader.pass",
            batch = read_count,
            floor = self.floor.floor
        );
        let entered = span.enter();
        let translated = if events.is_empty() {
            Vec::new()
        } else {
            self.counters.queries.fetch_add(1, Ordering::Relaxed);
            notices(&self.hub, &self.pool, &events).await?
        };
        for e in &events {
            if e.seq > self.floor.floor {
                self.seen.insert(e.seq);
            }
            self.window.insert(e.seq);
        }
        // The window follows the ring: numbers older than its bottom go.
        let bottom = self.hub.ring_floor();
        self.window = self.window.split_off(&bottom);
        drop(entered);
        self.hub.publish(translated, self.floor.floor).await;
        self.hub.stats.reader_pass(started.elapsed());
        Ok(read_count)
    }

    /// Runs until `stop`: a pass at each notification or each poll, at once again after
    /// a full batch.
    pub async fn run(mut self, mut stop: watch::Receiver<bool>) {
        let wake = Arc::new(Notify::new());
        let listener = tokio::spawn(listen(
            self.pool.clone(),
            wake.clone(),
            self.counters.clone(),
            stop.clone(),
        ));
        let poll = self.hub.settings.poll;
        loop {
            match self.pass().await {
                Ok(n) if i64::try_from(n).unwrap_or(i64::MAX) >= self.hub.settings.batch => {
                    continue;
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(name: "events.reader.failed", error = %e.0, "a pass of the event reader failed");
                }
            }
            tokio::select! {
                () = wake.notified() => {}
                () = tokio::time::sleep(poll) => {}
                _ = stop.changed() => break,
            }
            if *stop.borrow() {
                break;
            }
        }
        listener.abort();
    }
}

/// Listens to the outbox on a connection of its own, out of the pool, and wakes the
/// reader at each notification; listens again after a loss, waiting longer each time
/// (REQ-022-20): the poll covers the gap.
async fn listen(
    pool: PgPool,
    wake: Arc<Notify>,
    counters: Arc<Counters>,
    stop: watch::Receiver<bool>,
) {
    let mut backoff = Duration::from_millis(100);
    loop {
        if *stop.borrow() {
            return;
        }
        match PgListener::connect_with(&pool).await {
            Ok(mut listener) => match listener.listen(CHANNEL).await {
                Ok(()) => {
                    backoff = Duration::from_millis(100);
                    // Events committed while it did not listen.
                    wake.notify_one();
                    while let Ok(Some(_)) = listener.try_recv().await {
                        counters.notified.fetch_add(1, Ordering::Relaxed);
                        wake.notify_one();
                    }
                    tracing::warn!(name: "events.listen.lost", "the connection listening to the outbox was lost");
                }
                Err(e) => {
                    tracing::warn!(name: "events.listen.failed", error = %e, "cannot listen to the outbox");
                }
            },
            Err(e) => {
                tracing::warn!(name: "events.listen.failed", error = %e, "cannot connect to listen to the outbox");
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(LISTEN_BACKOFF_MAX);
    }
}
