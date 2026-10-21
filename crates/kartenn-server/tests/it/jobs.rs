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

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
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

/// A person granted everything everywhere.
async fn importer(pool: &PgPool) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'connector') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, label, valid_period, changeset_id)
         VALUES ($1, 'importer', 'r', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
         VALUES ($1, '*', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'identity', $1, $2, 'global', NULL,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(identity)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    identity
}

/// A domain, a network and `count` addresses of it, deposited in chunks of 5 000 and
/// started: its task is due.
async fn started_import(pool: &PgPool, count: u32) -> Uuid {
    use kartenn_core::objects::{AddressUsage, Nature};
    use kartenn_service::port::ImportItem;
    use kartenn_service::{Operation, Ref};

    let store = kartenn_store_pg::write::PgStore::new(pool.clone());
    let author = importer(pool).await;
    let import = kartenn_service::import::open_import(&store, author, "legacy")
        .await
        .unwrap();
    let item = |external: String, operation| ImportItem {
        external_id: Some(external),
        operation,
    };
    let mut elements = vec![
        item(
            "lab".into(),
            Operation::CreateDomain {
                local: Some(0),
                slug: "lab".into(),
            },
        ),
        item(
            "net".into(),
            Operation::CreatePrefix {
                local: Some(1),
                domain: Ref::Local(0),
                prefix: "10.0.0.0/16".into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
        ),
    ];
    elements.extend((0..count).map(|i| {
        item(
            format!("addr-{i}"),
            Operation::CreateAddress {
                local: Some(2 + i),
                domain: Ref::Local(0),
                address: format!("10.0.{}.{}", i / 250, i % 250 + 1),
                usage: AddressUsage::Host,
            },
        )
    }));
    for (n, chunk) in elements.chunks(5_000).enumerate() {
        let digest = [u8::try_from(n).unwrap()];
        let deposit = kartenn_store_pg::imports::deposit(
            pool,
            import,
            i32::try_from(n).unwrap(),
            &digest,
            chunk,
        )
        .await
        .unwrap();
        assert_eq!(deposit, kartenn_store_pg::imports::Deposit::Created);
    }
    let start = kartenn_store_pg::imports::start(pool, import, |items| {
        kartenn_service::import::batches(items, kartenn_service::pipeline::MAX_OPERATIONS)
    })
    .await
    .unwrap();
    assert!(matches!(start, kartenn_store_pg::imports::Start::Started));
    import
}

async fn count(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn an_import_ends_on_another_instance_without_duplicates(pool: PgPool) {
    // Only the import is due.
    sqlx::query("UPDATE job SET due_at = now() + interval '1 day'")
        .execute(&pool)
        .await
        .unwrap();
    let addresses = 20_000;
    let import = started_import(&pool, addresses).await;
    let store = kartenn_store_pg::write::PgStore::new(pool.clone());
    let progress = || async {
        kartenn_service::import::import_status(&store, import)
            .await
            .unwrap()
            .unwrap()
    };
    let batches = progress().await.remaining;
    assert!(batches >= 4, "{batches}");

    // REQ-006-18: the instance stops after its first batch, during the next.
    let lease = Duration::from_secs(1);
    let heartbeat = Duration::from_millis(200);
    let holder = Scheduler::new(pool.clone(), Uuid::now_v7()).with_lease(lease, heartbeat);
    let tasks = Tasks {
        store: store.clone(),
        sources: kartenn_service::expected::Sources::none(),
    };
    let running = tokio::spawn(async move { holder.tick(&tasks).await });
    while progress().await.applied == 0 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    running.abort();
    let stopped = progress().await;
    assert_eq!(stopped.state, "running");
    assert!(stopped.remaining > 0, "{stopped:?}");

    // Past the lease, another instance takes the import over at its first batch left.
    tokio::time::sleep(lease + Duration::from_millis(200)).await;
    let other = Scheduler::new(pool.clone(), Uuid::now_v7()).with_lease(lease, heartbeat);
    let tasks = Tasks {
        store: store.clone(),
        sources: kartenn_service::expected::Sources::none(),
    };
    let name = format!("import:{import}");
    assert_eq!(other.tick(&tasks).await, Some(name.clone()));
    let done = progress().await;
    assert_eq!(done.state, "done", "{done:?}");
    assert_eq!((done.applied, done.remaining), (batches, 0));
    assert_eq!(count(&pool, "address").await, i64::from(addresses));
    assert_eq!(count(&pool, "external_id").await, i64::from(addresses) + 2);
    // Over: its task is gone.
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM job WHERE name = $1")
        .bind(&name)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0);
}

/// Runs of the background task `ticker.tick` (REQ-016-16).
static TICKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// A module with one background task and nothing else.
#[derive(Debug)]
struct Ticker;

#[derive(Debug)]
struct TickerHandler;

impl kartenn_service::module::ModuleHandler for TickerHandler {
    fn plan(
        &self,
        _: &kartenn_service::operations::ModuleOperation,
        _: Uuid,
        _: &dyn Fn(kartenn_service::operations::Ref) -> Option<Uuid>,
    ) -> Result<kartenn_service::module::ModulePlan, kartenn_core::rules::Violation> {
        unreachable!("the module has no operation")
    }

    fn apply<'a>(
        &'a self,
        _: &'a mut dyn kartenn_service::module::ModuleTx,
        _: &'a kartenn_service::module::ModulePlan,
        _: Option<kartenn_core::period::Period>,
        _: Uuid,
    ) -> kartenn_service::module::BoxFuture<'a, Result<(), kartenn_service::port::WriteError>> {
        unreachable!("the module has no operation")
    }

    fn run_job<'a>(
        &'a self,
        name: &'a str,
        tx: &'a mut dyn kartenn_service::module::ModuleTx,
    ) -> kartenn_service::module::BoxFuture<'a, Result<(), kartenn_service::port::StoreError>> {
        Box::pin(async move {
            assert_eq!(name, "tick");
            // In a transaction of its own, on the connection of the adapter.
            let conn = tx
                .connection()
                .downcast_mut::<sqlx::PgConnection>()
                .expect("a PostgreSQL connection");
            sqlx::query("SELECT 1").execute(conn).await.unwrap();
            TICKS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
    }
}

