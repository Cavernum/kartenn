//! Budgets of the core (spec 002, REQ-002-45; spec 004, REQ-004-38; tasks 002-27,
//! 004-23) on the reference dataset with three years of history: reading an object now
//! and as known in the past, a filtered page, a subtree with its occupancy (now and as
//! known in the past, task 006-12d), a unitary changeset, a sustained rate of unitary
//! changesets and an import of as many addresses as the dataset holds, through the real
//! service and storage adapter; then those of the VLANs of the core (spec 026, REQ-026-7;
//! task 008-16).

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::import::{create_import, run_import};
use kartenn_service::operations::{DeclarationInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::ImportItem;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::filter::{Condition, filter};
use kartenn_store_pg::rbac::ReadScope;
use kartenn_store_pg::read::object;
use kartenn_store_pg::tree::{
    children, children_as_of, children_of, next_free_address, occupancy, occupancy_as_of,
    prefix_as_of, roots_of,
};
use kartenn_store_pg::write::PgStore;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::Dataset;
use crate::machine_budgets as machines;
use crate::measures::{Bench, Measure, Selection};
use crate::props::{Budget, SINCE, copy, load, p95, progress, verdict};
use crate::stats_budgets::STATS;
use crate::vlan_budgets as vlan;

const READ_OBJECT: &str = "core.read-object";
const HISTORY_INDEX: &str = "core.history-index";
const FILTERED_PAGE: &str = "core.filtered-page";
const SUBTREE: &str = "core.subtree";
const ROOTS_PAGE: &str = "core.roots-page";
const WIDE_CHILDREN: &str = "core.wide-children";
const SUBTREE_PAST: &str = "core.subtree-past";
const UNITARY: &str = "core.unitary-changeset";
const SUSTAINED_RATE: &str = "core.sustained";
const IMPORT: &str = "core.import";

/// The measures of `bench-core` (ADR 0041): those of the core, then those of the VLANs,
/// of the machines (whose owners are the entities of the VLANs) and of the statistics,
/// each module set up when one of its measures runs. The statistics count what the large
/// writes before them added.
pub(crate) static BENCH: Bench = Bench {
    name: "core",
    measures: &[
        Measure::new(READ_OBJECT),
        Measure::new(HISTORY_INDEX),
        Measure::new(FILTERED_PAGE),
        Measure::new(SUBTREE),
        Measure::new(ROOTS_PAGE),
        Measure::new(WIDE_CHILDREN),
        Measure::new(SUBTREE_PAST),
        Measure::new(UNITARY),
        Measure::new(SUSTAINED_RATE),
        Measure::new(IMPORT),
        Measure::new(vlan::READ),
        Measure::new(vlan::FILTERED_PAGE),
        Measure::new(vlan::OCCUPANCY),
        Measure::new(vlan::CREATE),
        Measure::new(machines::READ),
        Measure::new(machines::OWNER_PAGE),
        Measure::new(machines::SITE_PAGE),
        Measure::new(machines::NETWORK),
        Measure::new(machines::CREATE),
        Measure::new(machines::SUGGESTIONS),
        Measure::new(machines::IMPORT),
        Measure::new(STATS).after(&[WIDE_CHILDREN, IMPORT, machines::IMPORT]),
    ],
};

/// Start of the three years of history.
const HISTORY_FROM: &str = "2023-10-01";
/// Writers of the sustained rate, and how long they write.
const WRITERS: usize = 8;
const SUSTAINED: Duration = Duration::from_secs(20);
const DAY: Duration = Duration::from_hours(24);

/// Two past versions of every prefix and address, asserted over three years and spread
/// over the months: the first until a day of the first year, the second until the load.
async fn history(pool: &PgPool) -> anyhow::Result<()> {
    for (table, columns) in [
        (
            "prefix",
            "id, domain_id, prefix, nature, point_to_point, anycast, public, registry_version, valid_period, changeset_id",
        ),
        (
            "address",
            "id, domain_id, address, usage, public, registry_version, valid_period, changeset_id",
        ),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CALL kartenn_history_partitions('{table}', '{HISTORY_FROM}', 40)"
        )))
        .execute(pool)
        .await?;
        let split = format!(
            "'{HISTORY_FROM}'::timestamptz + (abs(hashtext(id::text)) % 365 + 1) * interval '1 day'"
        );
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO {table}_history ({columns}, sys_start, sys_end)
             SELECT {columns}, '{HISTORY_FROM}'::timestamptz - interval '30 days', {split}
             FROM {table}"
        )))
        .execute(pool)
        .await?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO {table}_history ({columns}, sys_start, sys_end)
             SELECT {columns}, {split}, sys_start FROM {table}"
        )))
        .execute(pool)
        .await?;
    }
    sqlx::raw_sql("ANALYZE").execute(pool).await?;
    Ok(())
}

