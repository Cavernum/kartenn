//! The hub of the event stream of an instance (spec 022, ADR 0034): the ring of the
//! notices read by the shared reader, the open connections, and the dispatcher that
//! decides by batch what each subscriber may see (REQ-022-7, 16, 17, 20, 21, 22).
//!
//! The reader publishes each batch once: appended to the ring and handed to every
//! connection under one lock, so that a connection that opens meanwhile replays it from
//! the ring instead of receiving it live, never both. The rights are decided outside the
//! lock, by one query for all the subjects and objects of the batch; each connection
//! then receives its events by `try_send` on its bounded channel, so that a slow
//! subscriber delays no one (INV-70).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use kartenn_auth::keys::KeyRing;
use kartenn_service::authz::Subject;
use kartenn_service::events::{Body, Filters, Kind, Notice, ResetReason, Sight, render};
use kartenn_service::module::ModuleRegistry;
use kartenn_service::{Store, StoreError};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

/// The settings of the stream (`[events]`).
#[derive(Debug, Clone)]
pub struct Settings {
    /// Keep-alive after this silence (REQ-022-10).
    pub heartbeat: Duration,
    /// Resumption points at most this often (REQ-022-9).
    pub checkpoint: Duration,
    /// Most objects an event names (REQ-022-5).
    pub bound: usize,
    /// How long the ring keeps a notice (REQ-022-7).
    pub ring_age: Duration,
    /// Most notices in the ring.
    pub ring_max: usize,
    /// Events waiting per connection (REQ-022-21).
    pub buffer: usize,
    /// Open streams per identity (REQ-022-22).
    pub per_identity: usize,
    /// Open streams per instance.
    pub max_streams: usize,
    /// The fallback poll of the reader (REQ-022-20).
    pub poll: Duration,
    /// Most events of a pass of the reader.
    pub batch: i64,
    /// `retry:` drawn in this interval at the opening, milliseconds (REQ-022-11).
    pub retry_open: (u64, u64),
    /// `retry:` drawn in this interval at the stop, milliseconds (REQ-022-12).
    pub retry_stop: (u64, u64),
    /// `Incremental: ?1` on the response (RFC 10036).
    pub incremental: bool,
    /// The grouped check of the subjects connected (REQ-022-18).
    pub recheck: Duration,
    /// A connection that cannot write for this long after an overflow is closed.
    pub overflow_close: Duration,
    /// The public origin, compared to `Origin` for a cookie session (REQ-022-15).
    pub public_origin: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            heartbeat: Duration::from_secs(15),
            checkpoint: Duration::from_secs(30),
            bound: kartenn_service::events::DEFAULT_BOUND,
            ring_age: Duration::from_mins(15),
            ring_max: 10_000,
            buffer: 256,
            per_identity: 16,
            max_streams: 5_000,
            poll: Duration::from_secs(1),
            batch: 1_000,
            retry_open: (1_000, 3_000),
            retry_stop: (500, 5_000),
            incremental: false,
            recheck: Duration::from_secs(10),
            overflow_close: Duration::from_secs(30),
            public_origin: None,
        }
    }
}

/// A number drawn in `[low, high]`, for the spread of the reconnections: no need of a
/// cryptographic source.
#[must_use]
pub fn jitter((low, high): (u64, u64)) -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(u64::from(std::process::id()));
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    low + h.finish() % (high.saturating_sub(low) + 1)
}

/// What the stream counts, read by the telemetry (REQ-022-25): counters, and the
/// observations of the histograms handed to a recorder when one is set.
#[derive(Default)]
pub struct Stats {
    /// Events written.
    pub delivered: AtomicU64,
    /// `reset` written, by reason (`too_old`, `invalid_cursor`, `overflow`,
    /// `rights_changed`).
    pub resets: [AtomicU64; 4],
    /// Events dropped by an overflow.
    pub dropped: AtomicU64,
    /// Records the lag (s) and the duration of a pass of the reader (s).
    pub recorder: std::sync::OnceLock<Arc<dyn Recorder>>,
}

