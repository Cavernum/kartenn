//! `GET /v1/stats` (spec 024, REQ-024-14 to 16; design « C »): the counters of the home
//! page in one call, each filtered by the read rights of the caller with the predicate of
//! its list; the modules add theirs through [`StatsContributor`]. A count discloses no
//! object: the call journals no read.
//!
//! The route serves snapshots (M18): what depends on the rights alone, by fingerprint of
//! the rights, so that callers of the same bindings share it; what is the caller's own
//! (its pending changesets, those it may apply, the counters of the modules, which may
//! count what it reads as itself), by subject and fingerprint. The first read of a key
//! computes at once; later ones are served the snapshot, recomputed in the background
//! once the outbox has moved and [`Freshness::refresh`] has passed ([`Snapshots`]).

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use axum::Extension;
use axum::extract::State;
use axum::http::{HeaderMap, header};
use axum::response::Response;
use futures_util::future::try_join_all;
use kartenn_core::audit::rfc3339;
use kartenn_service::authn::Principal;
use kartenn_service::authz::{Authorizer, Subject};
use kartenn_service::events::qualified;
use kartenn_service::module::{StatsContributor, StatsScope};
use kartenn_service::reads::{CHANGESET_READ_PERMISSION, CHANGESETS};
use kartenn_service::review::proposals_for;
use kartenn_store_pg::list::{self, Kind};
use kartenn_store_pg::modules::PgModuleRead;
use kartenn_store_pg::rbac::ReadScope;
use kartenn_store_pg::{anomalies, changesets, outbox, read};
use serde::Serialize;
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, UtcOffset};
use tokio::sync::{Notify, Semaphore};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::changesets::write_store;
use crate::dated::{Dates, ETag, hex, respond};
use crate::objects::{Api, instant, store};
use crate::problem::{Problem, ProblemKind};

/// The days of [`ChangesetStats::by_day`].
pub const DAYS: u16 = 30;
/// The days of [`ChangesetStats::applied_7d`].
const WEEK: usize = 7;

/// The changesets applied (Q-024-5: every changeset, whoever its author), by UTC day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ChangesetStats {
    /// Applied over the last 7 days, today included.
    pub applied_7d: i64,
    /// Applied over the last 30 days, today included.
    pub applied_30d: i64,
    /// Applied each of the last 30 days, the oldest first and today last.
    pub by_day: Vec<i64>,
}

/// The pending changesets of the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct PendingStats {
    /// Those of others it may apply (`<resource>.apply` on everything they touch), those
    /// of `GET /v1/proposals`.
    pub to_apply: usize,
    /// Its own.
    pub mine: i64,
}

/// The open anomalies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AnomalyStats {
    /// Open anomalies whose object the caller reads.
    pub open: i64,
}

/// The body of `GET /v1/stats`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Stats {
    /// The objects the caller reads, by qualified kind (`core.prefix`, `vlan.vlan`); a
    /// kind it reads nowhere is absent.
    pub objects: BTreeMap<String, i64>,
    /// The changesets applied; absent without `core.changeset.read`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changesets: Option<ChangesetStats>,
    /// The pending changesets of the caller.
    pub pending: PendingStats,
    /// The open anomalies.
    pub anomalies: AnomalyStats,
    /// The counters of each module, under its name; a module that counts nothing the
    /// caller reads is absent.
    pub modules: BTreeMap<String, BTreeMap<String, u64>>,
    /// When these counters were computed, RFC 3339: they lag the changes made since,
    /// until the next recomputation.
    pub computed_at: String,
}

/// What depends on the rights alone.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shared {
    objects: BTreeMap<String, i64>,
    changesets: Option<ChangesetStats>,
    anomalies: AnomalyStats,
}

/// What depends on the caller itself: its changesets, and the counters of the modules
/// (the flows it requested, REQ-011-17).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Own {
    pending: PendingStats,
    /// Counts of module types that replace those of the core.
    objects: BTreeMap<String, i64>,
    modules: BTreeMap<String, BTreeMap<String, u64>>,
}

/// How the snapshots are kept (spec 024, M18; design « C »).
#[derive(Debug, Clone)]
pub struct Freshness {
    /// A snapshot is recomputed once the outbox has moved and it is this old.
    pub refresh: Duration,
    /// Only the keys read this recently are recomputed.
    pub active: Duration,
    /// A key not read for this long is forgotten.
    pub forget: Duration,
    /// Most keys of each level: beyond, the least recently read goes.
    pub keys: usize,
    /// Most recomputations at once on the instance, not to starve the pool.
    pub concurrent: usize,
    /// How often [`Snapshots::run`] looks for what is due.
    pub tick: Duration,
}

