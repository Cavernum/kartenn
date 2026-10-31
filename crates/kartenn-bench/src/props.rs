//! Budgets of the properties (spec 003, REQ-003-26; task 003-16) on the reference dataset
//! of spec 002: loads it into an empty database, declares 10 properties (3 heritable),
//! assigns them, then measures through the real service and storage adapter.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use kartenn_core::values::Cardinality;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::DiffCount;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::explain::{explain, resolved_many};
use kartenn_store_pg::filter::{Condition, filter};
use kartenn_store_pg::write::PgStore;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sqlx::PgPool;
use sqlx::postgres::PgPoolCopyExt;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::Dataset;
use crate::measures::{self, Measure, Selection};

const ORIGINS_EVERY_ROOT: &str = "props.origins-every-root";
const RESOLUTION: &str = "props.explained-resolution";
const GROUPED: &str = "props.grouped-resolution";
const FILTERED: &str = "props.filtered-pages";
const CHANGESET: &str = "props.changeset-10k";
const VALUE_EVERY_ROOT: &str = "props.value-every-root";
const ORIGINS_3_DOMAINS: &str = "props.origins-3-domains";

/// The measures of `bench-properties` (ADR 0041). The writes run in turn on the tables the
/// previous one left, as in the whole bench.
pub(crate) static BENCH: measures::Bench = measures::Bench {
    name: "props",
    measures: &[
        // The first heritable property assigned on every root, by the preparation.
        Measure::new(ORIGINS_EVERY_ROOT).preparation(),
        Measure::new(RESOLUTION),
        Measure::new(GROUPED),
        Measure::new(FILTERED),
        Measure::new(CHANGESET),
        Measure::new(VALUE_EVERY_ROOT).after(&[CHANGESET]),
        Measure::new(ORIGINS_3_DOMAINS).after(&[VALUE_EVERY_ROOT]),
    ],
};

/// Validity of the loaded objects.
pub(crate) const SINCE: &str = "[\"2026-01-01 00:00:00+00\",)";
/// Heritable properties, then non-heritable ones.
const HERITABLE: [&str; 3] = ["custom.h1", "custom.h2", "custom.h3"];
const PLAIN: [&str; 7] = [
    "custom.n1",
    "custom.n2",
    "custom.n3",
    "custom.n4",
    "custom.n5",
    "custom.n6",
    "custom.n7",
];

/// One measured budget.
pub(crate) struct Budget {
    pub(crate) name: &'static str,
    pub(crate) limit: Duration,
    pub(crate) measured: Duration,
    pub(crate) note: String,
}

/// p95 of samples.
pub(crate) fn p95(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    let i = (samples.len() * 95).div_ceil(100).saturating_sub(1);
    samples.get(i).copied().unwrap_or_default()
}

pub(crate) async fn copy(pool: &PgPool, statement: &str, data: String) -> anyhow::Result<()> {
    let mut copy = pool.copy_in_raw(statement).await?;
    copy.send(data.into_bytes()).await?;
    copy.finish().await?;
    Ok(())
}

/// Loads domains, prefixes and addresses as objects valid since 2026 (ranges are not
/// needed by these budgets).
pub(crate) async fn load(pool: &PgPool, data: &Dataset) -> anyhow::Result<()> {
    let changeset = Uuid::now_v7();
    let mut domains = String::new();
    for d in &data.domains {
        writeln!(domains, "{}\t{}\t{SINCE}\t{changeset}", d.id, d.slug)?;
    }
    copy(
        pool,
        "COPY addressing_domain (id, slug, valid_period, changeset_id) FROM STDIN",
        domains,
    )
    .await?;
    let mut prefixes = String::new();
    for p in &data.prefixes {
        writeln!(
            prefixes,
            "{}\t{}\t{}\t{}\tf\t2025-10-09\t{SINCE}\t{changeset}",
            p.id,
            data.domains[p.domain].id,
            p.prefix,
            p.nature.as_str()
        )?;
    }
    copy(
        pool,
        "COPY prefix (id, domain_id, prefix, nature, public, registry_version, valid_period, changeset_id) FROM STDIN",
        prefixes,
    )
    .await?;
    let mut addresses = String::new();
    for a in &data.addresses {
        writeln!(
            addresses,
            "{}\t{}\t{}\thost\tf\t2025-10-09\t{SINCE}\t{changeset}",
            a.id, data.domains[a.domain].id, a.addr
        )?;
    }
    copy(
        pool,
        "COPY address (id, domain_id, address, usage, public, registry_version, valid_period, changeset_id) FROM STDIN",
        addresses,
    )
    .await?;
    sqlx::raw_sql("ANALYZE").execute(pool).await?;
    Ok(())
}

struct Bench {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
}