impl std::fmt::Debug for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stats")
            .field("delivered", &self.delivered)
            .field("dropped", &self.dropped)
            .finish_non_exhaustive()
    }
}

/// Where the histograms of the stream go (`kartenn.events.lag`,
/// `kartenn.events.reader.duration`).
pub trait Recorder: Send + Sync {
    /// From the commit of a changeset to the writing of its event, in seconds.
    fn lag(&self, seconds: f64);
    /// A pass of the reader, in seconds.
    fn reader_pass(&self, seconds: f64);
}

impl Stats {
    /// The index of `reason` in [`Self::resets`].
    #[must_use]
    pub const fn reset_index(reason: ResetReason) -> usize {
        match reason {
            ResetReason::TooOld => 0,
            ResetReason::InvalidCursor => 1,
            ResetReason::Overflow => 2,
            ResetReason::RightsChanged => 3,
        }
    }

    /// A pass of the reader took `d`.
    pub fn reader_pass(&self, d: Duration) {
        if let Some(r) = self.recorder.get() {
            r.reader_pass(d.as_secs_f64());
        }
    }

    /// An event of a changeset applied at `time` (Unix microseconds) was written now.
    pub fn delivered(&self, time: Option<i64>) {
        self.delivered.fetch_add(1, Ordering::Relaxed);
        if let (Some(r), Some(time)) = (self.recorder.get(), time) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_micros()).unwrap_or(i64::MAX));
            #[allow(clippy::cast_precision_loss)] // a lag in seconds
            r.lag((now - time).max(0) as f64 / 1e6);
        }
    }
}

/// Why a connection ends (`events.stream.closed`, REQ-022-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The client went away.
    Client,
    /// The instance stops.
    Shutdown,
    /// The identity, the session, the account or the token was revoked.
    Revoked,
    /// The session or the token expired.
    Expired,
    /// The client could not read for too long after an overflow.
    Overflow,
}

impl Cause {
    /// Its name in the audit.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Shutdown => "shutdown",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
            Self::Overflow => "overflow",
        }
    }
}

/// What the hub hands a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// An event to write.
    Event {
        /// Its kind.
        kind: Kind,
        /// Its data.
        data: String,
        /// The objects it names.
        objects: Vec<Uuid>,
        /// Its number in the outbox.
        seq: i64,
        /// The floor of its cursor: every number up to it is delivered, invisible, or
        /// never will exist.
        floor: i64,
        /// When its changeset was applied, Unix microseconds (the lag).
        time: Option<i64>,
    },
}

/// Who holds a connection, as the dispatcher groups them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubjectKey {
    identity: Uuid,
    issuer: Option<String>,
    groups: Vec<String>,
}

impl SubjectKey {
    /// The key of `subject`.
    #[must_use]
    pub fn of(subject: &Subject) -> Self {
        let mut groups = subject.groups.clone();
        groups.sort();
        groups.dedup();
        Self {
            identity: subject.id,
            issuer: subject.issuer.clone(),
            groups,
        }
    }

    /// As a subject of the RBAC.
    #[must_use]
    pub fn subject(&self) -> Subject {
        Subject {
            id: self.identity,
            issuer: self.issuer.clone(),
            groups: self.groups.clone(),
        }
    }
}

/// An open connection, as the hub sees it.
#[derive(Debug)]
pub struct Conn {
    /// Its number in the instance.
    pub id: u64,
    /// Its subscriber.
    pub subject: SubjectKey,
    /// Its filters.
    pub filters: Filters,
    tx: mpsc::Sender<Message>,
    /// Since when it must start over, and why, until it writes its `reset`.
    overflow: Mutex<Option<(Instant, ResetReason)>>,
    /// Events dropped because of an overflow.
    dropped: AtomicU64,
    /// Why it must end, once it must.
    end: Mutex<Option<Cause>>,
    /// Wakes its body: a `reset` or an end to write.
    pub wake: tokio::sync::Notify,
    /// What the revocation checks need.
    pub watch: Watched,
}

impl Conn {
    /// The identity of its subscriber.
    #[must_use]
    pub const fn subject_identity(&self) -> Uuid {
        self.subject.identity
    }
}