impl Default for Freshness {
    fn default() -> Self {
        Self {
            refresh: Duration::from_secs(30),
            active: Duration::from_mins(10),
            forget: Duration::from_hours(24),
            keys: 1_024,
            concurrent: 2,
            tick: Duration::from_secs(5),
        }
    }
}

/// A computed part, its date and the revision of the outbox read before it.
struct Snap<T> {
    body: Arc<T>,
    at: OffsetDateTime,
    revision: i64,
    made: Instant,
}

impl<T> Clone for Snap<T> {
    fn clone(&self) -> Self {
        Self {
            body: self.body.clone(),
            at: self.at,
            revision: self.revision,
            made: self.made,
        }
    }
}

/// A key: what recomputes it, its last snapshot, its last read, and whether it is being
/// computed (one computation per key at a time).
struct Slot<T: Part> {
    origin: T::Origin,
    snap: Option<Snap<T>>,
    read: Instant,
    busy: bool,
    /// Woken when a computation of the key ends.
    ready: Arc<Notify>,
}

type Slots<T> = Mutex<HashMap<String, Slot<T>>>;

/// What a read of a key finds.
enum Found<T> {
    /// Its snapshot, and whether this read started its recomputation.
    Snap(Snap<T>, bool),
    /// Its first computation, running: woken when it ends.
    Running(Arc<Notify>),
    /// Nothing: kept, busy, for this read to compute.
    New,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A level of the snapshots: how it is computed, and where it is kept.
trait Part: PartialEq + Send + Sync + Sized + 'static {
    /// What a computation needs besides the API.
    type Origin: Clone + Send + Sync + 'static;
    fn compute(
        api: &Api,
        origin: &Self::Origin,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<Self, Problem>> + Send;
    fn slots(snapshots: &Snapshots) -> &Slots<Self>;
}

impl Part for Shared {
    /// The read scope, and whether it reads the changesets.
    type Origin = (Arc<ReadScope>, bool);

    async fn compute(
        api: &Api,
        (scope, changesets_read): &Self::Origin,
        now: OffsetDateTime,
    ) -> Result<Self, Problem> {
        shared(api, scope, *changesets_read, now).await
    }

    fn slots(snapshots: &Snapshots) -> &Slots<Self> {
        &snapshots.shared
    }
}

impl Part for Own {
    /// The subject, and its read scope.
    type Origin = (Arc<Subject>, Arc<ReadScope>);

    async fn compute(
        api: &Api,
        (subject, scope): &Self::Origin,
        now: OffsetDateTime,
    ) -> Result<Self, Problem> {
        own(api, subject, scope, now).await
    }

    fn slots(snapshots: &Snapshots) -> &Slots<Self> {
        &snapshots.own
    }
}

/// The snapshots of the statistics of an instance (spec 024, M18): the route serves
/// them, a key read for the first time computed at once; a snapshot is recomputed in
/// the background when the outbox has moved past it and it is [`Freshness::refresh`]
/// old, by the read that finds it due or by [`Self::run`]. A recomputation that changed
/// a body calls what [`Self::on_changed`] set (the `stats` event).
pub struct Snapshots {
    settings: Freshness,
    shared: Slots<Shared>,
    own: Slots<Own>,
    permits: Arc<Semaphore>,
    computations: AtomicU64,
    changed: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for Snapshots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshots")
            .field("settings", &self.settings)
            .field("computations", &self.computations)
            .finish_non_exhaustive()
    }
}

impl Snapshots {
    /// No snapshot yet.
    #[must_use]
    pub fn new(settings: Freshness) -> Arc<Self> {
        let concurrent = settings.concurrent.max(1);
        Arc::new(Self {
            settings,
            shared: Mutex::default(),
            own: Mutex::default(),
            permits: Arc::new(Semaphore::new(concurrent)),
            computations: AtomicU64::new(0),
            changed: OnceLock::new(),
        })
    }

    /// Those of the process when the router is given none: default settings, recomputed
    /// only by the reads that find them due.
    fn fallback() -> Arc<Self> {
        static FALLBACK: OnceLock<Arc<Snapshots>> = OnceLock::new();
        FALLBACK
            .get_or_init(|| Self::new(Freshness::default()))
            .clone()
    }