/// Whether the plan of `explain` (bound to `at`) reads the system period index of a
/// history; with `forced`, sequential scans are disabled to tell whether it can.
async fn uses_index(
    pool: &PgPool,
    explain: &str,
    at: OffsetDateTime,
    forced: bool,
) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    if forced {
        sqlx::raw_sql("SET LOCAL enable_seqscan = off")
            .execute(&mut *tx)
            .await?;
    }
    let plan: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(explain.to_owned()))
        .bind(at)
        .fetch_all(&mut *tx)
        .await?;
    tx.rollback().await?;
    // On a partition, the index inherited from `<history>_system_period` is named after
    // the expression.
    Ok(plan
        .iter()
        .any(|l| l.contains("system_period") || l.contains("_tstzrange_idx")))
}

/// A unitary changeset of `author`: its duration, or why it was not applied.
pub(crate) async fn unitary(
    store: &PgStore,
    author: Uuid,
    operation: Operation,
) -> anyhow::Result<Duration> {
    let request = ChangesetRequest {
        author,
        motive: "bench".into(),
        period: None,
        operations: vec![operation],
        simulate: false,
        idempotency_key: None,
        approval_self: true,
    };
    let started = Instant::now();
    let outcome = submit(store, &Unrestricted, &request, MAX_OPERATIONS).await?;
    let elapsed = started.elapsed();
    match outcome {
        Outcome::Applied { .. } => Ok(elapsed),
        other => bail!("changeset not applied: {other:?}"),
    }
}

/// A new address in `network`: its first free address now.
pub(crate) async fn new_address(pool: &PgPool, network: (Uuid, Uuid)) -> anyhow::Result<Operation> {
    let free = next_free_address(pool, network.0, OffsetDateTime::now_utc())
        .await?
        .context("a network of the dataset is full")?;
    Ok(Operation::CreateAddress {
        local: None,
        domain: Ref::Id(network.1),
        address: free.to_string(),
        usage: AddressUsage::Host,
    })
}