/// What a connection is checked against while it runs (REQ-022-18).
#[derive(Debug, Clone, Default)]
pub struct Watched {
    /// For a session cookie: when it started (Unix seconds), checked against the
    /// revocations of its identity.
    pub session_started: Option<i64>,
    /// A service account.
    pub service: bool,
}

/// The ring and the connections, under one lock.
#[derive(Debug, Default)]
struct Inner {
    /// Notices by number, oldest first, with when they were read.
    ring: VecDeque<(Instant, Arc<Notice>)>,
    /// Every number above it is in the ring, or gave no notice, or is not read yet.
    ring_floor: i64,
    conns: BTreeMap<u64, Arc<Conn>>,
    next: u64,
    /// Open connections per identity.
    per_identity: HashMap<Uuid, usize>,
}

/// The read scope of a subject, as the dispatcher uses it.
#[derive(Debug, Clone, PartialEq)]
struct Scope {
    /// The kinds read everywhere without confinement: decided without the base.
    everywhere: HashSet<String>,
    /// The rest, for `kartenn_scopes`.
    json: serde_json::Value,
    at: Instant,
}

/// Why a stream is not opened (before the base is asked).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The instance holds its ceiling of streams, or stops: 503.
    Instance,
    /// The identity holds its ceiling: 429.
    Identity,
}

/// What the connection starts with.
#[derive(Debug)]
pub struct Opened {
    /// The connection.
    pub conn: Arc<Conn>,
    /// Its channel.
    pub rx: mpsc::Receiver<Message>,
    /// The notices to replay, above the floor of the cursor.
    pub replay: Vec<Arc<Notice>>,
    /// The floor of the hub when it opened: the ring covers everything up to it.
    pub floor: i64,
    /// Why it starts over, if it does.
    pub reset: Option<ResetReason>,
}

/// The hub of an instance.
#[derive(Debug)]
pub struct Hub {
    /// The settings.
    pub settings: Settings,
    /// The instance, in `ready`.
    pub instance: Uuid,
    /// The keys of the cursors.
    pub ring_keys: KeyRing,
    pool: PgPool,
    modules: Arc<ModuleRegistry>,
    inner: Mutex<Inner>,
    /// The floor published: every event up to it was handed to the connections.
    floor: AtomicI64,
    open: AtomicUsize,
    closing: watch::Sender<bool>,
    scopes: Mutex<HashMap<SubjectKey, Scope>>,
    /// Woken when an administrative changeset was applied (REQ-022-18).
    admin: tokio::sync::Notify,
    /// Ready once the ring is loaded.
    loaded: AtomicBool,
    /// What it counts.
    pub stats: Stats,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a subject may see of a batch.
struct SubjectSight<'a> {
    everywhere: &'a HashSet<String>,
    kinds: &'a HashMap<Uuid, String>,
    visible: HashSet<Uuid>,
    follows: HashSet<Uuid>,
}

impl Sight for SubjectSight<'_> {
    fn sees(&self, id: Uuid) -> bool {
        self.visible.contains(&id)
            || self
                .kinds
                .get(&id)
                .is_some_and(|k| self.everywhere.contains(k))
    }

    fn follows(&self, changeset: Uuid) -> bool {
        self.follows.contains(&changeset)
    }
}

/// The objects and domains a notice names, with their qualified types (domains are
/// `core.domain`).
fn named(notice: &Notice, out: &mut HashMap<Uuid, String>) {
    if let Body::Changes(changes) = &notice.body {
        for o in &changes.objects {
            out.insert(o.id, o.kind.clone());
        }
        for d in &changes.domains {
            out.insert(*d, "core.domain".to_owned());
        }
    }
}

impl Hub {
    /// A hub on `pool`, the ring empty until [`Self::load`].
    #[must_use]
    pub fn new(
        settings: Settings,
        pool: PgPool,
        ring_keys: KeyRing,
        modules: Arc<ModuleRegistry>,
    ) -> Arc<Self> {
        Arc::new(Self {
            settings,
            instance: Uuid::now_v7(),
            ring_keys,
            pool,
            modules,
            inner: Mutex::new(Inner::default()),
            floor: AtomicI64::new(0),
            open: AtomicUsize::new(0),
            closing: watch::channel(false).0,
            scopes: Mutex::new(HashMap::new()),
            admin: tokio::sync::Notify::new(),
            loaded: AtomicBool::new(false),
            stats: Stats::default(),
        })
    }