    /// Calls `signal` whenever a recomputation changed a body; set once.
    pub fn on_changed(&self, signal: impl Fn() + Send + Sync + 'static) {
        let _ = self.changed.set(Box::new(signal));
    }

    /// The parts computed so far (the first read of a caller computes two: what its
    /// rights share, and what is its own).
    #[must_use]
    pub fn computations(&self) -> u64 {
        self.computations.load(Ordering::Relaxed)
    }

    /// Whether `snap` is due at `revision`, `today`: old enough, and the outbox moved past
    /// it or it was computed on an earlier UTC day (`by_day` moved on).
    fn due<T>(&self, snap: &Snap<T>, revision: i64, today: time::Date) -> bool {
        snap.made.elapsed() >= self.settings.refresh
            && (revision > snap.revision || utc_day(snap.at) < today)
    }

    /// Room for a new key: the forgotten ones go, then the least recently read idle
    /// ones while the level is full.
    fn make_room<T: Part>(&self, slots: &mut HashMap<String, Slot<T>>) {
        if slots.len() < self.settings.keys {
            return;
        }
        slots.retain(|_, slot| slot.busy || slot.read.elapsed() < self.settings.forget);
        while slots.len() >= self.settings.keys {
            let Some(oldest) = slots
                .iter()
                .filter(|(_, slot)| !slot.busy)
                .min_by_key(|(_, slot)| slot.read)
                .map(|(key, _)| key.clone())
            else {
                return;
            };
            slots.remove(&oldest);
        }
    }

    /// What a read of `key` finds, the key marked read; marked busy when it is due at
    /// `revision`, `today`, or new (then kept with `origin`).
    fn find<T: Part>(
        &self,
        key: &str,
        origin: &T::Origin,
        revision: i64,
        today: time::Date,
    ) -> Found<T> {
        let mut slots = lock(T::slots(self));
        if let Some(slot) = slots.get_mut(key) {
            slot.read = Instant::now();
            return match slot.snap.clone() {
                Some(snap) => {
                    let due = !slot.busy && self.due(&snap, revision, today);
                    slot.busy |= due;
                    Found::Snap(snap, due)
                }
                // Its first computation runs.
                None => Found::Running(slot.ready.clone()),
            };
        }
        self.make_room(&mut slots);
        slots.insert(
            key.to_owned(),
            Slot {
                origin: origin.clone(),
                snap: None,
                read: Instant::now(),
                busy: true,
                ready: Arc::new(Notify::new()),
            },
        );
        Found::New
    }

    /// The snapshot of `key`, computed at once when there is none; its recomputation
    /// started in the background when it is due at `revision`, `now`.
    async fn get<T: Part>(
        self: &Arc<Self>,
        api: &Api,
        key: String,
        origin: T::Origin,
        revision: i64,
        now: OffsetDateTime,
    ) -> Result<Snap<T>, Problem> {
        loop {
            match self.find::<T>(&key, &origin, revision, utc_day(now)) {
                Found::Snap(snap, due) => {
                    if due {
                        drop(self.background::<T>(api.clone(), key));
                    }
                    return Ok(snap);
                }
                Found::New => {
                    // In a task of its own: a request gone does not leave the key busy.
                    let this = self.clone();
                    let api = api.clone();
                    return tokio::spawn(async move { this.recompute::<T>(&api, &key).await })
                        .await
                        .map_err(|_| Problem::new(ProblemKind::Internal))?;
                }
                Found::Running(ready) => {
                    let notified = ready.notified();
                    tokio::pin!(notified);
                    notified.as_mut().enable();
                    // The computation may have ended between the lock and `enable`.
                    let running = lock(T::slots(self))
                        .get(&key)
                        .is_some_and(|slot| slot.busy && slot.snap.is_none());
                    if running {
                        notified.await;
                    }
                }
            }
        }
    }

