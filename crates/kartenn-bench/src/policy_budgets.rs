//! Budgets of the flow policy module (spec 010, REQ-010-28 and 54; tasks 010-1 and
//! 010-19) on the reference dataset with the VLAN and machines modules deployed: a policy
//! of the size of the budget (6 axes, 50 environments, 20 flow types, 1 000 services,
//! 10⁴ rules, 100 patterns, 50 conditions, 30 signals) and 2 000 rules of inference.
//! Measures, through the router of the module: the verdict of a pair of addresses for a
//! service, machine × machine (16 × 16 addresses) for a flow type of 10 services, a raw
//! port covered by 20 services, the suggestion of the inference; through the write
//! pipeline: the policy differential of a changeset of rules, the control of the
//! inference on a changeset of 10⁴ objects; the background tasks: the audit report (and,
//! for information, the windows of its rubric of transverse values, one environment being
//! transverse) and a full re-evaluation of the deviations of the inference.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_policy::Policy;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::core_declarations_for;
use kartenn_service::expected::Sources;
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::{MAX_OPERATIONS, submit, submit_with};
use kartenn_service::port::ImportItem;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref};
use kartenn_store_pg::write::PgStore;
use rand::RngExt;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde_json::{Value, json};
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgConnection, PgPool};

use crate::Dataset;
use crate::core_budgets::{Prepared, prepare, unitary};
use crate::props::{Budget, p95, progress, verdict};
use crate::vlan_budgets::{app_with, get, import, reader, sync};

/// The size of the policy of the budget (REQ-010-28).
const ENVIRONMENTS: usize = 50;
/// The transverse environment, among them.
const TRANSVERSE: usize = ENVIRONMENTS - 1;
const FLOW_TYPES: usize = 20;
const SERVICES: usize = 1_000;
const RULES: usize = 10_000;
const PATTERNS: usize = 100;
const CONDITIONS: usize = 50;
const SIGNALS: usize = 30;
/// Rules of inference (REQ-010-54).
const INFERENCE_RULES: usize = 2_000;
/// The services of `ft00` (a flow type of 10 services) and those that also cover tcp/443.
const FT00_SERVICES: usize = 10;
const PORT_443_SERVICES: usize = 20;
/// Machines of 16 addresses for machine × machine, on each side.
const WIDE_MACHINES: usize = 10;
const MACHINE_ADDRESSES: usize = 16;
/// Objects of the changeset of the control of the inference (REQ-010-54).
const CONTROLLED: usize = 10_000;
/// Rounds of the control, with and without the inference in turn.
const CONTROL_ROUNDS: usize = 3;
/// A text long enough for the rules, patterns and conditions that need one (S29).
const TEXT: &str = "Texte pédagogique du banc : flux encadré par la politique.";
/// The values of the example axis `policy.exposure`, one per domain in turn.
const EXPOSURES: [&str; 4] = ["internet", "partenaires", "interne", "administration"];

/// `policy.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> anyhow::Result<Operation> {
    let mut object = fields;
    object["op"] = format!("policy.{name}").into();
    Ok(serde_json::from_value(object)?)
}

/// `machines.<name>` from its JSON fields.
fn machines_op(name: &str, fields: Value) -> anyhow::Result<Operation> {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    Ok(serde_json::from_value(object)?)
}

/// An import item that creates an object: an import names each creation by an external
/// identifier, unique within the source.
fn item(operation: Operation) -> ImportItem {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ImportItem {
        external_id: Some(format!("bench-policy-{n}")),
        operation,
    }
}

/// An import item that creates nothing (an assignment, an attachment, an environment).
fn plain(operation: Operation) -> ImportItem {
    ImportItem {
        external_id: None,
        operation,
    }
}

fn env(i: usize) -> String {
    format!("env{i:02}")
}

