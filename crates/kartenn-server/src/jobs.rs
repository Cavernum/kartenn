//! The background tasks of an instance (spec 006, REQ-006-38): every [`POLL_EVERY`], the
//! scheduler takes one due task under a lease in the database ([`LEASE`]), renews it every
//! [`HEARTBEAT_EVERY`] while the task runs, then reschedules the task one period later. A
//! single instance runs a task at a time; one that stops leaves its lease to expire, and
//! the task runs on another.

use std::future::Future;
use std::time::Duration;

use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use tokio::sync::watch;

/// The period of the scrutiny of the due tasks.
pub const POLL_EVERY: Duration = Duration::from_secs(5);
/// The lease taken on a task.
pub const LEASE: Duration = Duration::from_secs(30);
/// The renewal of the lease while a task runs.
pub const HEARTBEAT_EVERY: Duration = Duration::from_secs(10);

/// Objects by changeset of the tasks that write (re-evaluation, public flags).
const BATCH: usize = 1_000;

/// What runs the task of a name.
pub trait Runner: Sync {
    /// Runs the task `name`.
    fn run(&self, name: &str) -> impl Future<Output = anyhow::Result<()>> + Send;
}

/// The periodic tasks of kartenn, by the names of the migration `0039_job`.
pub struct Tasks {
    /// The store the tasks write through.
    pub store: PgStore,
    /// The sources of expected values.
    pub sources: kartenn_service::expected::Sources,
}

impl std::fmt::Debug for Tasks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tasks")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl Runner for Tasks {
    async fn run(&self, name: &str) -> anyhow::Result<()> {
        match name {
            "expire-pending" => {
                let expired = kartenn_service::review::expire_pending(&self.store)
                    .await
                    .map_err(|e| anyhow::anyhow!(e.0))?;
                tracing::info!(name: "job.expire_pending", expired = expired.len(), "pending changesets expired");
            }
            "reevaluate-expected" => {
                let evaluated = kartenn_service::expected::reevaluate_outdated(
                    &self.store,
                    &self.sources,
                    BATCH,
                )
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
                tracing::info!(name: "job.reevaluate_expected", evaluated, "expected values re-evaluated");
            }
            "public-drift" => {
                let rewritten = kartenn_store_pg::registry::rewrite_public(
                    &self.store,
                    kartenn_core::registry::UPDATED,
                    BATCH,
                )
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
                tracing::info!(name: "job.public_drift", rewritten, "public flags rewritten");
            }
            "idempotency-purge" => {
                let purged = kartenn_store_pg::idempotency::purge_expired(self.store.pool())
                    .await
                    .map_err(|e| anyhow::anyhow!(e.0))?;
                tracing::info!(name: "job.idempotency_purge", purged, "expired idempotency keys purged");
            }
            "draft-retention" => {
                // Spec 024, REQ-024-7: the drafts inactive for 90 days.
                let deleted =
                    kartenn_store_pg::drafts::expire(self.store.pool(), self.store.instance())
                        .await
                        .map_err(|e| anyhow::anyhow!(e.0))?;
                tracing::info!(name: "job.draft_retention", deleted, "inactive drafts deleted");
            }
            other => {
                if let Some(import) = kartenn_store_pg::imports::job_import(other) {
                    self.import(import).await?;
                } else if kartenn_service::module::run_job(&self.store, other)
                    .await
                    .map_err(|e| anyhow::anyhow!(e.0))?
                {
                    // A background task of a module (REQ-016-16).
                    tracing::info!(name: "job.module", job = other, "module task run");
                } else {
                    anyhow::bail!("unknown task {other}");
                }
            }
        }
        Ok(())
    }
}

