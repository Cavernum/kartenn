//! Background tasks under a lease in the database: one run by due date whatever the number
//! of instances, taken over after the lease when the holder stops (spec 006, REQ-006-38;
//! task 006-20).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use kartenn_server::jobs::{Runner, Scheduler, Tasks};
use sqlx::PgPool;
use sqlx::types::Uuid;

/// The four periodic tasks of the migration.
const SEEDED: [&str; 4] = [
    "expire-pending",
    "reevaluate-expected",
    "public-drift",
    "idempotency-purge",
];

/// Counts the runs by task, each taking a little time.
#[derive(Default)]
struct Counter(Mutex<HashMap<String, u32>>);

impl Counter {
    fn runs(&self) -> HashMap<String, u32> {
        self.0.lock().unwrap().clone()
    }
}

impl Runner for Counter {
    async fn run(&self, name: &str) -> anyhow::Result<()> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        *self.0.lock().unwrap().entry(name.to_owned()).or_default() += 1;
        Ok(())
    }
}

/// A task that never ends: its instance stops while it runs.
struct Hanging;

impl Runner for Hanging {
    async fn run(&self, _: &str) -> anyhow::Result<()> {
        std::future::pending().await
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn three_schedulers_run_each_due_task_once(pool: PgPool) {
    let counter = Counter::default();
    let schedulers: Vec<Scheduler> = (0..3)
        .map(|_| Scheduler::new(pool.clone(), Uuid::now_v7()))
        .collect();
    let round = |scheduler: &Scheduler| {
        let (scheduler, counter) = (scheduler.clone(), &counter);
        async move {
            for _ in 0..4 {
                scheduler.tick(counter).await;
            }
        }
    };
    tokio::join!(
        round(&schedulers[0]),
        round(&schedulers[1]),
        round(&schedulers[2])
    );
    let runs = counter.runs();
    assert_eq!(
        runs,
        SEEDED.iter().map(|n| ((*n).to_owned(), 1)).collect(),
        "each due task ran once"
    );
    // Rescheduled one period later, freed: nothing is due any more.
    for scheduler in &schedulers {
        assert_eq!(scheduler.tick(&counter).await, None);
    }
    let held: i64 = sqlx::query_scalar("SELECT count(*) FROM job WHERE holder IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(held, 0);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_stopped_holder_is_relieved_after_its_lease(pool: PgPool) {
    sqlx::query("UPDATE job SET due_at = now() + interval '1 day' WHERE name <> 'expire-pending'")
        .execute(&pool)
        .await
        .unwrap();
    let lease = Duration::from_secs(1);
    let holder =
        Scheduler::new(pool.clone(), Uuid::now_v7()).with_lease(lease, Duration::from_millis(200));
    let other =
        Scheduler::new(pool.clone(), Uuid::now_v7()).with_lease(lease, Duration::from_millis(200));
    // The holder renews its lease while it runs: the other instance waits.
    let running = tokio::spawn(async move { holder.tick(&Hanging).await });
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    let counter = Counter::default();
    assert_eq!(other.tick(&counter).await, None, "the lease is renewed");
    // The holder stops: past its lease, the task runs on the other instance.
    running.abort();
    assert_eq!(other.tick(&counter).await, None, "the lease still runs");
    tokio::time::sleep(lease + Duration::from_millis(200)).await;
    assert_eq!(
        other.tick(&counter).await.as_deref(),
        Some("expire-pending")
    );
    assert_eq!(counter.runs().get("expire-pending"), Some(&1));
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_tasks_of_kartenn_run(pool: PgPool) {
    let tasks = Tasks {
        store: kartenn_store_pg::write::PgStore::new(pool.clone()),
        sources: kartenn_service::expected::Sources::none(),
    };
    for name in SEEDED {
        tasks
            .run(name)
            .await
            .unwrap_or_else(|e| panic!("{name}: {e:#}"));
    }
    assert!(tasks.run("unknown").await.is_err());
}