/// `count` addresses in a new domain, with their networks (a `/22` per thousand) under
/// one container, each known to the source by an identifier.
fn import_items(count: usize) -> Vec<ImportItem> {
    let item = |external: String, operation| ImportItem {
        external_id: Some(external),
        operation,
    };
    let mut items = vec![
        item(
            "bulk".into(),
            Operation::CreateDomain {
                local: Some(1),
                slug: "bulk".into(),
            },
        ),
        item(
            "root".into(),
            Operation::CreatePrefix {
                local: Some(2),
                domain: Ref::Local(1),
                prefix: "10.0.0.0/8".into(),
                nature: Nature::Container,
                point_to_point: false,
                anycast: false,
            },
        ),
    ];
    let mut local = 2;
    for n in 0..count.div_ceil(1_000) {
        let base = 0x0a00_0000_u32 + (u32::try_from(n).unwrap_or(0) << 10);
        local += 1;
        items.push(item(
            format!("net-{n}"),
            Operation::CreatePrefix {
                local: Some(local),
                domain: Ref::Local(1),
                prefix: format!("{}/22", std::net::Ipv4Addr::from(base)),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
        ));
        for h in 1..=1_000.min(count - n * 1_000) {
            local += 1;
            items.push(item(
                format!("addr-{n}-{h}"),
                Operation::CreateAddress {
                    local: Some(local),
                    domain: Ref::Local(1),
                    address: std::net::Ipv4Addr::from(base + u32::try_from(h).unwrap_or(0))
                        .to_string(),
                    usage: AddressUsage::Host,
                },
            ));
        }
    }
    items
}

/// A domain `slug` whose container `root` has `children` direct networks `/30`, written
/// as the dataset is (spec 007, REQ-007-39 and 32); returns the domain and the container.
///
/// # Errors
///
/// Database failures.
pub(crate) async fn wide_container(
    pool: &PgPool,
    slug: &str,
    root: &str,
    children: i32,
) -> anyhow::Result<(Uuid, Uuid)> {
    let (domain, container): (Uuid, Uuid) = sqlx::query_as(
        "WITH d AS (
           INSERT INTO addressing_domain (id, slug, valid_period, changeset_id)
           VALUES (gen_random_uuid(), $1, tstzrange('2026-01-01', NULL), gen_random_uuid())
           RETURNING id),
         c AS (
           INSERT INTO prefix (id, domain_id, prefix, nature, public, registry_version,
                               valid_period, changeset_id)
           SELECT gen_random_uuid(), d.id, $2::cidr, 'container', false,
                  '2025-10-09', tstzrange('2026-01-01', NULL), gen_random_uuid()
           FROM d RETURNING id, domain_id),
         n AS (
           INSERT INTO prefix (id, domain_id, prefix, nature, public, registry_version,
                               valid_period, changeset_id)
           SELECT gen_random_uuid(), d.id,
                  set_masklen(host($2::cidr)::inet + 4 * i, 30)::cidr, 'network', false,
                  '2025-10-09', tstzrange('2026-01-01', NULL), gen_random_uuid()
           FROM d, generate_series(0, $3 - 1) i)
         SELECT domain_id, id FROM c",
    )
    .bind(slug)
    .bind(root)
    .bind(children)
    .fetch_one(pool)
    .await?;
    sqlx::query("ANALYZE prefix").execute(pool).await?;
    Ok((domain, container))
}

/// Loads the dataset of `bench-core` into the database at `database`, already migrated by
/// `kartenn-server migrate`, and a domain `bench-web` whose container has 10⁵ networks
/// (spec 007, REQ-007-32: the budgets of the browser); returns the line
/// `domain=<id> container=<id>` for the browser bench.
///
/// # Errors
///
/// Database or pipeline failures.
pub async fn seed(database: &str, data: &Dataset, seed: u64) -> anyhow::Result<String> {
    let Prepared { pool, .. } = prepare(database, data, seed).await?;
    let (domain, container) = wide_container(&pool, "bench-web", "100.64.0.0/13", 100_000).await?;
    sqlx::raw_sql("VACUUM ANALYZE prefix")
        .execute(&pool)
        .await?;
    Ok(format!("domain={domain} container={container}\n"))
}

/// The prepared database: the dataset loaded with three years of history, an author, and
/// the integer property `custom.rack` on 20 % of the addresses, vacuumed.
pub(crate) struct Prepared {
    pub(crate) pool: PgPool,
    pub(crate) author: Uuid,
    pub(crate) rack: Uuid,
    /// Lines for the report.
    pub(crate) report: String,
}