/// The catalogue: the example axes, the environments, the flow types, services,
/// patterns, conditions and signals, by local names.
fn catalogue_items() -> anyhow::Result<Vec<ImportItem>> {
    let flow_type = |t: usize| t + 1;
    let mut items = vec![plain(op("examples.install", json!({}))?)];
    // The last environment is transverse: the rubric of the nodes holding one has
    // something to list (REQ-010-23).
    for e in 0..ENVIRONMENTS {
        let mut fields = json!({ "key": env(e) });
        if e == TRANSVERSE {
            fields["transverse"] = true.into();
        }
        items.push(plain(op("environment.create", fields)?));
    }
    for t in 0..FLOW_TYPES {
        items.push(item(op(
            "flow_type.create",
            json!({ "local": flow_type(t), "key": format!("ft{t:02}"),
                    "labels": { "fr": format!("Type de flux {t}") } }),
        )?));
    }
    for s in 0..SERVICES {
        let t = if s < FT00_SERVICES {
            0
        } else {
            1 + (s - FT00_SERVICES) % (FLOW_TYPES - 1)
        };
        let mut entries = vec![json!({ "transport": "tcp", "ports": 2_000 + s })];
        if s < PORT_443_SERVICES {
            entries.push(json!({ "transport": "tcp", "ports": 443 }));
        }
        items.push(item(op(
            "service.create",
            json!({ "key": format!("svc{s:03}"), "labels": { "fr": format!("Service {s}") },
                    "flow_type": { "local": flow_type(t) }, "entries": entries }),
        )?));
    }
    for p in 0..PATTERNS {
        items.push(item(op(
            "pattern.create",
            json!({ "key": format!("pat{p:02}"), "labels": { "fr": format!("Motif {p}") },
                    "texts": { "fr": { "body": TEXT } } }),
        )?));
    }
    for c in 0..CONDITIONS {
        let (kind, params) = match c % 3 {
            0 => ("max_width", json!({ "addresses": 256 })),
            1 => ("named_service", json!({})),
            _ => ("not_whole_domain", json!({})),
        };
        items.push(item(op(
            "condition.create",
            json!({ "key": format!("cond{c:02}"), "kind": kind, "params": params,
                    "flow_types": [{ "local": flow_type(c % FLOW_TYPES) }],
                    "texts": { "fr": { "body": TEXT } } }),
        )?));
    }
    for s in 0..SIGNALS {
        let (kind, params) = match s % 5 {
            0 => ("wide_endpoint", json!({ "addresses": 64 })),
            1 => (
                "flow_type",
                json!({ "flow_types": [format!("ft{:02}", s % FLOW_TYPES)] }),
            ),
            2 => (
                "endpoint_value",
                json!({ "axis": "policy.exposure", "values": ["internet"] }),
            ),
            3 => ("axis_differs", json!({ "axis": "policy.exposure" })),
            _ => ("transverse_only", json!({})),
        };
        items.push(item(op(
            "signal.create",
            json!({ "key": format!("sig{s:02}"), "kind": kind, "params": params,
                    "weight": 1 + s % 5 }),
        )?));
    }
    Ok(items)
}

/// Some distinct environments, 1 to 3.
fn some_environments(rng: &mut ChaCha8Rng) -> Vec<String> {
    let mut picked: Vec<String> = (0..rng.random_range(1..=3))
        .map(|_| env(rng.random_range(0..ENVIRONMENTS)))
        .collect();
    picked.sort();
    picked.dedup();
    picked
}

/// The 10⁴ rules: 80 % allow, 10 % allow under a pattern, 10 % deny; a tenth select on
/// the exposure as well, a twentieth require the same exposure on both sides.
fn rule_items(
    rng: &mut ChaCha8Rng,
    flow_types: &[Uuid],
    patterns: &[Uuid],
) -> anyhow::Result<Vec<ImportItem>> {
    let mut items = Vec::with_capacity(RULES + 1);
    // The rule every machine × machine draw evaluates: env00 to env01 for ft00.
    items.push(item(op(
        "rule.create",
        json!({ "key": "bench-wide", "labels": { "fr": "Banc large" }, "effect": "allow",
                "flow_types": [{ "id": flow_types[0] }],
                "source": { "policy.environment": [env(0)] },
                "destination": { "policy.environment": [env(1)] } }),
    )?));
    for r in 0..RULES {
        let mut source = json!({ "policy.environment": some_environments(rng) });
        if r % 10 == 0 {
            source["policy.exposure"] = json!(["interne", "administration"]);
        }
        let mut fields = json!({
            "key": format!("r{r:05}"), "labels": { "fr": format!("Règle {r}") },
            "source": source,
            "destination": { "policy.environment": some_environments(rng) },
            "flow_types": (0..rng.random_range(1..=2))
                .map(|_| json!({ "id": flow_types[rng.random_range(0..flow_types.len())] }))
                .collect::<Vec<_>>(),
        });
        if r % 20 == 0 {
            fields["same"] = json!(["policy.exposure"]);
        }
        match rng.random_range(0..10) {
            0 => {
                fields["effect"] = "deny".into();
                fields["texts"] = json!({ "fr": { "body": TEXT } });
            }
            1 => {
                fields["effect"] = "allow".into();
                fields["required_patterns"] =
                    json!([{ "id": patterns[rng.random_range(0..patterns.len())] }]);
            }
            _ => fields["effect"] = "allow".into(),
        }
        items.push(item(op("rule.create", fields)?));
    }
    Ok(items)
}