    /// The modules of the build: the types a filter may name.
    #[must_use]
    pub fn modules(&self) -> &ModuleRegistry {
        &self.modules
    }

    /// The floor published.
    #[must_use]
    pub fn floor(&self) -> i64 {
        self.floor.load(Ordering::SeqCst)
    }

    /// Open streams.
    #[must_use]
    pub fn open_streams(&self) -> usize {
        self.open.load(Ordering::SeqCst)
    }

    /// The signal of the stop.
    #[must_use]
    pub fn closing(&self) -> watch::Receiver<bool> {
        self.closing.subscribe()
    }

    /// Whether the instance stops.
    #[must_use]
    pub fn is_closing(&self) -> bool {
        *self.closing.borrow()
    }

    /// Stops: no new stream (503), and every connection writes its `retry:` and ends
    /// (REQ-022-12).
    pub fn shutdown(&self) {
        self.closing.send_replace(true);
    }

    /// Fills the ring at the start with the notices read again from the base, and sets
    /// the floor (REQ-022-7).
    pub fn load(&self, notices: Vec<Notice>, floor: i64) {
        let now = Instant::now();
        let mut inner = lock(&self.inner);
        inner.ring = notices.into_iter().map(|n| (now, Arc::new(n))).collect();
        inner.ring_floor = inner
            .ring
            .front()
            .map_or(floor, |(_, n)| (n.seq - 1).min(floor));
        drop(inner);
        self.floor.store(floor, Ordering::SeqCst);
        self.loaded.store(true, Ordering::SeqCst);
    }

    /// Opens a connection of `subject` (REQ-022-22: ceilings decided without the base),
    /// resuming after `resume` if given: its floor, or why it does not open.
    ///
    /// # Errors
    ///
    /// [`Refused`] beyond a ceiling or at the stop.
    pub fn open(
        &self,
        subject: SubjectKey,
        filters: Filters,
        watch: Watched,
        resume: Option<Result<i64, ()>>,
    ) -> Result<Opened, Refused> {
        if self.is_closing() {
            return Err(Refused::Instance);
        }
        let mut inner = lock(&self.inner);
        if self.open.load(Ordering::SeqCst) >= self.settings.max_streams {
            return Err(Refused::Instance);
        }
        let held = inner
            .per_identity
            .get(&subject.identity)
            .copied()
            .unwrap_or(0);
        if held >= self.settings.per_identity {
            return Err(Refused::Identity);
        }
        *inner.per_identity.entry(subject.identity).or_default() += 1;
        self.open.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(self.settings.buffer.max(1));
        inner.next += 1;
        let conn = Arc::new(Conn {
            id: inner.next,
            subject,
            filters,
            tx,
            overflow: Mutex::new(None),
            dropped: AtomicU64::new(0),
            end: Mutex::new(None),
            wake: tokio::sync::Notify::new(),
            watch,
        });
        inner.conns.insert(conn.id, conn.clone());
        let floor = self.floor();
        let (replay, reset) = match resume {
            None => (Vec::new(), None),
            Some(Err(())) => (Vec::new(), Some(ResetReason::InvalidCursor)),
            Some(Ok(from)) if from < inner.ring_floor => (Vec::new(), Some(ResetReason::TooOld)),
            Some(Ok(from)) => (
                inner
                    .ring
                    .iter()
                    .filter(|(_, n)| n.seq > from)
                    .map(|(_, n)| n.clone())
                    .collect(),
                None,
            ),
        };
        drop(inner);
        Ok(Opened {
            conn,
            rx,
            replay,
            floor,
            reset,
        })
    }

    /// Releases the connection `id` (its body dropped).
    pub fn release(&self, id: u64) {
        let mut inner = lock(&self.inner);
        if let Some(conn) = inner.conns.remove(&id) {
            self.forget(&mut inner, &conn);
        }
    }

