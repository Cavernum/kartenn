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

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
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
