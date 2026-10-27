//! Budgets of `GET /v1/stats` (spec 024, REQ-024-16, M18; tasks 024-7 and 7b) on the
//! dataset of `bench-core`, its VLANs loaded and the machines module deployed, for a reader of
//! everything and for a person of 50 groups and 1 000 bindings (the subject of
//! `bench-rbac`), whose counts go through the predicate of the read scope: the route
//! served from its snapshot, through the router, and the computation of a snapshot
//! (`kartenn_rest::stats::compute`, what a recomputation in the background costs).

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow, bail};
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::{Extension, Router};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rate, Rates};
use kartenn_rest::stats::{Freshness, Snapshots};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use tower::ServiceExt;

use crate::Dataset;
use crate::props::{Budget, SINCE, progress};
use crate::rbac::{GROUPS, ISSUER, bind};
use crate::vlan_budgets::As;

/// Computations per subject (each of seconds at the full scale).
const COMPUTATIONS: usize = 20;
/// Reads of the route per subject, served from the snapshot.
const READS: usize = 200;
/// Budget of the route served from a snapshot (REQ-024-16).
const ROUTE: Duration = Duration::from_millis(200);
/// Budget of a computation, in the background (REQ-024-16, Q-024-7).
const COMPUTATION: Duration = Duration::from_secs(10);

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
    let group = crate::rbac::group(pool, "bench-stats", &[person], &[]).await?;
    sqlx::query(
        "INSERT INTO binding (id, group_id, role_id, scope_kind, valid_period, changeset_id)
         VALUES ($1, $2, $3, 'global', $4::tstzrange, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(group)
    .bind(role)
    .bind(SINCE)
    .bind(nil)
    .execute(pool)
    .await?;
    Ok(())
}

/// p50, p95, p99 of samples.
struct Spread {
    p50: Duration,
    p95: Duration,
    p99: Duration,
}

impl Spread {
    fn of(mut samples: Vec<Duration>) -> Self {
        samples.sort();
        let at = |q: usize| {
            let i = (samples.len() * q).div_ceil(100).saturating_sub(1);
            samples.get(i).copied().unwrap_or_default()
        };
        Self {
            p50: at(50),
            p95: at(95),
            p99: at(99),
        }
    }
}

impl std::fmt::Display for Spread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "p50 {:?}, p95 {:?}, p99 {:?}",
            self.p50, self.p95, self.p99
        )
    }
}

/// [`COMPUTATIONS`] computations of the statistics of `principal`, after a warm-up of
/// the connections and statements; the objects counted by the last, for the report.
async fn computations(api: &Api, principal: &Principal) -> anyhow::Result<(Spread, i64)> {
    let subject = principal.subject();
    let compute = async || {
        kartenn_rest::stats::compute(api, &subject, OffsetDateTime::now_utc())
            .await
            .map_err(|p| anyhow!("{p:?}"))
    };
    for _ in 0..2 {
        compute().await?;
    }
    let mut samples = Vec::with_capacity(COMPUTATIONS);
    let mut last = None;
    for _ in 0..COMPUTATIONS {
        let t = Instant::now();
        last = Some(compute().await?);
        samples.push(t.elapsed());
    }
    let objects = last.map_or(0, |s| s.objects.values().sum::<i64>());
    Ok((Spread::of(samples), objects))
}

/// The router as `principal`, with `snapshots`.
fn app(api: Api, principal: Principal, snapshots: &Arc<Snapshots>) -> anyhow::Result<Router> {
    let pool = api.pool.clone();
    let unlimited = Rate::new(1_000_000, 1_000_000).context("non-zero literals")?;
    Ok(kartenn_rest::router(
        kartenn_rest::v1_with(api, vec![kartenn_machines::rest::routes()])
            .layer(Extension(snapshots.clone())),
        Arc::new(PgReadiness::new(pool, AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates {
            read: unlimited,
            write: unlimited,
            anonymous: unlimited,
        })),
        Arc::new(As(principal)),
    ))
}

/// `GET /v1/stats`, timed to the last byte of the body, which must be a 200.
async fn get(app: &Router) -> anyhow::Result<Duration> {
    let request = Request::builder()
        .uri("/v1/stats")
        .header(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer bench"),
        )
        .body(Body::empty())?;
    let t = Instant::now();
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = response.into_body().collect().await?.to_bytes();
    let elapsed = t.elapsed();
    if status != StatusCode::OK {
        bail!(
            "GET /v1/stats: {status} {}",
            String::from_utf8_lossy(&bytes)
        );
    }
    Ok(elapsed)
}

/// [`READS`] reads of the route as `principal`, served from the snapshot that its first
/// read computed (not measured).
async fn reads(api: &Api, principal: &Principal) -> anyhow::Result<Spread> {
    let snapshots = Snapshots::new(Freshness::default());
    let app = app(api.clone(), principal.clone(), &snapshots)?;
    get(&app).await?;
    let mut samples = Vec::with_capacity(READS);
    for _ in 0..READS {
        samples.push(get(&app).await?);
    }
    if snapshots.computations() != 2 {
        bail!(
            "GET /v1/stats computed {} parts instead of 2",
            snapshots.computations()
        );
    }
    Ok(Spread::of(samples))
}

/// Measures the budgets of the statistics; the VLANs are loaded and the module machines
/// deployed on `pool` by the budgets before it.
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
        Arc::new(kartenn_machines::Machines) as Arc<dyn Module>,
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
    let subjects = [
        (
            "reader of everything",
            Principal {
                identity: reader,
                kind: IdentityKind::Person,
                issuer: None,
                groups: Vec::new(),
                method: Method::Bearer,
            },
        ),
        (
            "1 000 bindings",
            Principal {
                identity: scoped,
                kind: IdentityKind::Person,
                issuer: Some(ISSUER.into()),
                groups: (0..GROUPS).map(|g| format!("g{g:02}")).collect(),
                method: Method::Bearer,
            },
        ),
    ];
    let mut measured = Vec::new();
    for (who, principal) in &subjects {
        let (computed, objects) = computations(&api, principal).await?;
        let served = reads(&api, principal).await?;
        writeln!(
            report,
            "INFO stats, {who}: {objects} objects counted; route served {served}; \
             computation {computed}"
        )?;
        measured.push((served, computed));
    }
    progress(&format!("stats: {:?}", started.elapsed()));
    let [
        (global_served, global_computed),
        (bound_served, bound_computed),
    ] = <[(Spread, Spread); 2]>::try_from(measured)
        .map_err(|_| anyhow!("two subjects measured"))?;
    Ok(vec![
        Budget {
            name: "stats, reader of everything, route from the snapshot, p95",
            limit: ROUTE,
            measured: global_served.p95,
            note: format!("{READS} reads"),
        },
        Budget {
            name: "stats, 1 000 bindings, route from the snapshot, p95",
            limit: ROUTE,
            measured: bound_served.p95,
            note: format!("{READS} reads, 50 groups"),
        },
        Budget {
            name: "stats, reader of everything, computation, p95",
            limit: COMPUTATION,
            measured: global_computed.p95,
            note: format!("{COMPUTATIONS} computations, core and modules"),
        },
        Budget {
            name: "stats, 1 000 bindings, computation, p95",
            limit: COMPUTATION,
            measured: bound_computed.p95,
            note: format!("{COMPUTATIONS} computations, 50 groups"),
        },
    ])
}