impl Bench {
    async fn submit(
        &self,
        operations: Vec<Operation>,
    ) -> anyhow::Result<(Duration, Vec<DiffCount>)> {
        let request = ChangesetRequest {
            author: self.author,
            motive: "bench".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        let started = Instant::now();
        let outcome = submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS).await?;
        let elapsed = started.elapsed();
        match outcome {
            Outcome::Applied { diff, .. } => Ok((elapsed, diff)),
            other => bail!("changeset not applied: {other:?}"),
        }
    }

    async fn property(&self, key: &str) -> anyhow::Result<Uuid> {
        Ok(
            sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = $1")
                .bind(key)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// Declares the 10 properties; values `v0`…`v9` for the heritable ones.
    async fn declare(&self) -> anyhow::Result<()> {
        let mut operations = Vec::new();
        for (i, key) in HERITABLE.iter().enumerate() {
            let local = u32::try_from(i + 1)?;
            operations.push(Operation::DeclareProperty {
                local: Some(local),
                declaration: input(
                    key,
                    TypeInput::ENUM,
                    Cardinality::Set,
                    &["prefix", "address"],
                    true,
                ),
            });
            for v in 0..10 {
                operations.push(Operation::AddEnumValue {
                    local: None,
                    property: Ref::Local(local),
                    key: format!("v{v}"),
                    // Labelled in the two languages shipped, as the dictionary serves
                    // them (spec 018).
                    labels: [
                        ("fr".to_owned(), format!("Valeur {v}")),
                        ("en".to_owned(), format!("Value {v}")),
                    ]
                    .into(),
                });
            }
        }
        for key in PLAIN {
            operations.push(Operation::DeclareProperty {
                local: None,
                declaration: input(
                    key,
                    TypeInput::Int {
                        min: 0,
                        max: 1_000_000,
                    },
                    Cardinality::Single,
                    &["address"],
                    false,
                ),
            });
        }
        self.submit(operations).await?;
        Ok(())
    }

    /// Prefixes that no other prefix of their domain contains.
    async fn roots(&self) -> anyhow::Result<Vec<(Uuid, Uuid)>> {
        Ok(sqlx::query_as(
            "SELECT p.id, p.domain_id FROM prefix p
             WHERE NOT EXISTS (SELECT 1 FROM prefix q
                               WHERE q.domain_id = p.domain_id AND q.prefix >> p.prefix)
             ORDER BY p.domain_id, p.prefix",
        )
        .fetch_all(&self.pool)
        .await?)
    }
}

fn input(
    key: &str,
    value_type: TypeInput,
    cardinality: Cardinality,
    carriers: &[&str],
    heritable: bool,
) -> DeclarationInput {
    DeclarationInput {
        key: key.into(),
        value_type,
        cardinality,
        carriers: carriers.iter().map(|c| (*c).to_owned()).collect(),
        inherit: heritable.then(|| "addressing".into()),
        module: None,
        obligations: Vec::new(),
        expected: None,
        labels: Some(
            [
                ("fr".to_owned(), key.to_owned()),
                ("en".to_owned(), key.to_owned()),
            ]
            .into(),
        ),
    }
}

fn assign(object: Uuid, property: Uuid, value: &str) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(object),
        kind: ObjectKind::Prefix,
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    }
}

fn touched(diff: &[DiffCount]) -> u64 {
    diff.iter().map(|d| d.count).sum()
}

/// Shows how far the run is (the reference run takes minutes).
pub(crate) fn progress(line: &str) {
    eprintln!("{line}");
}

/// The store of the bench. Below the reference dataset (10⁶ addresses), the number of
/// targets from which origins are recomputed going down shrinks with it, so that the large
/// changes of origin measured go down as at full scale: the reduced bench went up where
/// the full one went down, and missed the generic plan of the way down (task 003-20). The
/// setup steps do not all follow (its batches of 1 000 prefixes go down here, up at full
/// scale).
fn store(pool: &PgPool, data: &Dataset) -> PgStore {
    const REFERENCE: usize = 1_000_000;
    let store = PgStore::new(pool.clone());
    let addresses = data.addresses.len();
    if addresses >= REFERENCE / 2 {
        return store;
    }
    let down_from = store.down_from() * addresses / REFERENCE;
    store.with_down_from(down_from.max(1))
}