/// The identifiers of the living rows of `table` (`policy.<table>`), by key order.
async fn ids(pool: &PgPool, table: &str) -> anyhow::Result<Vec<Uuid>> {
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT id FROM policy.{table} WHERE upper_inf(valid_period) ORDER BY key"
    )))
    .fetch_all(pool)
    .await?)
}

/// `property` of `object` set to `value`.
fn assign(object: Uuid, kind: ObjectKind, property: Uuid, value: &str) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(object),
        kind,
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    }
}

/// Runs the background task `name` until `done` says it is over; the number of runs.
async fn drain(
    store: &PgStore,
    name: &str,
    done: impl AsyncFn() -> anyhow::Result<bool>,
) -> anyhow::Result<usize> {
    let mut runs = 0;
    loop {
        if !kartenn_service::module::run_job(store, name).await? {
            bail!("no module of the build declares the task {name}");
        }
        runs += 1;
        if done().await? {
            return Ok(runs);
        }
    }
}

/// A changeset of `count` new addresses in `domain` from `10.<base[0]>.<base[1]>.1`, 250 a
/// /24.
fn addresses_request(author: Uuid, domain: Uuid, base: [u8; 2], count: usize) -> ChangesetRequest {
    let operations = (0..count)
        .map(|i| Operation::CreateAddress {
            local: None,
            domain: Ref::Id(domain),
            address: format!(
                "10.{}.{}.{}",
                base[0],
                base[1] + u8::try_from(i / 250).unwrap_or(0),
                i % 250 + 1
            ),
            usage: AddressUsage::Host,
        })
        .collect();
    ChangesetRequest {
        author,
        motive: "bench".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        apply_own: true,
    }
}

/// Loads the dataset of the core and measures the policy's budgets; returns the report and
/// whether every budget holds. The VLAN and machines modules are deployed, as the policy
/// needs them, without their datasets: REQ-010-28 counts the objects of REQ-002-45, and
/// `bench-core` measures those modules.
///
/// # Errors
///
/// Database, pipeline or router failures.
pub async fn run(database: &str, data: &Dataset, seed: u64) -> anyhow::Result<(String, bool)> {
    let Prepared {
        pool,
        author,
        mut report,
        ..
    } = prepare(database, data, seed).await?;
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(2));
    let budgets = measure(&pool, author, data, &mut rng, &mut report).await?;
    let (lines, ok) = verdict(&budgets)?;
    report.push_str(&lines);
    Ok((report, ok))
}