    /// Recomputes `key` (marked busy) in the background, within the limit of the
    /// instance.
    fn background<T: Part>(self: &Arc<Self>, api: Api, key: String) -> tokio::task::JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            let Ok(_permit) = this.permits.clone().acquire_owned().await else {
                return;
            };
            if let Err(problem) = this.recompute::<T>(&api, &key).await {
                tracing::warn!(name: "stats.snapshot.failed", problem = ?problem, "a snapshot of the statistics could not be recomputed");
            }
        })
    }

    /// Computes `key` (marked busy by the caller) and keeps the result; a body that
    /// changed calls the signal. A failure keeps the previous snapshot, if any.
    async fn recompute<T: Part>(&self, api: &Api, key: &str) -> Result<Snap<T>, Problem> {
        let origin = lock(T::slots(self))
            .get(key)
            .map(|slot| slot.origin.clone());
        let computed = match origin {
            Some(origin) => self.compute::<T>(api, &origin).await,
            None => Err(Problem::new(ProblemKind::Internal)),
        };
        let mut slots = lock(T::slots(self));
        if computed.is_ok() {
            // Under the lock: whoever sees the count sees the snapshot.
            self.computations.fetch_add(1, Ordering::Relaxed);
        }
        let Some(slot) = slots.get_mut(key) else {
            return computed;
        };
        slot.busy = false;
        slot.ready.notify_waiters();
        match computed {
            Ok(snap) => {
                let changed = slot
                    .snap
                    .as_ref()
                    .is_some_and(|before| before.body != snap.body);
                slot.snap = Some(snap.clone());
                drop(slots);
                if changed && let Some(signal) = self.changed.get() {
                    signal();
                }
                Ok(snap)
            }
            Err(problem) => {
                if slot.snap.is_none() {
                    slots.remove(key);
                }
                Err(problem)
            }
        }
    }

    /// A computation of a part: the revision of the outbox read first, so that a change
    /// committed during it makes the snapshot due again.
    async fn compute<T: Part>(&self, api: &Api, origin: &T::Origin) -> Result<Snap<T>, Problem> {
        let revision = outbox::revision(&api.pool).await?;
        let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
        let body = T::compute(api, origin, now).await?;
        Ok(Snap {
            body: Arc::new(body),
            at: now,
            revision,
            made: Instant::now(),
        })
    }

    /// Forgets the keys of a level unread for [`Freshness::forget`]; whether one read
    /// within [`Freshness::active`] is idle and old enough to be due.
    fn waiting<T: Part>(&self) -> bool {
        let mut slots = lock(T::slots(self));
        slots.retain(|_, slot| slot.busy || slot.read.elapsed() < self.settings.forget);
        slots.values().any(|slot| {
            !slot.busy
                && slot.read.elapsed() < self.settings.active
                && slot
                    .snap
                    .as_ref()
                    .is_some_and(|snap| snap.made.elapsed() >= self.settings.refresh)
        })
    }

    /// Marks busy and recomputes in the background the active keys of a level due at
    /// `revision`, `today`.
    fn start_due<T: Part>(
        self: &Arc<Self>,
        api: &Api,
        revision: i64,
        today: time::Date,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let keys: Vec<String> = {
            let mut slots = lock(T::slots(self));
            slots
                .iter_mut()
                .filter(|(_, slot)| {
                    !slot.busy
                        && slot.read.elapsed() < self.settings.active
                        && slot
                            .snap
                            .as_ref()
                            .is_some_and(|snap| self.due(snap, revision, today))
                })
                .map(|(key, slot)| {
                    slot.busy = true;
                    key.clone()
                })
                .collect()
        };
        keys.into_iter()
            .map(|key| self.background::<T>(api.clone(), key))
            .collect()
    }

    /// Forgets the keys unread for [`Freshness::forget`], and recomputes the active ones
    /// that are due; returns once they are done.
    ///
    /// # Errors
    ///
    /// The database unavailable, to read the revision of the outbox.
    pub async fn sweep(self: &Arc<Self>, api: &Api) -> Result<(), Problem> {
        let shared = self.waiting::<Shared>();
        let own = self.waiting::<Own>();
        if !shared && !own {
            return Ok(());
        }
        let revision = outbox::revision(&api.pool).await?;
        let today = utc_day(OffsetDateTime::now_utc());
        let mut started = self.start_due::<Shared>(api, revision, today);
        started.extend(self.start_due::<Own>(api, revision, today));
        for task in started {
            let _ = task.await;
        }
        Ok(())
    }

    /// Sweeps every [`Freshness::tick`], until aborted.
    pub async fn run(self: Arc<Self>, api: Api) {
        let mut tick = tokio::time::interval(self.settings.tick);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            if let Err(problem) = self.sweep(&api).await {
                tracing::warn!(name: "stats.sweep.failed", problem = ?problem, "the snapshots of the statistics could not be swept");
            }
        }
    }
}