/// Loads, assigns and measures what `selection` runs; returns the report and whether every
/// budget holds.
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
    let pool = PgPool::connect(database)
        .await
        .with_context(|| format!("connecting to {database}"))?;
    kartenn_store_pg::MIGRATOR.run(&pool).await?;
    let mut report = String::new();
    let started = Instant::now();
    load(&pool, data).await?;
    progress(&format!("load: {:?}", started.elapsed()));
    let author = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'bench') RETURNING id",
    )
    .fetch_one(&pool)
    .await?;
    let bench = Bench {
        store: store(&pool, data),
        pool: pool.clone(),
        author,
    };
    bench.declare().await?;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let roots = bench.roots().await?;
    let mut budgets = Vec::new();

    // Heritable values: every root, then 3 % of the other prefixes. The first property
    // on all roots is the largest change of origin of the run (P10).
    for (i, key) in HERITABLE.iter().enumerate() {
        let property = bench.property(key).await?;
        let ops = roots
            .iter()
            .enumerate()
            .map(|(r, (id, _))| assign(*id, property, &format!("v{}", (r + i) % 10)))
            .collect();
        let (elapsed, diff) = bench.submit(ops).await?;
        if i == 0 && selection.runs(ORIGINS_EVERY_ROOT) {
            budgets.push(Budget {
                name: "origins of every root (P10)",
                limit: Duration::from_secs(60),
                measured: elapsed,
                note: format!("{} objects", touched(&diff)),
            });
        }
        let others: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM prefix WHERE abs(hashtext(id::text || $1)) % 100 < 3",
        )
        .bind(*key)
        .fetch_all(&pool)
        .await?;
        let others: Vec<Uuid> = others
            .into_iter()
            .filter(|o| !roots.iter().any(|(r, _)| r == o))
            .collect();
        for chunk in others.chunks(1_000) {
            let ops = chunk
                .iter()
                .map(|o| assign(*o, property, &format!("v{}", rng.random_range(0..10))))
                .collect();
            bench.submit(ops).await?;
        }
        eprintln!("{key}: {:?}", started.elapsed());
    }
    progress(&format!("heritable assignments: {:?}", started.elapsed()));

    // Non-heritable values: 20 % of the addresses each, written directly.
    for key in PLAIN {
        let property = bench.property(key).await?;
        let mut rows = String::new();
        for a in &data.addresses {
            if rng.random_range(0..100) >= 20 {
                continue;
            }
            writeln!(
                rows,
                "{}\taddress\t{property}\tvalue\t{}\tinput\t{SINCE}\t{}",
                a.id,
                rng.random_range(0..1_000_000),
                Uuid::nil()
            )?;
        }
        copy(
            &pool,
            "COPY property_assignment (object_id, object_type, property_id, kind, v_int, provenance, valid_period, changeset_id) FROM STDIN",
            rows,
        )
        .await?;
    }
    sqlx::raw_sql("ANALYZE").execute(&pool).await?;
    progress(&format!("all assignments: {:?}", started.elapsed()));

    let now = OffsetDateTime::now_utc();
    let h: Vec<Uuid> = {
        let mut out = Vec::new();
        for key in HERITABLE {
            out.push(bench.property(key).await?);
        }
        out
    };
    let n1 = bench.property(PLAIN[0]).await?;

    // Explained resolution of an address.
    if selection.runs(RESOLUTION) {
        let mut samples = Vec::new();
        for _ in 0..200 {
            let a = &data.addresses[rng.random_range(0..data.addresses.len())];
            let property = h[rng.random_range(0..h.len())];
            let t = Instant::now();
            explain(&pool, a.id, property, now, None).await?;
            samples.push(t.elapsed());
        }
        budgets.push(Budget {
            name: "explained resolution, p95",
            limit: Duration::from_millis(50),
            measured: p95(samples),
            note: "200 draws".into(),
        });
    }

    // Grouped read: 512 addresses, 6 properties (REQ-016-1).
    if selection.runs(GROUPED) {
        let keys: Vec<String> = HERITABLE
            .iter()
            .chain(&PLAIN[..3])
            .map(|k| (*k).to_owned())
            .collect();
        let mut samples = Vec::new();
        for _ in 0..50 {
            let objects: Vec<Uuid> = (0..512)
                .map(|_| data.addresses[rng.random_range(0..data.addresses.len())].id)
                .collect();
            let t = Instant::now();
            resolved_many(&pool, &objects, &keys, now, None).await?;
            samples.push(t.elapsed());
        }
        budgets.push(Budget {
            name: "grouped resolution 512 × 6, p95",
            limit: Duration::from_millis(30),
            measured: p95(samples),
            note: "50 draws".into(),
        });
    }

    // Filtered pages of 100.
    if selection.runs(FILTERED) {
        filtered_pages(&pool, data, &h, n1, now, &mut rng, &mut budgets).await?;
    }

    // A changeset whose differential touches at most 10⁴ objects: the largest container
    // under 10⁴ addresses that carries no value yet, so that origins move.
    if selection.runs(CHANGESET) {
        let (container, n): (Uuid, i64) = sqlx::query_as(
            "SELECT p.id, count(*) AS n FROM prefix p
             JOIN address a ON a.domain_id = p.domain_id AND a.address <<= p.prefix
             WHERE p.nature = 'container'
               AND NOT EXISTS (SELECT 1 FROM property_assignment x WHERE x.object_id = p.id)
             GROUP BY p.id HAVING count(*) <= 10000 ORDER BY n DESC, p.id LIMIT 1",
        )
        .fetch_optional(&pool)
        .await?
        .context("no container under 10^4 addresses")?;
        let (elapsed, diff) = bench.submit(vec![assign(container, h[0], "v9")]).await?;
        budgets.push(Budget {
            name: "changeset, differential up to 10^4",
            limit: Duration::from_millis(2_500),
            measured: elapsed,
            note: format!("{n} addresses below, {} objects changed", touched(&diff)),
        });
    }

    // New value on every root: about 10⁶ objects change value, no origin moves.
    if selection.runs(VALUE_EVERY_ROOT) {
        let ops = roots
            .iter()
            .enumerate()
            .map(|(r, (id, _))| assign(*id, h[1], &format!("v{}", (r + 5) % 10)))
            .collect();
        let (elapsed, diff) = bench.submit(ops).await?;
        budgets.push(Budget {
            name: "value of every root (up to 10^6)",
            limit: Duration::from_secs(10),
            measured: elapsed,
            note: format!("{} objects changed", touched(&diff)),
        });
    }

    // Removing the roots' value in the three largest domains: > 10⁵ origins move.
    if selection.runs(ORIGINS_3_DOMAINS) {
        let mut sizes: Vec<(usize, Uuid)> = data
            .domains
            .iter()
            .enumerate()
            .map(|(i, d)| {
                (
                    data.addresses.iter().filter(|a| a.domain == i).count(),
                    d.id,
                )
            })
            .collect();
        sizes.sort_by_key(|s| std::cmp::Reverse(s.0));
        let largest: Vec<Uuid> = sizes.iter().take(3).map(|s| s.1).collect();
        let ops = roots
            .iter()
            .filter(|(_, d)| largest.contains(d))
            .map(|(id, _)| Operation::UnassignProperty {
                object: Ref::Id(*id),
                kind: ObjectKind::Prefix,
                property: Ref::Id(h[2]),
            })
            .collect();
        let (elapsed, diff) = bench.submit(ops).await?;
        budgets.push(Budget {
            name: "origins of 3 domains removed (P10)",
            limit: Duration::from_secs(60),
            measured: elapsed,
            note: format!("{} objects changed", touched(&diff)),
        });
    }

    let (lines, ok) = verdict(&budgets)?;
    report.push_str(&lines);
    Ok((report, ok))
}

