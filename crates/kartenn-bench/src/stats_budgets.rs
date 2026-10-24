//! Budget of `GET /v1/stats` (spec 024, REQ-024-16; task 024-7) on the dataset of
//! `bench-core`, the VLAN and machines modules deployed: every counter computed with a
//! cold cache, as `kartenn_rest::stats::compute` does on a miss, for a reader of
//! everything and for a person of 50 groups and 1 000 bindings (the subject of
//! `bench-rbac`), whose counts go through the predicate of the read scope.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::objects::Api;
use kartenn_service::audit::Instance;
use kartenn_service::authz::Subject;
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::Dataset;
use crate::props::{Budget, SINCE, p95, progress};
use crate::rbac::{GROUPS, ISSUER, bind};

/// Draws per subject.
const DRAWS: usize = 40;

/// A person, its id.
async fn person(pool: &PgPool, name: &str) -> anyhow::Result<Uuid> {
    Ok(sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), $1) RETURNING id",
    )
    .bind(name)
    .fetch_one(pool)
    .await?)
}

/// Binds every permission, everywhere, to `person`.
async fn everything(pool: &PgPool, person: Uuid) -> anyhow::Result<()> {
    let role = Uuid::now_v7();
    let nil = Uuid::nil();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, 'bench_stats_reader', jsonb_build_object('fr', 'Bench'), $2::tstzrange, $3)",
    )
    .bind(role)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
         VALUES ($1, '*', $2::tstzrange, $3)",
    )
    .bind(role)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind,
                              valid_period, changeset_id)
         VALUES ($1, 'identity', $2, $3, 'global', $4::tstzrange, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(person)
    .bind(role)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    Ok(())
}

/// The p95 of [`DRAWS`] cold computations of the statistics of `subject`, after a
/// warm-up of the connections and statements; the last result, for the report.
async fn measure(api: &Api, subject: &Subject) -> anyhow::Result<(Duration, String)> {
    let compute = async || {
        kartenn_rest::stats::compute(api, subject, OffsetDateTime::now_utc())
            .await
            .map_err(|p| anyhow!("{p:?}"))
    };
    for _ in 0..5 {
        compute().await?;
    }
    let mut samples = Vec::with_capacity(DRAWS);
    let mut last = None;
    for _ in 0..DRAWS {
        let t = Instant::now();
        last = Some(compute().await?);
        samples.push(t.elapsed());
    }
    let objects = last.map_or(0, |s| s.objects.values().sum::<i64>());
    Ok((p95(samples), format!("{objects} objects counted")))
}

/// Measures the budget of the statistics; the modules VLAN and machines are deployed
/// on `pool` by the budgets before it.
///
/// # Errors
///
/// Database failures.
pub(crate) async fn run(
    pool: &PgPool,
    data: &Dataset,
    seed: u64,
    report: &mut String,
) -> anyhow::Result<Vec<Budget>> {
    let started = Instant::now();
    let registry = Arc::new(ModuleRegistry::new(vec![
        Arc::new(kartenn_vlan::Vlan) as Arc<dyn Module>,
        Arc::new(kartenn_machines::Machines),
    ])?);
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1"))?,
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("bench")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: registry,
    };
    let reader = person(pool, "bench-stats-reader").await?;
    everything(pool, reader).await?;
    let scoped = person(pool, "bench-stats-scoped").await?;
    bind(pool, data, scoped, seed).await?;
    let (global, global_note) = measure(
        &api,
        &Subject {
            id: reader,
            issuer: None,
            groups: Vec::new(),
        },
    )
    .await?;
    let (bound, bound_note) = measure(
        &api,
        &Subject {
            id: scoped,
            issuer: Some(ISSUER.into()),
            groups: (0..GROUPS).map(|g| format!("g{g:02}")).collect(),
        },
    )
    .await?;
    writeln!(
        report,
        "INFO stats: reader of everything {global_note}, 1 000 bindings {bound_note}"
    )?;
    progress(&format!("stats: {:?}", started.elapsed()));
    Ok(vec![
        Budget {
            name: "stats, reader of everything, cold cache, p95",
            limit: Duration::from_millis(200),
            measured: global,
            note: format!("{DRAWS} computations, core and modules"),
        },
        Budget {
            name: "stats, 1 000 bindings, cold cache, p95",
            limit: Duration::from_millis(200),
            measured: bound,
            note: format!("{DRAWS} computations, 50 groups"),
        },
    ])
}
