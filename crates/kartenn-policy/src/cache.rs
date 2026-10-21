//! The cache of policy snapshots of an instance (feature `pg`; REQ-010-16, 28; design 010
//! « Instantané de politique »).
//!
//! A snapshot read at (`valid_at`, `as_of`) holds over the span of valid time in which no
//! version of a policy object starts or ends ([`read::stable_span`]):
//! - asserted **now** (`as_of` absent or not past), it holds until the policy changes: it
//!   is kept under the revision, read first (one row) at each request;
//! - asserted in the **past**, it never changes: no revision is read.
//!
//! The entries live in one LRU of [`CAPACITY`] snapshots.

use std::sync::{Arc, Mutex, PoisonError};

use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;

use crate::read;
use crate::snapshot::PolicySnapshot;

/// Snapshots kept by an instance (design: 16 past snapshots, plus the present ones).
pub const CAPACITY: usize = 16;

/// The revision cited by a snapshot asserted in the past: the revision counter is not
/// versioned, so a past snapshot cites none.
pub const PAST: u64 = 0;

struct Entry {
    /// `None`: asserted now, valid while the revision is `revision`.
    as_of: Option<OffsetDateTime>,
    revision: u64,
    /// The span of valid time it holds over, start included, end excluded.
    from: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
    snapshot: Arc<PolicySnapshot>,
    /// Logical time of the last use, for eviction.
    used: u64,
}

impl Entry {
    fn covers(&self, valid_at: OffsetDateTime) -> bool {
        self.from.is_none_or(|from| from <= valid_at)
            && self.until.is_none_or(|until| valid_at < until)
    }
}

#[derive(Default)]
struct Inner {
    entries: Vec<Entry>,
    clock: u64,
    /// Snapshots read from the database, for the tests and the metrics.
    loads: u64,
}

/// The snapshots of the policy an instance has read, shared by its requests.
#[derive(Default)]
pub struct SnapshotCache {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for SnapshotCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.lock();
        f.debug_struct("SnapshotCache")
            .field("entries", &inner.entries.len())
            .field("loads", &inner.loads)
            .finish()
    }
}

impl SnapshotCache {
    /// An empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshots read from the database since the cache was made.
    #[must_use]
    pub fn loads(&self) -> u64 {
        self.lock().loads
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The policy valid at `valid_at` as asserted at `as_of` (now when `None` or not
    /// past), read through the cache on `conn` (REQ-010-16).
    ///
    /// # Errors
    /// A failure of the database, or a stored row that does not read back.
    pub async fn snapshot(
        &self,
        conn: &mut PgConnection,
        valid_at: OffsetDateTime,
        as_of: Option<OffsetDateTime>,
    ) -> Result<Arc<PolicySnapshot>, StoreError> {
        let as_of = as_of.filter(|t| *t < OffsetDateTime::now_utc());
        let revision = match as_of {
            None => read::revision(conn).await?,
            Some(_) => PAST,
        };
        if let Some(hit) = self.find(valid_at, as_of, revision) {
            return Ok(hit);
        }
        let (from, until) = read::stable_span(conn, valid_at, as_of).await?;
        let parts = read::parts(conn, valid_at, as_of, revision).await?;
        let snapshot = Arc::new(PolicySnapshot::new(parts));
        let mut inner = self.lock();
        inner.loads += 1;
        inner.clock += 1;
        let used = inner.clock;
        // A present snapshot of an older revision will never be read again.
        inner
            .entries
            .retain(|e| e.as_of.is_some() || e.revision == revision);
        if inner.entries.len() >= CAPACITY
            && let Some(oldest) = inner
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.used)
                .map(|(i, _)| i)
        {
            inner.entries.swap_remove(oldest);
        }
        inner.entries.push(Entry {
            as_of,
            revision,
            from,
            until,
            snapshot: snapshot.clone(),
            used,
        });
        Ok(snapshot)
    }

    fn find(
        &self,
        valid_at: OffsetDateTime,
        as_of: Option<OffsetDateTime>,
        revision: u64,
    ) -> Option<Arc<PolicySnapshot>> {
        let mut inner = self.lock();
        inner.clock += 1;
        let clock = inner.clock;
        let entry = inner.entries.iter_mut().find(|e| {
            e.as_of == as_of && (as_of.is_some() || e.revision == revision) && e.covers(valid_at)
        })?;
        entry.used = clock;
        Some(entry.snapshot.clone())
    }
}