    fn forget(&self, inner: &mut Inner, conn: &Conn) {
        self.open.fetch_sub(1, Ordering::SeqCst);
        let identity = conn.subject.identity;
        if let Some(n) = inner.per_identity.get_mut(&identity) {
            *n -= 1;
            if *n == 0 {
                inner.per_identity.remove(&identity);
            }
        }
    }

    /// The read scope of `key`, kept [`Settings::recheck`].
    async fn scope(&self, key: &SubjectKey) -> Result<Scope, StoreError> {
        self.scope_checked(key, false).await.map(|(s, _)| s)
    }

    /// The read scope of `key`, read again with `force`; and whether it changed since
    /// it was last read.
    async fn scope_checked(
        &self,
        key: &SubjectKey,
        force: bool,
    ) -> Result<(Scope, bool), StoreError> {
        let before = lock(&self.scopes).get(key).cloned();
        if let Some(s) = &before
            && !force
            && s.at.elapsed() < self.settings.recheck
        {
            return Ok((s.clone(), false));
        }
        let authorizer = PgAuthorizer::new(self.pool.clone()).with_modules(self.modules.clone());
        let read = authorizer
            .read_scope(&key.subject(), time::OffsetDateTime::now_utc())
            .await?;
        let mut everywhere = HashSet::new();
        let mut rest = serde_json::Map::new();
        if let Some(kinds) = read.kinds() {
            for (kind, v) in kinds {
                if v.is_empty() {
                    continue;
                }
                if v.global && v.confinement.is_none() {
                    everywhere.insert(kartenn_service::events::qualified(kind));
                } else {
                    rest.insert((*kind).to_owned(), v.to_json());
                }
            }
        }
        let scope = Scope {
            everywhere,
            json: serde_json::Value::Object(rest),
            at: Instant::now(),
        };
        lock(&self.scopes).insert(key.clone(), scope.clone());
        let changed =
            before.is_some_and(|b| b.everywhere != scope.everywhere || b.json != scope.json);
        Ok((scope, changed))
    }

    /// Reads the scope of `key` at the opening of a stream, so that a later change of its
    /// rights is seen by [`Self::recheck`]; kept for the next ones.
    ///
    /// # Errors
    ///
    /// Database errors.
    pub async fn prime(&self, key: &SubjectKey) -> Result<(), StoreError> {
        self.scope(key).await.map(|_| ())
    }

    /// An administrative changeset was applied: the subjects are checked at once.
    pub fn administration_changed(&self) {
        self.admin.notify_one();
    }

    /// The grouped check of the connections (REQ-022-18): an identity disabled or whose
    /// sessions were revoked after the opening ends them (`revoked`); a subject whose
    /// read scope changed starts over (`reset` `rights_changed`).
    ///
    /// # Errors
    ///
    /// Database errors.
    pub async fn recheck(&self) -> Result<(), StoreError> {
        let conns = self.connections();
        if conns.is_empty() {
            return Ok(());
        }
        let mut ids: Vec<Uuid> = conns.iter().map(|c| c.subject.identity).collect();
        ids.sort_unstable();
        ids.dedup();
        let states: HashMap<Uuid, (bool, Option<i64>)> = {
            let mut db = self
                .pool
                .acquire()
                .await
                .map_err(|e| StoreError(e.to_string()))?;
            kartenn_store_pg::events::states(&mut db, &ids)
                .await?
                .into_iter()
                .map(|(id, disabled, revoked)| (id, (disabled, revoked)))
                .collect()
        };
        for conn in &conns {
            let revoked = match states.get(&conn.subject.identity) {
                None | Some((true, _)) => true,
                Some((false, Some(after))) => conn
                    .watch
                    .session_started
                    .is_some_and(|started| started <= *after),
                Some((false, None)) => false,
            };
            if revoked {
                Self::end(conn, Cause::Revoked);
            }
        }
        let mut subjects: Vec<SubjectKey> = conns.iter().map(|c| c.subject.clone()).collect();
        subjects.sort();
        subjects.dedup();
        let mut changed = HashSet::new();
        for s in &subjects {
            if self.scope_checked(s, true).await?.1 {
                changed.insert(s.clone());
            }
        }
        for conn in conns.iter().filter(|c| changed.contains(&c.subject)) {
            Self::restart(conn, ResetReason::RightsChanged);
        }
        Ok(())
    }