#[allow(clippy::too_many_lines)] // one linear scenario, step by step
async fn measure(
    pool: &PgPool,
    author: Uuid,
    data: &Dataset,
    rng: &mut ChaCha8Rng,
    report: &mut String,
) -> anyhow::Result<Vec<Budget>> {
    let started = Instant::now();
    let load = |what: &str| format!("{what}: {:?}, {}", started.elapsed(), load_now());
    let modules: Vec<Arc<dyn Module>> = vec![
        Arc::new(kartenn_vlan::Vlan),
        Arc::new(kartenn_machines::Machines),
        Arc::new(Policy),
    ];
    let registry = Arc::new(ModuleRegistry::new(modules)?);
    kartenn_store_pg::modules::migrate(pool, &registry).await?;
    let store = PgStore::new(pool.clone()).with_modules(registry.clone());
    let declarations: Vec<_> = std::iter::once(core_declarations_for(&registry))
        .chain(registry.modules().filter_map(|m| m.declarations()))
        .collect();
    // The loaded domains were written without an owner, which the deployment makes
    // mandatory (REQ-003-16): the declarations without their obligations, an owner on each
    // domain, then all (as the VLAN budgets do).
    let mut relaxed = declarations.clone();
    for module in &mut relaxed {
        for (declaration, _) in &mut module.properties {
            declaration.obligations.clear();
        }
    }
    sync(&store, &relaxed).await?;
    unitary(
        &store,
        author,
        Operation::CreateEntity {
            local: None,
            slug: "bench-policy".into(),
            parent: None,
        },
    )
    .await?;
    let entity: Uuid = sqlx::query_scalar(
        "SELECT id FROM entity WHERE slug = 'bench-policy' AND valid_period @> now()",
    )
    .fetch_one(pool)
    .await
    .context("the entity of the bench")?;
    let owner: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
            .fetch_one(pool)
            .await?;
    let assignments = data
        .domains
        .iter()
        .map(|d| {
            plain(Operation::AssignProperty {
                object: Ref::Id(d.id),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(entity.to_string())],
            })
        })
        .collect();
    import(&store, author, "bench-policy-owners", assignments).await?;
    sync(&store, &declarations).await?;
    let sources = Sources::none().with(Arc::new(kartenn_policy::infer::InferenceSource));

    // The catalogue, then the rules, which cite it by identifier.
    import(&store, author, "bench-policy-catalogue", catalogue_items()?).await?;
    let flow_types = ids(pool, "policy_flow_type").await?;
    let patterns = ids(pool, "policy_pattern").await?;
    if flow_types.len() != FLOW_TYPES || patterns.len() != PATTERNS {
        bail!(
            "{} flow types and {} patterns loaded",
            flow_types.len(),
            patterns.len()
        );
    }
    import(
        &store,
        author,
        "bench-policy-rules",
        rule_items(rng, &flow_types, &patterns)?,
    )
    .await?;
    progress(&format!(
        "policy catalogue and rules: {:?}",
        started.elapsed()
    ));

    // The classification: an environment and an exposure on each domain, an environment
    // of its own on a tenth of the networks.
    let property = |key: &'static str| async move {
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM property_declaration WHERE key = $1")
            .bind(key)
            .fetch_one(pool)
            .await
            .with_context(|| format!("the declaration {key}"))
    };
    let environment = property("policy.environment").await?;
    let exposure = property("policy.exposure").await?;
    let mut items = Vec::new();
    for (d, domain) in data.domains.iter().enumerate() {
        items.push(plain(assign(
            domain.id,
            ObjectKind::Domain,
            environment,
            &env(d % ENVIRONMENTS),
        )));
        items.push(plain(assign(
            domain.id,
            ObjectKind::Domain,
            exposure,
            EXPOSURES[d % 4],
        )));
    }
    let networks: Vec<_> = data
        .prefixes
        .iter()
        .filter(|p| p.nature == crate::Nature::Network)
        .collect();
    for (n, network) in networks.iter().enumerate() {
        if n % 10 == 0 {
            items.push(plain(assign(
                network.id,
                ObjectKind::Prefix,
                environment,
                &env(rng.random_range(0..ENVIRONMENTS)),
            )));
        }
    }
    let classified = items.len();
    import(&store, author, "bench-policy-classification", items).await?;
    progress(&format!("policy classification: {:?}", started.elapsed()));

    // A domain of its own: two networks (env00 and env01) holding the machines of 16
    // addresses, and two networks for the control of the inference.
    let mut items = vec![
        item(Operation::CreateDomain {
            local: Some(1),
            slug: "bench-policy".into(),
        }),
        plain(Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(entity.to_string())],
        }),
    ];
    for (n, (prefix, environment_value)) in [
        ("10.1.0.0/16", env(0)),
        ("10.2.0.0/16", env(1)),
        ("10.3.0.0/16", env(2)),
        ("10.4.0.0/16", env(3)),
    ]
    .into_iter()
    .enumerate()
    {
        let local = u32::try_from(10 + n)?;
        items.push(item(Operation::CreatePrefix {
            local: Some(local),
            domain: Ref::Local(1),
            prefix: prefix.into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        }));
        items.push(plain(Operation::AssignProperty {
            object: Ref::Local(local),
            kind: ObjectKind::Prefix,
            property: Ref::Id(environment),
            values: vec![RawInput::Text(environment_value)],
        }));
    }
    import(&store, author, "bench-policy-domain", items).await?;
    let domain: Uuid = sqlx::query_scalar(
        "SELECT id FROM addressing_domain WHERE slug = 'bench-policy' AND valid_period @> now()",
    )
    .fetch_one(pool)
    .await?;
    // The machines: one interface each, 16 new addresses attached.
    let mut items = Vec::new();
    let mut local = 100_u32;
    for side in 0..2_u8 {
        for m in 0..WIDE_MACHINES {
            let machine = local;
            let interface = local + 1;
            local += 2;
            items.push(item(machines_op(
                "create_machine",
                json!({ "local": machine, "name": format!("policy{side}x{m}"),
                        "kind": "physical", "owner": { "id": entity } }),
            )?));
            items.push(item(machines_op(
                "create_interface",
                json!({ "local": interface, "machine": { "local": machine }, "name": "eth0" }),
            )?));
            for a in 0..MACHINE_ADDRESSES {
                items.push(item(Operation::CreateAddress {
                    local: Some(local),
                    domain: Ref::Id(domain),
                    address: format!("10.{}.{m}.{}", side + 1, a + 1),
                    usage: AddressUsage::Host,
                }));
                items.push(plain(machines_op(
                    "attach_address",
                    json!({ "interface": { "local": interface }, "address": { "local": local } }),
                )?));
                local += 1;
            }
        }
    }
    import(&store, author, "bench-policy-machines", items).await?;
    let wide: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT name, id FROM machines.machine WHERE name LIKE 'policy%' AND upper_inf(valid_period) ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    let (sources_side, destinations): (Vec<_>, Vec<_>) = wide
        .iter()
        .partition(|(name, _)| name.starts_with("policy0"));
    if sources_side.len() != WIDE_MACHINES || destinations.len() != WIDE_MACHINES {
        bail!("{} machines of 16 addresses loaded", wide.len());
    }
    sqlx::raw_sql("VACUUM ANALYZE").execute(pool).await?;
    writeln!(
        report,
        "INFO policy dataset: {ENVIRONMENTS} environments, {FLOW_TYPES} flow types, {SERVICES} \
         services, {} rules, {PATTERNS} patterns, {CONDITIONS} conditions, {SIGNALS} signals, \
         {classified} classifications, {} machines of {MACHINE_ADDRESSES} addresses, loaded in {:?}",
        RULES + 1,
        2 * WIDE_MACHINES,
        started.elapsed()
    )?;
    progress(&load("policy dataset"));

    // The snapshot of the policy, read cold (REQ-010-16, ADR 0030).
    let mut conn = pool.acquire().await?;
    let mut samples = Vec::new();
    for _ in 0..10 {
        let cache = kartenn_policy::cache::SnapshotCache::new();
        let t = Instant::now();
        cache
            .snapshot(&mut conn, OffsetDateTime::now_utc(), None)
            .await?;
        samples.push(t.elapsed());
    }
    drop(conn);
    writeln!(
        report,
        "INFO policy snapshot of {} rules read cold: p95 {:?} (10 reads; {})",
        RULES + 1,
        p95(samples),
        load_now()
    )?;

    let identity = reader(
        pool,
        &[
            "policy.catalogue.read",
            "policy.verdict.read",
            "core.address.read",
            "core.prefix.read",
            "core.domain.read",
            "machines.read",
            "machines.interface.read",
        ],
    )
    .await?;
    let app = app_with(
        pool,
        registry,
        vec![
            kartenn_vlan::rest::routes(),
            kartenn_machines::rest::routes(),
            kartenn_policy::rest::routes(),
        ],
        identity,
        sources.clone(),
    )?;
    let address =
        |rng: &mut ChaCha8Rng| data.addresses[rng.random_range(0..data.addresses.len())].id;
    let mut budgets = Vec::new();

    // Warm-up: the snapshot in the cache of the instance, the statements.
    for _ in 0..20 {
        let uri = format!(
            "/v1/verdicts?source=address:{}&destination=address:{}&service=svc{:03}",
            address(rng),
            address(rng),
            rng.random_range(0..SERVICES)
        );
        get(&app, &uri).await?;
    }

    // The verdict of a pair of addresses for a service.
    let mut samples = Vec::new();
    for _ in 0..200 {
        let uri = format!(
            "/v1/verdicts?source=address:{}&destination=address:{}&service=svc{:03}",
            address(rng),
            address(rng),
            rng.random_range(0..SERVICES)
        );
        samples.push(get(&app, &uri).await?.0);
    }
    budgets.push(Budget {
        name: "verdict of a pair of addresses for a service, p95",
        limit: Duration::from_millis(50),
        measured: p95(samples),
        note: format!(
            "200 draws through the router, {} rules; {}",
            RULES + 1,
            load_now()
        ),
    });

    // Machine × machine, 16 × 16 addresses, a flow type of 10 services.
    let mut samples = Vec::new();
    let mut verdicts = 0;
    for _ in 0..100 {
        let (_, source) = sources_side[rng.random_range(0..WIDE_MACHINES)];
        let (_, destination) = destinations[rng.random_range(0..WIDE_MACHINES)];
        let uri = format!(
            "/v1/verdicts?source=machine:{source}&destination=machine:{destination}&flow_type=ft00"
        );
        let (elapsed, answer) = get(&app, &uri).await?;
        verdicts += answer["evaluations"].as_array().map_or(0, Vec::len);
        samples.push(elapsed);
    }
    if verdicts == 0 {
        bail!("machine × machine answers no verdict");
    }
    budgets.push(Budget {
        name: "machine × machine 16 × 16, flow type of 10 services, p95",
        limit: Duration::from_millis(300),
        measured: p95(samples),
        note: format!(
            "100 draws, {} evaluations an answer on average; {}",
            verdicts / 100,
            load_now()
        ),
    });

    // A raw port covered by 20 services.
    let mut samples = Vec::new();
    for _ in 0..200 {
        let uri = format!(
            "/v1/verdicts?source=address:{}&destination=address:{}&transport=tcp&port=443",
            address(rng),
            address(rng)
        );
        samples.push(get(&app, &uri).await?.0);
    }
    budgets.push(Budget {
        name: "raw port covered by 20 services, p95",
        limit: Duration::from_millis(100),
        measured: p95(samples),
        note: format!("200 draws, tcp/443; {}", load_now()),
    });
    progress(&load("policy verdicts"));

    // The policy differential of a changeset of rules: a deny without end.
    let mut samples = Vec::new();
    for i in 0..20 {
        let operation = op(
            "rule.create",
            json!({ "key": format!("bench-deny-{i}"), "labels": { "fr": format!("Interdiction {i}") },
                    "effect": "deny", "texts": { "fr": { "body": TEXT } },
                    "flow_types": [{ "id": flow_types[rng.random_range(0..FLOW_TYPES)] }],
                    "source": { "policy.environment": some_environments(rng) },
                    "destination": { "policy.environment": some_environments(rng) } }),
        )?;
        samples.push(unitary(&store, author, operation).await?);
    }
    budgets.push(Budget {
        name: "policy differential of a changeset of rules, p95",
        limit: Duration::from_secs(1),
        measured: p95(samples),
        note: format!(
            "20 unitary changesets of a deny without end, validated and applied; {}",
            load_now()
        ),
    });
    progress(&load("policy differential"));

    // The audit report, by its background task.
    let mut conn = pool.acquire().await?;
    let t = Instant::now();
    let id = match kartenn_policy::report::request(&mut conn, OffsetDateTime::now_utc(), None)
        .await?
    {
        kartenn_policy::report::Requested::Started(id) => id,
        kartenn_policy::report::Requested::Busy(id) => bail!("a report {id} is already running"),
    };
    let mut runs = 0;
    loop {
        kartenn_policy::report::run_with(
            &mut conn,
            kartenn_policy::report::BATCH,
            kartenn_policy::report::STEPS,
        )
        .await?;
        runs += 1;
        let state = kartenn_policy::report::report(&mut conn, id)
            .await?
            .context("the report vanished")?
            .state;
        if state == "done" {
            break;
        }
    }
    budgets.push(Budget {
        name: "audit report",
        limit: Duration::from_mins(10),
        measured: t.elapsed(),
        note: format!("{runs} runs of the task; {}", load_now()),
    });
    transverse_windows(&mut conn, report).await?;
    drop(conn);
    progress(&load("policy report"));

    // The inference: 2 000 rules on the networks of the dataset, a fifth of them on the
    // domain of the budget.
    let mut items = Vec::with_capacity(INFERENCE_RULES + 2);
    for k in 0..INFERENCE_RULES {
        let network = networks[(k * 37) % networks.len()];
        items.push(item(op(
            "inference_rule.create",
            json!({ "key": format!("inf{k:04}"), "axis": "policy.environment",
                    "priority": k % 10,
                    "domain": { "id": data.domains[network.domain].id },
                    "criteria": [{ "within": [network.prefix.to_string()] }],
                    "values": [env(k % ENVIRONMENTS)] }),
        )?));
    }
    for (k, prefix) in ["10.3.0.0/16", "10.4.0.0/16"].into_iter().enumerate() {
        items.push(item(op(
            "inference_rule.create",
            json!({ "key": format!("inf-bench-{k}"), "axis": "policy.environment",
                    "domain": { "id": domain },
                    "criteria": [{ "within": [prefix] }], "values": [env(2 + k)] }),
        )?));
    }
    import(&store, author, "bench-policy-inference", items).await?;
    progress(&load("policy inference rules"));

    // The suggestion of the inference for an address (REQ-010-54).
    // From the rules down to a few addresses of each (a join from the addresses took ten
    // minutes at 10⁶ addresses).
    let inferred: Vec<Uuid> = sqlx::query_scalar(
        "SELECT a.id FROM policy.policy_inference_rule r
           CROSS JOIN LATERAL jsonb_array_elements_text(r.criteria -> 0 -> 'within') w(prefix)
           CROSS JOIN LATERAL (
               SELECT a.id FROM address a
                WHERE a.domain_id = r.domain_id AND a.address <<= w.prefix::inet
                  AND upper_inf(a.valid_period) LIMIT 5) a
          WHERE upper_inf(r.valid_period) LIMIT 1000",
    )
    .fetch_all(pool)
    .await?;
    if inferred.is_empty() {
        bail!("no address under a rule of inference");
    }
    for _ in 0..20 {
        let a = inferred[rng.random_range(0..inferred.len())];
        get(
            &app,
            &format!("/v1/addresses/{a}/properties/policy.environment/suggestion"),
        )
        .await?;
    }
    let mut samples = Vec::new();
    for _ in 0..200 {
        let a = inferred[rng.random_range(0..inferred.len())];
        samples.push(
            get(
                &app,
                &format!("/v1/addresses/{a}/properties/policy.environment/suggestion"),
            )
            .await?
            .0,
        );
    }
    budgets.push(Budget {
        name: "suggestion of the inference for an object, p95",
        limit: Duration::from_millis(50),
        measured: p95(samples),
        note: format!(
            "200 draws among {} addresses under a rule; {}",
            inferred.len(),
            load_now()
        ),
    });

    // The control of the inference on a changeset of 10⁴ objects: the same changeset
    // with and without the source, in two networks under a rule each, three times in
    // turn; the added time is the difference of the medians, a changeset of 10⁴ objects
    // varying by more than the budget with the load of the machine.
    let (mut with, mut without) = (Vec::new(), Vec::new());
    for round in 0..CONTROL_ROUNDS {
        let offset = u8::try_from(round * 40)?;
        for (network, controlled) in [(3, true), (4, false)] {
            let request = addresses_request(author, domain, [network, offset], CONTROLLED);
            let t = Instant::now();
            let outcome = if controlled {
                submit_with(&store, &Unrestricted, &sources, &request, MAX_OPERATIONS).await?
            } else {
                submit(&store, &Unrestricted, &request, MAX_OPERATIONS).await?
            };
            let elapsed = t.elapsed();
            if !matches!(outcome, Outcome::Applied { .. }) {
                bail!("the changeset of the control (round {round}) is not applied: {outcome:?}");
            }
            if controlled { &mut with } else { &mut without }.push(elapsed);
        }
    }
    let median = |mut samples: Vec<Duration>| {
        samples.sort();
        samples[samples.len() / 2]
    };
    let (with, without) = (median(with), median(without));
    budgets.push(Budget {
        name: "control of the inference on a changeset of 10⁴ objects, added",
        limit: Duration::from_secs(1),
        measured: with.saturating_sub(without),
        note: format!(
            "medians of {CONTROL_ROUNDS} rounds in turn: {with:?} with the source, {without:?} \
             without; {}",
            load_now()
        ),
    });
    progress(&load("policy inference control"));

    // The preview of a rule of inference in the simulation of its changeset (REQ-010-55,
    // S36): a rule over the network of the control, against the same rule over an empty
    // prefix; the added time of the medians, for a budget of 1 s per 10⁴ nodes in scope.
    let preview = |prefix: &str| -> anyhow::Result<ChangesetRequest> {
        Ok(ChangesetRequest {
            author,
            motive: "bench preview".into(),
            period: None,
            operations: vec![op(
                "inference_rule.create",
                json!({ "key": "inf-preview", "axis": "policy.environment",
                        "priority": 99, "domain": { "id": domain },
                        "criteria": [{ "within": [prefix] }], "values": [env(1)] }),
            )?],
            simulate: true,
            idempotency_key: None,
            apply_own: true,
        })
    };
    let (mut wide, mut empty) = (Vec::new(), Vec::new());
    let mut scope = 0;
    for _ in 0..CONTROL_ROUNDS {
        for (prefix, measured) in [("10.3.0.0/16", true), ("10.250.0.0/16", false)] {
            let request = preview(prefix)?;
            let t = Instant::now();
            let outcome = submit(&store, &Unrestricted, &request, MAX_OPERATIONS).await?;
            let elapsed = t.elapsed();
            let Outcome::Simulated { sections, .. } = outcome else {
                bail!("the preview is not simulated: {outcome:?}");
            };
            if measured {
                scope = sections
                    .iter()
                    .flat_map(|s| &s.rows)
                    .find(|r| r.kind == "policy.inference.summary")
                    .and_then(|r| r.after.as_ref()?.get("scope")?.as_u64())
                    .unwrap_or_default();
            }
            if measured { &mut wide } else { &mut empty }.push(elapsed);
        }
    }
    let (wide, empty) = (median(wide), median(empty));
    budgets.push(Budget {
        name: "preview of a rule of inference in its simulation, added",
        limit: Duration::from_secs(scope.div_ceil(10_000).max(1)),
        measured: wide.saturating_sub(empty),
        note: format!(
            "{scope} nodes in scope; medians of {CONTROL_ROUNDS} rounds: {wide:?}, {empty:?} \
             over an empty prefix; {}",
            load_now()
        ),
    });
    progress(&load("policy inference preview"));

    // The full re-evaluation of the deviations of the inference: the rules cite every
    // domain, so the pass goes over every origin and its heirs.
    let t = Instant::now();
    let runs = drain(&store, "policy.inference_check", async || {
        let (pass, checked, revision): (Option<i64>, i64, i64) = sqlx::query_as(
            "SELECT c.pass, c.checked, r.inference
               FROM policy.policy_inference_check c, policy.policy_revision r",
        )
        .fetch_one(pool)
        .await?;
        Ok(pass.is_none() && checked == revision)
    })
    .await?;
    let heirs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM address WHERE upper_inf(valid_period)")
            .fetch_one(pool)
            .await?;
    budgets.push(Budget {
        name: "full re-evaluation of the inference",
        limit: Duration::from_mins(10),
        measured: t.elapsed(),
        note: format!(
            "{runs} runs of the task, {heirs} addresses in the referential; {}",
            load_now()
        ),
    });
    progress(&load("policy inference check"));
    Ok(budgets)
}