fn utc_day(at: OffsetDateTime) -> time::Date {
    at.to_offset(UtcOffset::UTC).date()
}

/// The rights a count depends on, and the database (tests share the process).
fn rights_key(api: &Api, scope: &ReadScope, changesets: bool) -> String {
    let scope = scope
        .to_json()
        .map_or_else(|| "all".to_owned(), |json| json.to_string());
    let digest = Sha256::digest(scope.as_bytes());
    format!("{}|{}|{changesets}", database(api), hex(&digest[..16]))
}

/// The subject, and its rights.
fn subject_key(subject: &Subject, rights: &str) -> String {
    let mut groups = subject.groups.clone();
    groups.sort();
    groups.dedup();
    format!(
        "{rights}|{}|{}|{}",
        subject.id,
        subject.issuer.as_deref().unwrap_or_default(),
        groups.join("\u{1f}")
    )
}

fn database(api: &Api) -> String {
    api.pool
        .connect_options()
        .get_database()
        .unwrap_or_default()
        .to_owned()
}

/// The counts of the core under `scope` at `now`.
async fn shared(
    api: &Api,
    scope: &ReadScope,
    changesets_read: bool,
    now: OffsetDateTime,
) -> Result<Shared, Problem> {
    let core = try_join_all(
        Kind::ALL
            .into_iter()
            .filter(|kind| scope.reads(kind.as_str()))
            .map(|kind| async move {
                list::count(&api.pool, kind, now, scope)
                    .await
                    .map(|n| (qualified(kind.as_str()), n))
            }),
    );
    let types: Vec<&str> = api.modules.object_types().map(|t| t.kind).collect();
    let of_modules = list::module_counts(&api.pool, &types, now, scope);
    let open = anomalies::open_count(&api.pool, scope);
    let applied = async {
        if changesets_read {
            changesets::applied_by_day(&api.pool, DAYS).await.map(Some)
        } else {
            Ok(None)
        }
    };
    let (core, of_modules, open, applied) = futures_util::try_join!(
        async { core.await.map_err(|e| store(&e)) },
        async { of_modules.await.map_err(|e| store(&e)) },
        async { open.await.map_err(|e| store(&e)) },
        async { applied.await.map_err(Problem::from) },
    )?;
    let objects: BTreeMap<String, i64> = core.into_iter().chain(of_modules).collect();
    let changesets = applied.map(|by_day| ChangesetStats {
        applied_7d: by_day.iter().rev().take(WEEK).sum(),
        applied_30d: by_day.iter().sum(),
        by_day,
    });
    Ok(Shared {
        objects,
        changesets,
        anomalies: AnomalyStats { open },
    })
}