    /// Checks the connections every [`Settings::recheck`], and at once after an
    /// administrative changeset, until `stop`.
    pub async fn watch(self: Arc<Self>, mut stop: watch::Receiver<bool>) {
        loop {
            tokio::select! {
                () = tokio::time::sleep(self.settings.recheck) => {}
                () = self.admin.notified() => {}
                _ = stop.changed() => return,
            }
            if *stop.borrow() {
                return;
            }
            if let Err(e) = self.recheck().await {
                tracing::warn!(name: "events.recheck.failed", error = %e.0, "the subjects of the event streams could not be checked");
            }
        }
    }

    /// Forgets the scopes of `identities` (their rights changed), all of them with
    /// `None`.
    pub fn forget_scopes(&self, identities: Option<&HashSet<Uuid>>) {
        let mut scopes = lock(&self.scopes);
        match identities {
            None => scopes.clear(),
            Some(ids) => scopes.retain(|k, _| !ids.contains(&k.identity)),
        }
    }

    /// What each of `subjects` may see of `notices` now: one query for all of them
    /// (design « Visibilité par lot »).
    async fn sights(
        &self,
        subjects: &[SubjectKey],
        notices: &[Arc<Notice>],
        kinds: &HashMap<Uuid, String>,
    ) -> Result<Vec<(Scope, HashSet<Uuid>, HashSet<Uuid>)>, StoreError> {
        let mut scopes = Vec::with_capacity(subjects.len());
        for s in subjects {
            scopes.push(self.scope(s).await?);
        }
        let objects: Vec<Uuid> = kinds.keys().copied().collect();
        let asked: Vec<(i32, serde_json::Value)> = scopes
            .iter()
            .enumerate()
            .filter(|(_, s)| s.json.as_object().is_some_and(|o| !o.is_empty()))
            .map(|(i, s)| (i32::try_from(i).unwrap_or(i32::MAX), s.json.clone()))
            .collect();
        let mut visible: Vec<HashSet<Uuid>> = vec![HashSet::new(); subjects.len()];
        if !asked.is_empty() && !objects.is_empty() {
            let mut conn = self
                .pool
                .acquire()
                .await
                .map_err(|e| StoreError(e.to_string()))?;
            for (n, id) in kartenn_store_pg::events::visible(&mut conn, &asked, &objects).await? {
                if let Some(v) = usize::try_from(n).ok().and_then(|n| visible.get_mut(n)) {
                    v.insert(id);
                }
            }
        }
        // The pending changesets: their author, and who could decide them (REQ-022-17).
        let mut follows: Vec<HashSet<Uuid>> = vec![HashSet::new(); subjects.len()];
        let pending: BTreeSet<(Uuid, Option<Uuid>)> = notices
            .iter()
            .filter_map(|n| match &n.body {
                Body::Changeset {
                    changeset, author, ..
                } => Some((*changeset, *author)),
                _ => None,
            })
            .collect();
        if !pending.is_empty() {
            let store = PgStore::new(self.pool.clone());
            let authorizer =
                PgAuthorizer::new(self.pool.clone()).with_modules(self.modules.clone());
            for (changeset, author) in pending {
                let found = store.pending(changeset).await?;
                for (i, s) in subjects.iter().enumerate() {
                    let theirs = author == Some(s.identity)
                        || found
                            .as_ref()
                            .is_some_and(|f| f.request.author == s.identity);
                    let decides = match &found {
                        Some(found) if !theirs => {
                            kartenn_service::review::may_validate(&authorizer, found, &s.subject())
                                .await?
                        }
                        _ => false,
                    };
                    if theirs || decides {
                        follows[i].insert(changeset);
                    }
                }
            }
        }
        Ok(scopes
            .into_iter()
            .zip(visible)
            .zip(follows)
            .map(|((s, v), f)| (s, v, f))
            .collect())
    }