/// The windows of the rubric `transverse` of the audit report (nodes holding a transverse
/// value), timed one by one in a report of their own, reported then removed: the rubric
/// goes over every resolved origin of the axes with a transverse value, by windows of
/// [`kartenn_policy::report::BATCH`] objects, each in the write transaction of a run.
async fn transverse_windows(conn: &mut PgConnection, report: &mut String) -> anyhow::Result<()> {
    let rubric = kartenn_policy::report::RUBRICS
        .iter()
        .position(|r| *r == "transverse")
        .context("the rubric transverse")?;
    let rubric = i32::try_from(rubric)?;
    let id = match kartenn_policy::report::request(&mut *conn, OffsetDateTime::now_utc(), None)
        .await?
    {
        kartenn_policy::report::Requested::Started(id) => id,
        kartenn_policy::report::Requested::Busy(id) => bail!("a report {id} is already running"),
    };
    sqlx::query("UPDATE policy.policy_report SET rubric = $2 WHERE id = $1")
        .bind(id)
        .bind(rubric)
        .execute(&mut *conn)
        .await?;
    let mut windows = Vec::new();
    loop {
        let at: i32 = sqlx::query_scalar("SELECT rubric FROM policy.policy_report WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;
        if at != rubric {
            break;
        }
        let t = Instant::now();
        kartenn_policy::report::run_with(&mut *conn, kartenn_policy::report::BATCH, 1).await?;
        windows.push(t.elapsed());
    }
    let listed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM policy.policy_report_item WHERE report_id = $1")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;
    sqlx::query("DELETE FROM policy.policy_report WHERE id = $1")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    if listed == 0 {
        bail!("the rubric transverse listed no node");
    }
    let total: Duration = windows.iter().sum();
    let n = u32::try_from(windows.len())?;
    writeln!(
        report,
        "INFO audit report, rubric transverse: {n} windows of {} objects in {total:?} \
         (mean {:?}, first {:?}, last {:?}, max {:?}), {listed} nodes listed",
        kartenn_policy::report::BATCH,
        total / n.max(1),
        windows.first().copied().unwrap_or_default(),
        windows.last().copied().unwrap_or_default(),
        windows.iter().max().copied().unwrap_or_default(),
    )?;
    Ok(())
}

/// The load average of the machine over a minute, as `uptime` prints it.
fn load_now() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|l| l.split_whitespace().next().map(|l| format!("load {l}")))
        .unwrap_or_else(|| "load ?".into())
}