/// What each module of the build counts, on a connection of its own.
async fn contributions(
    api: &Api,
    subject: &Subject,
    scope: &ReadScope,
    now: OffsetDateTime,
) -> Result<Vec<(&'static str, kartenn_service::module::Contribution)>, Problem> {
    let json = scope.to_json();
    let reads = |kind: &str| scope.reads(kind);
    let stats_scope = StatsScope {
        identity: Some(subject.id),
        scope: json.as_ref(),
        reads: &reads,
    };
    let contributors: Vec<(&'static str, Arc<dyn StatsContributor>)> = api
        .modules
        .modules()
        .filter_map(|m| m.stats().map(|c| (m.name(), c)))
        .collect();
    let stats_scope = &stats_scope;
    try_join_all(
        contributors
            .into_iter()
            .map(|(name, contributor)| async move {
                let mut read = PgModuleRead::open(&api.pool, instant(now), None)
                    .await
                    .map_err(|e| store(&e))?;
                let contribution = contributor.contribute(&mut read, stats_scope).await?;
                Ok::<_, Problem>((name, contribution))
            }),
    )
    .await
}

/// The pending changesets of `subject` and what the modules count for it.
async fn own(
    api: &Api,
    subject: &Subject,
    scope: &ReadScope,
    now: OffsetDateTime,
) -> Result<Own, Problem> {
    let store_ = write_store(api);
    let authorizer = api.authorizer();
    let (to_apply, mine, contributed) = futures_util::try_join!(
        async {
            proposals_for(&store_, &authorizer, subject)
                .await
                .map_err(Problem::from)
        },
        async {
            changesets::pending_of(&api.pool, subject.id)
                .await
                .map_err(Problem::from)
        },
        contributions(api, subject, scope, now),
    )?;
    let mut objects = BTreeMap::new();
    let mut modules = BTreeMap::new();
    for (name, contribution) in contributed {
        // The module decides: it reads its types beyond the scope of the core.
        for (kind, n) in contribution.objects {
            objects.insert(kind.to_owned(), i64::try_from(n).unwrap_or(i64::MAX));
        }
        if !contribution.counters.is_empty() {
            modules.insert(
                name.to_owned(),
                contribution
                    .counters
                    .into_iter()
                    .map(|(k, n)| (k.to_owned(), n))
                    .collect(),
            );
        }
    }
    Ok(Own {
        pending: PendingStats {
            to_apply: to_apply.len(),
            mine,
        },
        objects,
        modules,
    })
}

/// Whether `subject` reads the changesets: without, the counts of the changesets are
/// left out (the refusal is not journaled: nothing is asked of them).
async fn changesets_read(api: &Api, subject: &Subject) -> Result<bool, Problem> {
    Ok(api
        .authorizer()
        .authorize_many(subject, CHANGESET_READ_PERMISSION, &[CHANGESETS])
        .await?
        .is_empty())
}

/// The statistics of `subject` at `now`, computed without the snapshots: what a
/// computation of the route costs, which the bench measures (REQ-024-16).
///
/// # Errors
///
/// The database unavailable.
pub async fn compute(api: &Api, subject: &Subject, now: OffsetDateTime) -> Result<Stats, Problem> {
    let scope = api.authorizer().read_scope(subject, now).await?;
    let changesets_read = changesets_read(api, subject).await?;
    let (shared, own) = futures_util::try_join!(
        shared(api, &scope, changesets_read, now),
        own(api, subject, &scope, now),
    )?;
    Ok(assemble(&shared, &own, now))
}

fn assemble(shared: &Shared, own: &Own, at: OffsetDateTime) -> Stats {
    let mut objects = shared.objects.clone();
    objects.extend(own.objects.iter().map(|(k, n)| (k.clone(), *n)));
    Stats {
        objects,
        changesets: shared.changesets.clone(),
        pending: own.pending.clone(),
        anomalies: shared.anomalies.clone(),
        modules: own.modules.clone(),
        computed_at: rfc3339(instant(at)),
    }
}

/// `GET /v1/stats`: the counters of the home page (spec 024, REQ-024-14 to 16).
#[utoipa::path(
    get,
    path = "/stats",
    operation_id = "stats",
    tag = "operations",
    responses(
        (status = 200, description = "The counters, each filtered by the read rights of the \
            caller; a kind it reads nowhere is absent, `changesets` without \
            `core.changeset.read`. Served from the last computation for the rights of the \
            caller (`computed_at`), recomputed in the background at most every 30 s once \
            something changed; the event `stats` of `GET /v1/events` says when a \
            recomputation changed them. Its strong ETag is a digest of the body.", body = Stats),
        (status = 304, description = "Unchanged since the `ETag` of `If-None-Match`."),
    )
)]
async fn stats(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    snapshots: Option<Extension<Arc<Snapshots>>>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let snapshots = snapshots.map_or_else(Snapshots::fallback, |Extension(s)| s);
    let subject = principal.subject();
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = Dates::default().at(now, api.bound)?;
    let revision = outbox::revision(&api.pool).await?;
    let scope = Arc::new(api.authorizer().read_scope(&subject, now).await?);
    let changesets_read = changesets_read(&api, &subject).await?;
    let shared_key = rights_key(&api, &scope, changesets_read);
    let own_key = subject_key(&subject, &shared_key);
    let (shared_part, own_part) = futures_util::try_join!(
        snapshots.get::<Shared>(
            &api,
            shared_key,
            (scope.clone(), changesets_read),
            revision,
            now
        ),
        snapshots.get::<Own>(&api, own_key, (Arc::new(subject), scope), revision, now),
    )?;
    // The older of the two parts dates the whole.
    let at = shared_part.at.min(own_part.at);
    let body = assemble(&shared_part.body, &own_part.body, at);
    let bytes = serde_json::to_vec(&body).map_err(|_| Problem::new(ProblemKind::Internal))?;
    let etag = ETag::fingerprint(hex(&Sha256::digest(&bytes)[..16]));
    let response = ([(header::CONTENT_TYPE, "application/json")], bytes);
    Ok(respond(&headers, &moment, &etag, response))
}

pub(crate) fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new().routes(routes!(stats))
}