    /// The events of `notices` for `conn`, their floors set so that a cursor never passes
    /// an event it has not delivered: below the next one delivered, at most `floor`.
    fn events_for(
        &self,
        conn: &Conn,
        notices: &[Arc<Notice>],
        sight: &dyn Sight,
        floor: i64,
    ) -> Vec<Message> {
        let mut out: Vec<Message> = notices
            .iter()
            .filter_map(|n| {
                render(n, &conn.filters, sight, self.settings.bound).map(|r| Message::Event {
                    kind: r.kind,
                    data: r.data,
                    objects: r.objects,
                    seq: n.seq,
                    floor,
                    time: n.time,
                })
            })
            .collect();
        let mut next: Option<i64> = None;
        for m in out.iter_mut().rev() {
            let Message::Event { seq, floor: f, .. } = m;
            if let Some(next) = next {
                *f = (*f).min(next - 1);
            }
            next = Some(*seq);
        }
        out
    }

    /// The events to replay to a connection that resumes: the subject's sight of the
    /// ring above its cursor, decided now.
    ///
    /// # Errors
    ///
    /// Database errors.
    pub async fn replay(
        &self,
        conn: &Conn,
        notices: &[Arc<Notice>],
        floor: i64,
    ) -> Result<Vec<Message>, StoreError> {
        if notices.is_empty() {
            return Ok(Vec::new());
        }
        let mut kinds = HashMap::new();
        for n in notices {
            named(n, &mut kinds);
        }
        let mut sights = self
            .sights(std::slice::from_ref(&conn.subject), notices, &kinds)
            .await?;
        let (scope, visible, follows) = sights.pop().unwrap_or_else(|| {
            (
                Scope {
                    everywhere: HashSet::new(),
                    json: serde_json::Value::Null,
                    at: Instant::now(),
                },
                HashSet::new(),
                HashSet::new(),
            )
        });
        let sight = SubjectSight {
            everywhere: &scope.everywhere,
            kinds: &kinds,
            visible,
            follows,
        };
        Ok(self.events_for(conn, notices, &sight, floor))
    }

