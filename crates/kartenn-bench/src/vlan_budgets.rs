//! Budgets of the VLAN module (spec 008, REQ-008-25; task 008-16) on the reference dataset
//! augmented with 2·10⁴ groups and 10⁵ VLANs, half of them on one or two networks: reading
//! a VLAN, a filtered page and the occupancy of a group through the router of the module,
//! and the unitary changeset that creates a VLAN through the write pipeline.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rate, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{ModuleDeclarations, core_declarations_for, sync_modules};
use kartenn_service::import::{create_import, run_import};
use kartenn_service::module::ModuleRegistry;
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::ImportItem;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ObjectKind, Operation, Outcome, Ref};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use tower::ServiceExt;

use crate::Dataset;
use crate::core_budgets::unitary;
use crate::props::{Budget, p95, progress};

/// Groups and VLANs of the reference dataset (REQ-008-25); a smaller dataset (fewer
/// networks than the VLANs carry) gets one VLAN per network and a fifth as many groups.
const GROUPS: usize = 20_000;
const VLANS: usize = 100_000;
/// Entities the groups are anchored on, in turn.
const ENTITIES: usize = 20;

/// Any `Bearer` token is this principal.
pub(crate) struct As(pub(crate) Principal);

impl Authenticator for As {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(self.0.clone().into()),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }
}

/// `vlan.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> anyhow::Result<Operation> {
    let mut object = fields;
    object["op"] = format!("vlan.{name}").into();
    Ok(serde_json::from_value(object)?)
}

/// Runs `items` as one import, which must end.
pub(crate) async fn import(
    store: &PgStore,
    author: Uuid,
    source: &str,
    items: Vec<ImportItem>,
) -> anyhow::Result<()> {
    let import = create_import(store, author, source, items, MAX_OPERATIONS).await?;
    let status = run_import(store, &Unrestricted, import, MAX_OPERATIONS)
        .await?
        .context("the import vanished")?;
    if status.state != "done" {
        bail!("import {source} not done: {}", status.report);
    }
    Ok(())
}

/// Brings the stored declarations in line with `declarations`.
pub(crate) async fn sync(
    store: &PgStore,
    declarations: &[ModuleDeclarations],
) -> anyhow::Result<()> {
    for outcome in sync_modules(store, &Unrestricted, declarations, MAX_OPERATIONS).await? {
        if !matches!(outcome, Outcome::Applied { .. }) {
            bail!("module declarations not applied: {outcome:?}");
        }
    }
    Ok(())
}

/// A reader of `entries`, everywhere.
pub(crate) async fn reader(pool: &PgPool, entries: &[&str]) -> anyhow::Result<Uuid> {
    reader_on(pool, entries, None).await
}

/// A reader of `entries` on the entity `scope`, everywhere without one.
pub(crate) async fn reader_on(
    pool: &PgPool,
    entries: &[&str],
    scope: Option<Uuid>,
) -> anyhow::Result<Uuid> {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'bench-module') RETURNING id",
    )
    .fetch_one(pool)
    .await?;
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, $2, jsonb_build_object('fr', 'bench'), tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .bind(format!("bench_{}", role.simple()))
    .execute(pool)
    .await?;
    for entry in entries {
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(entry)
        .execute(pool)
        .await?;
    }
    sqlx::query(crate::rbac::BIND_IDENTITY)
        .bind(identity)
        .bind(role)
        .bind(if scope.is_some() { "entity" } else { "global" })
        .bind(scope)
        .execute(pool)
        .await?;
    Ok(identity)
}

/// The router of an instance with the modules of `registry` and their `routes`, as
/// `identity`, without quotas in the way.
pub(crate) fn app(
    pool: &PgPool,
    registry: Arc<ModuleRegistry>,
    routes: Vec<kartenn_rest::ModuleRouter>,
    identity: Uuid,
) -> anyhow::Result<Router> {
    app_with(
        pool,
        registry,
        routes,
        identity,
        kartenn_service::expected::Sources::none(),
    )
}

/// [`app`] with the sources of expected values of the deployment (the policy's inference).
pub(crate) fn app_with(
    pool: &PgPool,
    registry: Arc<ModuleRegistry>,
    routes: Vec<kartenn_rest::ModuleRouter>,
    identity: Uuid,
    sources: kartenn_service::expected::Sources,
) -> anyhow::Result<Router> {
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1"))?,
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("bench")),
        audit: ReadAudit::default(),
        sources,
        outputs: Arc::from([]),
        modules: registry,
    };
    let unlimited = Rate::new(1_000_000, 1_000_000).context("non-zero literals")?;
    Ok(kartenn_rest::router(
        kartenn_rest::v1_with(api, routes),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates {
            read: unlimited,
            write: unlimited,
            anonymous: unlimited,
        })),
        Arc::new(As(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    ))
}