impl Tasks {
    /// Sorts a `sorting` import (H18), then runs the batches left of `import`
    /// (REQ-006-18), each a changeset of its author, and removes its task once the import
    /// is over, paused or cancelled; a failure keeps the task, run again one period later.
    async fn import(&self, import: Uuid) -> anyhow::Result<()> {
        self.sort(import).await?;
        let authorizer = kartenn_store_pg::rbac::PgAuthorizer::new(self.store.pool().clone());
        let status = kartenn_service::import::run_import(
            &self.store,
            &authorizer,
            import,
            kartenn_service::pipeline::MAX_OPERATIONS,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let state = status.as_ref().map(|s| s.state.as_str());
        if !matches!(state, Some("sorting" | "ready" | "running")) {
            kartenn_store_pg::imports::unschedule(self.store.pool(), import)
                .await
                .map_err(|e| anyhow::anyhow!(e.0))?;
        }
        tracing::info!(name: "job.import", %import, state = state.unwrap_or("unknown"), "import run");
        Ok(())
    }

    /// Sorts and cuts in batches the elements of `import` if it is `sorting`: it becomes
    /// `ready`, or `uploading` with its `error` when they cannot be sorted. The sort runs
    /// off the runtime threads, so that the lease is renewed meanwhile; an instance that
    /// stops leaves the import `sorting`, sorted again by the next.
    async fn sort(&self, import: Uuid) -> anyhow::Result<()> {
        let pool = self.store.pool();
        let Some(items) = kartenn_store_pg::imports::sorting(pool, import)
            .await
            .map_err(|e| anyhow::anyhow!(e.0))?
        else {
            return Ok(());
        };
        let planned = tokio::task::spawn_blocking(move || {
            kartenn_service::import::batches(items, kartenn_service::pipeline::MAX_OPERATIONS)
        })
        .await?;
        match planned {
            Ok(batches) => {
                kartenn_store_pg::imports::sorted(pool, import, &batches)
                    .await
                    .map_err(|e| anyhow::anyhow!(e.0))?;
                tracing::info!(name: "job.import.sorted", %import, batches = batches.len(), "import sorted");
            }
            Err(kartenn_service::import::ImportError::Store(e)) => anyhow::bail!(e.0),
            Err(e) => {
                let error = kartenn_rest::imports::sort_error(&e);
                kartenn_store_pg::imports::unsorted(pool, import, &error)
                    .await
                    .map_err(|e| anyhow::anyhow!(e.0))?;
                tracing::info!(name: "job.import.unsorted", %import, error = %e, "import not sorted");
            }
        }
        Ok(())
    }
}

/// The scheduler of one instance.
#[derive(Debug, Clone)]
pub struct Scheduler {
    pool: PgPool,
    holder: Uuid,
    lease: Duration,
    heartbeat: Duration,
}

impl Scheduler {
    /// For the instance `holder`, with [`LEASE`] and [`HEARTBEAT_EVERY`].
    #[must_use]
    pub const fn new(pool: PgPool, holder: Uuid) -> Self {
        Self {
            pool,
            holder,
            lease: LEASE,
            heartbeat: HEARTBEAT_EVERY,
        }
    }

    /// With another lease and renewal (tests).
    #[must_use]
    pub const fn with_lease(mut self, lease: Duration, heartbeat: Duration) -> Self {
        self.lease = lease;
        self.heartbeat = heartbeat;
        self
    }

    /// Takes one due task, if any, and runs it under its lease: the task is rescheduled
    /// whatever its outcome (a failure is logged), and dropped if the lease is lost.
    /// Returns its name.
    pub async fn tick(&self, runner: &impl Runner) -> Option<String> {
        let name = match kartenn_store_pg::jobs::claim(&self.pool, self.holder, self.lease).await {
            Ok(name) => name?,
            Err(e) => {
                tracing::error!(name: "job.claim_failed", error = %e.0, "the due tasks could not be read");
                return None;
            }
        };
        match self.under_lease(runner, &name).await {
            None => {
                tracing::warn!(name: "job.lease_lost", job = %name, "the lease was lost, the task is dropped");
            }
            Some(outcome) => {
                if let Err(e) = outcome {
                    tracing::error!(name: "job.failed", job = %name, error = format!("{e:#}"), "a background task failed");
                }
                if let Err(e) = kartenn_store_pg::jobs::finish(&self.pool, &name, self.holder).await
                {
                    tracing::error!(name: "job.finish_failed", job = %name, error = %e.0, "the task could not be rescheduled");
                }
            }
        }
        Some(name)
    }

    /// Runs the task `name`, renewing its lease; `None` if the lease is lost.
    async fn under_lease(&self, runner: &impl Runner, name: &str) -> Option<anyhow::Result<()>> {
        let task = runner.run(name);
        tokio::pin!(task);
        let mut heartbeat = tokio::time::interval(self.heartbeat);
        heartbeat.tick().await;
        loop {
            tokio::select! {
                outcome = &mut task => return Some(outcome),
                _ = heartbeat.tick() => {
                    match kartenn_store_pg::jobs::heartbeat(&self.pool, name, self.holder, self.lease).await {
                        Ok(true) => {}
                        Ok(false) => return None,
                        Err(e) => {
                            tracing::warn!(name: "job.heartbeat_failed", job = %name, error = %e.0, "the lease could not be renewed");
                        }
                    }
                }
            }
        }
    }

    /// Ticks every [`POLL_EVERY`] until `stop`; a task in progress is dropped at the stop
    /// (its transaction rolls back, its lease expires).
    pub async fn run(self, runner: impl Runner, mut stop: watch::Receiver<bool>) {
        let mut poll = tokio::time::interval(POLL_EVERY);
        loop {
            tokio::select! {
                _ = stop.changed() => break,
                _ = poll.tick() => {
                    tokio::select! {
                        _ = stop.changed() => break,
                        _ = self.tick(&runner) => {}
                    }
                }
            }
        }
    }
}