    /// Publishes a batch read by the reader, `floor` its floor (REQ-022-16, 20, 21):
    /// into the ring, then to every connection after its rights; the floor published
    /// last. The rights unknown (the base unreachable), the batch is dropped for the
    /// connections, which start over (`reset`).
    pub async fn publish(&self, notices: Vec<Notice>, floor: i64) {
        let notices: Vec<Arc<Notice>> = notices.into_iter().map(Arc::new).collect();
        let conns: Vec<Arc<Conn>> = {
            let mut inner = lock(&self.inner);
            let now = Instant::now();
            inner.ring.extend(notices.iter().map(|n| (now, n.clone())));
            self.trim(&mut inner, now);
            inner.conns.values().cloned().collect()
        };
        if !notices.is_empty() && !conns.is_empty() {
            let mut kinds = HashMap::new();
            for n in &notices {
                named(n, &mut kinds);
            }
            let mut subjects: Vec<SubjectKey> = conns.iter().map(|c| c.subject.clone()).collect();
            subjects.sort();
            subjects.dedup();
            match self.sights(&subjects, &notices, &kinds).await {
                Ok(sights) => {
                    let by_subject: HashMap<&SubjectKey, SubjectSight<'_>> = subjects
                        .iter()
                        .zip(sights.iter())
                        .map(|(k, (scope, visible, follows))| {
                            (
                                k,
                                SubjectSight {
                                    everywhere: &scope.everywhere,
                                    kinds: &kinds,
                                    visible: visible.clone(),
                                    follows: follows.clone(),
                                },
                            )
                        })
                        .collect();
                    for conn in &conns {
                        let Some(sight) = by_subject.get(&conn.subject) else {
                            continue;
                        };
                        for message in self.events_for(conn, &notices, sight, floor) {
                            self.send(conn, message);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(name: "events.visibility.failed", error = %e.0, "the rights of a batch could not be decided");
                    for conn in &conns {
                        Self::restart(conn, ResetReason::Overflow);
                    }
                }
            }
        }
        self.floor.fetch_max(floor, Ordering::SeqCst);
        self.close_overflowed();
    }

    /// Hands `message` to `conn`, or marks it overflowed (REQ-022-21).
    fn send(&self, conn: &Conn, message: Message) {
        let mut overflow = lock(&conn.overflow);
        if overflow.is_some() || conn.tx.try_send(message).is_err() {
            overflow.get_or_insert((Instant::now(), ResetReason::Overflow));
            conn.dropped.fetch_add(1, Ordering::Relaxed);
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Makes `conn` start over for `reason`: what waits is dropped, its `reset` written
    /// first.
    fn restart(conn: &Conn, reason: ResetReason) {
        let mut overflow = lock(&conn.overflow);
        if overflow.is_none() {
            *overflow = Some((Instant::now(), reason));
        }
        drop(overflow);
        conn.wake.notify_one();
    }

    /// Ends `conn` for `cause`, whatever waits in its channel.
    pub fn end(conn: &Conn, cause: Cause) {
        lock(&conn.end).get_or_insert(cause);
        conn.wake.notify_one();
    }

    /// Why `conn` must end, if it must.
    #[must_use]
    pub fn ending(conn: &Conn) -> Option<Cause> {
        *lock(&conn.end)
    }

    /// Makes the connections of `identities` start over (their rights changed,
    /// REQ-022-18).
    pub fn rights_changed(&self, identities: &HashSet<Uuid>) {
        self.forget_scopes(Some(identities));
        for conn in lock(&self.inner)
            .conns
            .values()
            .filter(|c| identities.contains(&c.subject.identity))
        {
            Self::restart(conn, ResetReason::RightsChanged);
        }
    }

    /// The connections, for the grouped checks.
    #[must_use]
    pub fn connections(&self) -> Vec<Arc<Conn>> {
        lock(&self.inner).conns.values().cloned().collect()
    }

    /// Takes the overflow of `conn` (its `reset` about to be written): drops what waits
    /// in `rx`; the cursor of the `reset` is the floor published, which no event dropped
    /// can pass unseen since the `reset` invalidates everything.
    pub fn take_overflow(
        &self,
        conn: &Conn,
        rx: &mut mpsc::Receiver<Message>,
    ) -> Option<(i64, ResetReason)> {
        let mut overflow = lock(&conn.overflow);
        let (_, reason) = (*overflow)?;
        let floor = self.floor();
        // What waits was handed before the restart: dropped, the `reset` covers it.
        while rx.try_recv().is_ok() {}
        *overflow = None;
        Some((floor, reason))
    }

    /// Events dropped for `conn` so far.
    #[must_use]
    pub fn dropped(conn: &Conn) -> u64 {
        conn.dropped.load(Ordering::Relaxed)
    }

    /// Closes the connections overflowed for too long: their channel closed, they end
    /// when written to; they no longer count against the ceilings.
    fn close_overflowed(&self) {
        let mut inner = lock(&self.inner);
        let stale: Vec<u64> = inner
            .conns
            .values()
            .filter(|c| {
                lock(&c.overflow)
                    .is_some_and(|(at, _)| at.elapsed() >= self.settings.overflow_close)
            })
            .map(|c| c.id)
            .collect();
        for id in stale {
            if let Some(conn) = inner.conns.remove(&id) {
                self.forget(&mut inner, &conn);
                Self::end(&conn, Cause::Overflow);
            }
        }
    }

    /// Drops the notices of the ring past their age or beyond its size.
    fn trim(&self, inner: &mut Inner, now: Instant) {
        while let Some((at, n)) = inner.ring.front() {
            let old = now.duration_since(*at) > self.settings.ring_age;
            if !(old || inner.ring.len() > self.settings.ring_max) {
                break;
            }
            let seq = n.seq;
            inner.ring.pop_front();
            inner.ring_floor = inner.ring_floor.max(seq);
        }
    }

    /// The bottom of the ring: a cursor below it starts over (`too_old`).
    #[must_use]
    pub fn ring_floor(&self) -> i64 {
        lock(&self.inner).ring_floor
    }
}