impl kartenn_service::module::Module for Ticker {
    fn name(&self) -> &'static str {
        "ticker"
    }
    fn version(&self) -> &'static str {
        "0.0.0"
    }
    fn jobs(&self) -> &'static [kartenn_service::module::JobSpec] {
        const JOBS: &[kartenn_service::module::JobSpec] = &[kartenn_service::module::JobSpec {
            name: "tick",
            period: Duration::from_secs(3_600),
        }];
        JOBS
    }
    fn handler(&self) -> Option<std::sync::Arc<dyn kartenn_service::module::ModuleHandler>> {
        Some(std::sync::Arc::new(TickerHandler))
    }
}

#[kartenn_testing::pg_test]
async fn a_module_job_runs_once_per_due_time_across_instances(pool: PgPool) {
    // Only the task of the module is due.
    sqlx::query("UPDATE job SET due_at = now() + interval '1 day'")
        .execute(&pool)
        .await
        .unwrap();
    let registry = std::sync::Arc::new(
        kartenn_service::module::ModuleRegistry::new(vec![std::sync::Arc::new(Ticker)]).unwrap(),
    );
    kartenn_store_pg::jobs::declare_module_jobs(&pool, &registry.jobs())
        .await
        .unwrap();
    let tasks = Tasks {
        store: kartenn_store_pg::write::PgStore::new(pool.clone()).with_modules(registry),
        sources: kartenn_service::expected::Sources::none(),
    };
    let schedulers: Vec<Scheduler> = (0..3)
        .map(|_| Scheduler::new(pool.clone(), Uuid::now_v7()))
        .collect();
    let round = |scheduler: &Scheduler| {
        let (scheduler, tasks) = (scheduler.clone(), &tasks);
        async move {
            for _ in 0..3 {
                scheduler.tick(tasks).await;
            }
        }
    };
    tokio::join!(
        round(&schedulers[0]),
        round(&schedulers[1]),
        round(&schedulers[2])
    );
    assert_eq!(
        TICKS.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "one run"
    );
    let (period, module): (f64, Option<String>) = sqlx::query_as(
        "SELECT extract(epoch FROM period)::float8, module FROM job WHERE name = 'ticker.tick'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((period, module.as_deref()), (3_600.0, Some("ticker")));
    // Due again: one more run.
    kartenn_store_pg::jobs::due_now(&pool, "ticker.tick")
        .await
        .unwrap();
    assert_eq!(
        schedulers[1].tick(&tasks).await.as_deref(),
        Some("ticker.tick")
    );
    assert_eq!(TICKS.load(std::sync::atomic::Ordering::SeqCst), 2);
    // The build no longer declares it: disabled, kept, never claimed again.
    kartenn_store_pg::jobs::declare_module_jobs(&pool, &[])
        .await
        .unwrap();
    kartenn_store_pg::jobs::due_now(&pool, "ticker.tick")
        .await
        .unwrap();
    assert_eq!(schedulers[2].tick(&tasks).await, None);
    let enabled: bool = sqlx::query_scalar("SELECT enabled FROM job WHERE name = 'ticker.tick'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!enabled, "a retired task is disabled, not deleted");
    assert_eq!(TICKS.load(std::sync::atomic::Ordering::SeqCst), 2);
}