/// `GET uri`, timed to the last byte of the body, which must be a 200.
pub(crate) async fn get(app: &Router, uri: &str) -> anyhow::Result<(Duration, Value)> {
    let request = Request::builder()
        .uri(uri)
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
        bail!("GET {uri}: {status} {}", String::from_utf8_lossy(&bytes));
    }
    Ok((elapsed, serde_json::from_slice(&bytes)?))
}

/// Deploys the module, loads its groups, VLANs and networks, and measures; appends to
/// `report` and returns the budgets.
///
/// # Errors
///
/// Database, pipeline or router failures.
#[allow(clippy::too_many_lines)] // one linear scenario, step by step
pub(crate) async fn run(
    pool: &PgPool,
    author: Uuid,
    data: &Dataset,
    rng: &mut ChaCha8Rng,
    report: &mut String,
) -> anyhow::Result<Vec<Budget>> {
    let started = Instant::now();
    // The deployment of a build with the module: its schema, the core completed by its
    // types, then its declarations.
    let registry = Arc::new(ModuleRegistry::new(vec![Arc::new(kartenn_vlan::Vlan)])?);
    kartenn_store_pg::modules::migrate(pool, &registry).await?;
    let store = PgStore::new(pool.clone()).with_modules(registry.clone());
    let declarations: Vec<_> = std::iter::once(core_declarations_for(&registry))
        .chain(registry.modules().filter_map(|m| m.declarations()))
        .collect();
    // The loaded domains were written without an owner, which the deployment makes
    // mandatory on them as on any domain created through the API (REQ-003-16): the
    // declarations first without their obligations, an owner on each domain, then all.
    let mut relaxed = declarations.clone();
    for module in &mut relaxed {
        for (declaration, _) in &mut module.properties {
            declaration.obligations.clear();
        }
    }
    sync(&store, &relaxed).await?;

    // The entities the groups are anchored on, and the owners of the domains.
    let mut entities = Vec::new();
    for e in 0..ENTITIES {
        unitary(
            &store,
            author,
            Operation::CreateEntity {
                local: None,
                slug: format!("bench-vlan-{e}"),
                parent: None,
            },
        )
        .await?;
        entities.push(
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM entity WHERE slug = $1 AND valid_period @> now()",
            )
            .bind(format!("bench-vlan-{e}"))
            .fetch_one(pool)
            .await?,
        );
    }
    let owner: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
            .fetch_one(pool)
            .await?;
    let domains: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM addressing_domain WHERE upper_inf(valid_period)")
            .fetch_all(pool)
            .await?;
    for (i, domain) in domains.iter().enumerate() {
        unitary(
            &store,
            author,
            Operation::AssignProperty {
                object: Ref::Id(*domain),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(entities[i % ENTITIES].to_string())],
            },
        )
        .await?;
    }
    sync(&store, &declarations).await?;

    // Groups and VLANs in one import: VIDs 1 to n in each group.
    let networks: Vec<Uuid> = data
        .prefixes
        .iter()
        .filter(|p| p.nature == crate::Nature::Network)
        .map(|p| p.id)
        .collect();
    // The reference dataset has about 10⁵ networks: enough for the 7.5·10⁴ the VLANs carry.
    let (groups, vlans) = if networks.len() >= VLANS * 3 / 4 {
        (GROUPS, VLANS)
    } else {
        (networks.len().div_ceil(5), networks.len())
    };
    let per_group = vlans.div_ceil(groups);
    let mut items = Vec::with_capacity(groups + vlans);
    for g in 0..groups {
        items.push(ImportItem {
            external_id: Some(format!("group-{g}")),
            operation: op(
                "create_group",
                json!({"local": g + 1, "slug": format!("bench-{g}"),
                       "anchor": {"entity": {"id": entities[g % ENTITIES]}},
                       "vid_min": 1, "vid_max": 4094}),
            )?,
        });
    }
    for v in 0..vlans {
        let (g, vid) = (v / per_group, v % per_group + 1);
        items.push(ImportItem {
            external_id: Some(format!("vlan-{v}")),
            operation: op(
                "create_vlan",
                json!({"local": groups + v + 1, "group": {"local": g + 1},
                       "vid": vid, "name": format!("v{vid}")}),
            )?,
        });
    }
    import(&store, author, "bench-vlan", items).await?;
    progress(&format!("vlan groups and vlans: {:?}", started.elapsed()));

    // Half of the VLANs on one or two networks, in turn.
    let vlan_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT v.id FROM vlan.vlan v JOIN external_id e ON e.object_id = v.id
          WHERE e.source = 'bench-vlan' AND upper_inf(v.valid_period)
          ORDER BY split_part(e.external_id, '-', 2)::int",
    )
    .fetch_all(pool)
    .await?;
    if vlan_ids.len() != vlans {
        bail!("{} VLANs loaded, {vlans} expected", vlan_ids.len());
    }
    let property: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'vlan.vlan'")
            .fetch_one(pool)
            .await?;
    let mut items = Vec::new();
    let mut next = networks.iter();
    let mut carried = 0;
    for (i, vlan) in vlan_ids.iter().enumerate().filter(|(i, _)| i % 2 == 0) {
        for _ in 0..=(i / 2 % 2) {
            let Some(network) = next.next() else { break };
            items.push(ImportItem {
                external_id: None,
                operation: Operation::AssignProperty {
                    object: Ref::Id(*network),
                    kind: ObjectKind::Prefix,
                    property: Ref::Id(property),
                    values: vec![RawInput::Text(vlan.to_string())],
                },
            });
        }
        carried += 1;
    }
    let assigned = items.len();
    import(&store, author, "bench-vlan-networks", items).await?;
    sqlx::raw_sql("VACUUM ANALYZE vlan.vlan_group, vlan.vlan, module_object, property_assignment")
        .execute(pool)
        .await?;
    writeln!(
        report,
        "INFO vlan dataset: {groups} groups, {vlans} VLANs, {carried} on {assigned} networks, loaded in {:?}",
        started.elapsed()
    )?;
    progress(&format!("vlan networks: {:?}", started.elapsed()));

    let group_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM vlan.vlan_group WHERE upper_inf(valid_period) AND slug LIKE 'bench-%'",
    )
    .fetch_all(pool)
    .await?;
    let identity = reader(pool, &["vlan.group.read", "vlan.read", "core.prefix.read"]).await?;
    let app = app(pool, registry, vec![kartenn_vlan::rest::routes()], identity)?;
    let mut budgets = Vec::new();

    // Warm-up of the connections and statements.
    for _ in 0..20 {
        let vlan = vlan_ids[rng.random_range(0..vlan_ids.len())];
        get(&app, &format!("/v1/vlans/{vlan}")).await?;
    }

    // Reading a VLAN.
    let mut samples = Vec::new();
    for _ in 0..200 {
        let vlan = vlan_ids[rng.random_range(0..vlan_ids.len())];
        samples.push(get(&app, &format!("/v1/vlans/{vlan}")).await?.0);
    }
    budgets.push(Budget {
        name: "read a VLAN, p95",
        limit: Duration::from_millis(50),
        measured: p95(samples),
        note: "200 draws, through the router".into(),
    });

    // Filtered pages of 100: a range of VIDs and a status, across the groups.
    let mut samples = Vec::new();
    let mut rows = 0;
    for _ in 0..100 {
        let low = rng.random_range(1..=per_group.max(1));
        let uri = format!(
            "/v1/vlans?vid={low}..{}&status=active&page_size=100",
            low + 1
        );
        let (elapsed, page) = get(&app, &uri).await?;
        rows += page["items"].as_array().map_or(0, Vec::len);
        samples.push(elapsed);
    }
    budgets.push(Budget {
        name: "filtered page of 100 VLANs, p95",
        limit: Duration::from_millis(150),
        measured: p95(samples),
        note: format!(
            "100 draws of two VIDs across the groups, {} rows a page on average",
            rows / 100
        ),
    });

    // Occupancy of a group.
    let mut samples = Vec::new();
    for _ in 0..200 {
        let group = group_ids[rng.random_range(0..group_ids.len())];
        samples.push(
            get(&app, &format!("/v1/vlan-groups/{group}/occupancy"))
                .await?
                .0,
        );
    }
    budgets.push(Budget {
        name: "occupancy of a VLAN group, p95",
        limit: Duration::from_millis(200),
        measured: p95(samples),
        note: "200 draws, through the router".into(),
    });

    // Unitary changesets: a new VLAN in a random group, past the loaded VIDs.
    let mut samples = Vec::new();
    for i in 0..100 {
        let group = group_ids[rng.random_range(0..group_ids.len())];
        let vid = 1_000 + i;
        let operation = op(
            "create_vlan",
            json!({"group": {"id": group}, "vid": vid, "name": format!("new{vid}")}),
        )?;
        samples.push(unitary(&store, author, operation).await?);
    }
    budgets.push(Budget {
        name: "create a VLAN, p95",
        limit: Duration::from_millis(200),
        measured: p95(samples),
        note: "100 unitary changesets, validated and applied".into(),
    });
    progress(&format!("vlan measures: {:?}", started.elapsed()));
    Ok(budgets)
}