/// Loads `data` into the empty database at `database` and prepares it for the measures.
///
/// # Errors
///
/// Database or pipeline failures.
pub(crate) async fn prepare(database: &str, data: &Dataset, seed: u64) -> anyhow::Result<Prepared> {
    let pool = PgPool::connect(database)
        .await
        .with_context(|| format!("connecting to {database}"))?;
    kartenn_store_pg::MIGRATOR.run(&pool).await?;
    let started = Instant::now();
    load(&pool, data).await?;
    progress(&format!("load: {:?}", started.elapsed()));
    history(&pool).await?;
    progress(&format!("history: {:?}", started.elapsed()));
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'bench') RETURNING id",
    )
    .fetch_one(&pool)
    .await?;
    let store = PgStore::new(pool.clone());
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut report = String::new();

    // A non-heritable integer on 20 % of the addresses, for the filtered pages.
    unitary(
        &store,
        author,
        Operation::DeclareProperty {
            local: None,
            declaration: DeclarationInput {
                key: "custom.rack".into(),
                value_type: TypeInput::Int {
                    min: 0,
                    max: 1_000_000,
                },
                cardinality: Cardinality::Single,
                carriers: vec!["address".into()],
                inherit: None,
                module: None,
                obligations: Vec::new(),
                expected: None,
                labels: None,
            },
        },
    )
    .await?;
    let rack: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'custom.rack'")
            .fetch_one(&pool)
            .await?;
    let mut rows = String::new();
    for a in &data.addresses {
        if rng.random_range(0..100) < 20 {
            writeln!(
                rows,
                "{}\taddress\t{rack}\tvalue\t{}\tinput\t{SINCE}\t{}",
                a.id,
                rng.random_range(0..1_000_000),
                Uuid::nil()
            )?;
        }
    }
    copy(
        &pool,
        "COPY property_assignment (object_id, object_type, property_id, kind, v_int, provenance, valid_period, changeset_id) FROM STDIN",
        rows,
    )
    .await?;
    // Measured in a steady state: no checkpoint of the bulk writes under way.
    // CHECKPOINT needs the role pg_checkpoint; without it, VACUUM has written most pages.
    sqlx::raw_sql("VACUUM ANALYZE").execute(&pool).await?;
    let checkpoint = sqlx::raw_sql("CHECKPOINT").execute(&pool).await.is_ok();
    writeln!(report, "INFO checkpoint before the measures: {checkpoint}")?;
    progress(&format!("assignments: {:?}", started.elapsed()));
    Ok(Prepared {
        pool,
        author,
        rack,
        report,
    })
}