/// Filtered pages of 100, on a heritable property then on a non-heritable one, in turn.
async fn filtered_pages(
    pool: &PgPool,
    data: &Dataset,
    h: &[Uuid],
    n1: Uuid,
    now: OffsetDateTime,
    rng: &mut ChaCha8Rng,
    budgets: &mut Vec<Budget>,
) -> anyhow::Result<()> {
    let mut heritable = Vec::new();
    let mut plain = Vec::new();
    for _ in 0..100 {
        let domain = data.domains[rng.random_range(0..data.domains.len())].id;
        let condition = Condition::Equals(format!("v{}", rng.random_range(0..10)));
        let t = Instant::now();
        filter(
            pool,
            h[rng.random_range(0..h.len())],
            domain,
            &condition,
            now,
            None,
            100,
        )
        .await?;
        heritable.push(t.elapsed());
        let low: i64 = rng.random_range(0..900_000);
        let condition =
            Condition::Between(Some(low.to_string()), Some((low + 100_000).to_string()));
        let t = Instant::now();
        filter(pool, n1, domain, &condition, now, None, 100).await?;
        plain.push(t.elapsed());
    }
    budgets.push(Budget {
        name: "filtered page, heritable, p95",
        limit: Duration::from_millis(150),
        measured: p95(heritable),
        note: "100 draws".into(),
    });
    budgets.push(Budget {
        name: "filtered page, non-heritable, p95",
        limit: Duration::from_millis(150),
        measured: p95(plain),
        note: "100 draws".into(),
    });
    Ok(())
}

/// One line per budget, and whether all hold.
///
/// # Errors
///
/// Never in practice (writing to a string).
pub(crate) fn verdict(budgets: &[Budget]) -> anyhow::Result<(String, bool)> {
    let mut report = String::new();
    let mut ok = true;
    for b in budgets {
        let pass = b.measured <= b.limit;
        ok &= pass;
        writeln!(
            report,
            "{} {}: {:?} (limit {:?}; {})",
            if pass { "PASS" } else { "FAIL" },
            b.name,
            b.measured,
            b.limit,
            b.note
        )?;
    }
    Ok((report, ok))
}