/// Loads, writes the history and measures what `selection` runs; returns the report and
/// whether every budget holds.
///
/// # Errors
///
/// Database or pipeline failures.
#[allow(clippy::too_many_lines)] // one linear scenario, step by step
pub async fn run(
    database: &str,
    data: &Dataset,
    seed: u64,
    selection: &Selection,
) -> anyhow::Result<(String, bool)> {
    let started = Instant::now();
    let Prepared {
        pool,
        author,
        rack,
        mut report,
    } = prepare(database, data, seed).await?;
    let store = PgStore::new(pool.clone());
    // Draws distinct from those of the preparation, in the order of the measures that run.
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(1));
    let mut budgets = Vec::new();
    let mut ok = true;

    // Reading an object, now and as known at a past date (REQ-004-38: at most twice).
    let now = OffsetDateTime::now_utc();
    // HISTORY_FROM, 2023-10-01T00:00Z.
    let from = OffsetDateTime::from_unix_timestamp(1_696_118_400)?;
    if selection.runs(READ_OBJECT) {
        // Independent draws for both reads (the same object read twice would be in cache
        // the second time), after a warm-up of the connections and statements.
        let draw = |rng: &mut ChaCha8Rng| {
            if rng.random_range(0..4) == 0 {
                data.prefixes[rng.random_range(0..data.prefixes.len())].id
            } else {
                data.addresses[rng.random_range(0..data.addresses.len())].id
            }
        };
        for i in 0..20 {
            let as_of = (i % 2 == 0).then(|| from + DAY * 100);
            object(&pool, draw(&mut rng), now, as_of).await?;
        }
        let mut current = Vec::new();
        let mut past = Vec::new();
        for _ in 0..200 {
            let t = Instant::now();
            object(&pool, draw(&mut rng), now, None)
                .await?
                .context("a loaded object is missing")?;
            current.push(t.elapsed());
            let as_of = from + DAY * rng.random_range(0..700);
            let t = Instant::now();
            object(&pool, draw(&mut rng), now, Some(as_of))
                .await?
                .context("a past version is missing")?;
            past.push(t.elapsed());
        }
        let current = p95(current);
        let past = p95(past);
        budgets.push(Budget {
            name: "read an object, p95",
            limit: Duration::from_millis(50),
            measured: current,
            note: "200 draws, with its properties".into(),
        });
        budgets.push(Budget {
            name: "read an object as known in the past, p95",
            limit: (current * 2).max(Duration::from_millis(5)),
            measured: past,
            note: "limit: twice the current read (floor 5 ms)".into(),
        });
    }

    // The versions asserted over a day, from the system period index (004-25): usable
    // (blocking), and chosen by the planner (depends on the volume, reported).
    if selection.runs(HISTORY_INDEX) {
        let day = from + DAY * 400;
        let over_a_day = "SELECT count(*) FROM address_history
                          WHERE tstzrange(sys_start, sys_end) && tstzrange($1, $1 + interval '1 day')";
        let explain = format!("EXPLAIN {over_a_day}");
        let chosen = uses_index(&pool, &explain, day, false).await?;
        let usable = uses_index(&pool, &explain, day, true).await?;
        let t = Instant::now();
        let _: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(over_a_day))
            .bind(day)
            .fetch_one(&pool)
            .await?;
        writeln!(
            report,
            "{} system period index of the history: usable {usable}, chosen {chosen} ({:?} for a day)",
            if usable { "PASS" } else { "FAIL" },
            t.elapsed()
        )?;
        ok &= usable;
    }

    // Filtered pages of 100.
    if selection.runs(FILTERED_PAGE) {
        let mut samples = Vec::new();
        for _ in 0..100 {
            let domain = data.domains[rng.random_range(0..data.domains.len())].id;
            let low: i64 = rng.random_range(0..900_000);
            let condition =
                Condition::Between(Some(low.to_string()), Some((low + 100_000).to_string()));
            let t = Instant::now();
            filter(&pool, rack, domain, &condition, now, None, 100).await?;
            samples.push(t.elapsed());
        }
        budgets.push(Budget {
            name: "filtered page of 100, p95",
            limit: Duration::from_millis(150),
            measured: p95(samples),
            note: "100 draws".into(),
        });
    }

    // Subtree (direct content) and occupancy of a prefix.
    if selection.runs(SUBTREE) {
        let mut samples = Vec::new();
        for _ in 0..200 {
            let p = &data.prefixes[rng.random_range(0..data.prefixes.len())];
            let t = Instant::now();
            children(&pool, p.id, now).await?;
            occupancy(&pool, p.id, now)
                .await?
                .context("a loaded prefix is missing")?;
            samples.push(t.elapsed());
        }
        budgets.push(Budget {
            name: "subtree and occupancy, p95",
            limit: Duration::from_millis(200),
            measured: p95(samples),
            note: "200 draws, containers and networks".into(),
        });
    }

    // The first page of 100 roots of a domain (spec 007, REQ-007-39).
    if selection.runs(ROOTS_PAGE) {
        let mut samples = Vec::new();
        for _ in 0..100 {
            let domain = data.domains[rng.random_range(0..data.domains.len())].id;
            let t = Instant::now();
            roots_of(&pool, domain, now, None, &ReadScope::all(), None, Some(101)).await?;
            samples.push(t.elapsed());
        }
        budgets.push(Budget {
            name: "roots of a domain, page of 100, p95",
            limit: Duration::from_millis(150),
            measured: p95(samples),
            note: "100 draws".into(),
        });
    }

    // A page of 100 children of a container with 10⁴ of them, after a cursor and as known
    // when the first page was read, as the API reads it (task 007-18): a domain of its own.
    if selection.runs(WIDE_CHILDREN) {
        let (_, wide) = wide_container(&pool, "bench-wide", "100.64.0.0/16", 10_000).await?;
        let parent = prefix_as_of(&pool, wide, now, None)
            .await?
            .context("the wide container is missing")?;
        let all = children_of(&pool, &parent, now, None, &ReadScope::all(), None, None).await?;
        let mut samples = Vec::new();
        for _ in 0..100 {
            let after = all[rng.random_range(0..99) * 100 + 99].key();
            let t = Instant::now();
            children_of(
                &pool,
                &parent,
                now,
                Some(now),
                &ReadScope::all(),
                Some(&after),
                Some(101),
            )
            .await?;
            samples.push(t.elapsed());
        }
        budgets.push(Budget {
            name: "children of a container of 10^4, page of 100, p95",
            limit: Duration::from_millis(150),
            measured: p95(samples),
            note: format!("100 draws among {} children, after a cursor", all.len()),
        });
    }

    // The same as known in the past (task 006-12d): the prefix read first, its domain and
    // value given to the queries, so that the history keeps its per-domain index.
    if selection.runs(SUBTREE_PAST) {
        let mut samples = Vec::new();
        for _ in 0..200 {
            let p = &data.prefixes[rng.random_range(0..data.prefixes.len())];
            let as_of = Some(from + DAY * rng.random_range(0..700));
            let t = Instant::now();
            children_as_of(&pool, p.id, now, as_of, &ReadScope::all()).await?;
            occupancy_as_of(&pool, p.id, now, as_of)
                .await?
                .context("a past prefix is missing")?;
            samples.push(t.elapsed());
        }
        budgets.push(Budget {
            name: "subtree and occupancy in the past, p95",
            limit: Duration::from_millis(200),
            measured: p95(samples),
            note: "200 draws over two years of history".into(),
        });
    }

    // Unitary changesets: a new address in a random network.
    let networks: Vec<(Uuid, Uuid)> = data
        .prefixes
        .iter()
        .filter(|p| p.nature == crate::Nature::Network)
        .map(|p| (p.id, data.domains[p.domain].id))
        .collect();
    if selection.runs(UNITARY) {
        let mut samples = Vec::new();
        for _ in 0..100 {
            let network = networks[rng.random_range(0..networks.len())];
            let operation = new_address(&pool, network).await?;
            samples.push(unitary(&store, author, operation).await?);
        }
        budgets.push(Budget {
            name: "unitary changeset, p95",
            limit: Duration::from_millis(200),
            measured: p95(samples),
            note: "100 new addresses, validated and applied".into(),
        });
    }
    progress(&format!(
        "reads and unitary changesets: {:?}",
        started.elapsed()
    ));

    if selection.runs(SUSTAINED_RATE) {
        budgets.push(sustained(&pool, &store, author, &networks).await?);
        progress(&format!("sustained rate: {:?}", started.elapsed()));
    }

    // Import of as many addresses as the dataset holds, in batches.
    if selection.runs(IMPORT) {
        let count = data.addresses.len();
        let items = import_items(count);
        let t = Instant::now();
        let import = create_import(&store, author, "bench", items, MAX_OPERATIONS).await?;
        let status = run_import(&store, &Unrestricted, import, MAX_OPERATIONS)
            .await?
            .context("the import vanished")?;
        let elapsed = t.elapsed();
        if status.state != "done" {
            bail!("import not done: {status:?}");
        }
        budgets.push(Budget {
            name: "import",
            limit: Duration::from_mins(15),
            measured: elapsed,
            note: format!("{count} addresses, {} batches", status.applied),
        });
        progress(&format!("import: {:?}", started.elapsed()));
    }

    // The VLANs of the core on the same dataset (spec 026, REQ-026-7; REQ-008-25), whose
    // entities own the machines; the machines module beside it (spec 009, REQ-009-21);
    // the counters of the home page over both (spec 024, REQ-024-16).
    let statistics = selection.runs(STATS);
    if selection.any("core.vlan-") || selection.any("core.machines-") || statistics {
        let run = vlan::run(&pool, author, data, &mut rng, selection, &mut report);
        budgets.extend(run.await?);
    }
    if selection.any("core.machines-") || statistics {
        let run = machines::run(&pool, author, data, &mut rng, selection, &mut report);
        budgets.extend(run.await?);
    }
    if statistics {
        budgets.extend(crate::stats_budgets::run(&pool, data, seed, &mut report).await?);
    }

    let (lines, all) = verdict(&budgets)?;
    report.push_str(&lines);
    Ok((report, ok && all))
}

/// Sustained rate: writers on disjoint networks for a while.
async fn sustained(
    pool: &PgPool,
    store: &PgStore,
    author: Uuid,
    networks: &[(Uuid, Uuid)],
) -> anyhow::Result<Budget> {
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
    let count = done.load(Ordering::Relaxed);
    let rate = f64::from(u32::try_from(count)?) / begun.elapsed().as_secs_f64();
    Ok(Budget {
        name: "sustained unitary changesets",
        // As a time per changeset: 50 per second at least.
        limit: Duration::from_millis(20),
        measured: begun.elapsed() / u32::try_from(count.max(1))?,
        note: format!("{rate:.1} changesets/s, {WRITERS} writers, {count} in {SUSTAINED:?}"),
    })
}
