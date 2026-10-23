//! Budgets of the flow registry (spec 011, REQ-011-19, F14; task 011-10) on the reference
//! dataset with the four modules deployed: a policy of 10⁴ rules on the environments
//! (`policy.environment` on the first containers below the roots of the domains), then
//! 10⁵ registered flows between addresses, 10⁴ of them leaving one container.
//!
//! Measured: the request of a flow machine × machine 16 × 16, triage included; the
//! changeset that reclassifies the container 10⁴ flows leave, against the same changeset
//! on a container no flow touches (the added time of the re-evaluation); a write of the
//! policy, which re-evaluates every living flow (011-5); filtered pages of flows and of
//! non-conformities through the router of the module.
//!
//! `kartenn-bench flow <database-url> <seed> [small]` (`bench-flow`).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use kartenn_service::declarations::core_declarations_for;
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::ImportItem;
use kartenn_service::{
    ChangesetRequest, ObjectKind, Operation, Outcome, Ref, authz::Unrestricted, submit,
};
use kartenn_store_pg::write::PgStore;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::{Value, json};
use sqlx::types::Uuid;

use crate::core_budgets::{Prepared, prepare, unitary};
use crate::props::{Budget, p95, progress, verdict};
use crate::vlan_budgets::{app, get, import, reader, sync};
use crate::{Dataset, Nature, Prefix};

/// Flows of the reference dataset (REQ-011-19), and those that leave the hot container.
const FLOWS: usize = 100_000;
const HOT: usize = 10_000;
/// The policy, of the size of the banc 010 (REQ-010-28): 50 environments, 20 flow types
/// of 50 services each, and for each flow type and source environment a rule per block
/// of [`BLOCK`] destination environments, 20 × 50 × 10 = 10⁴ rules. A smaller dataset
/// keeps the reference ratio of a rule per ten flows, in whole blocks, with at least
/// 10 environments (400 rules): 15 environments, 900 rules, at the reduced scale.
const ENVIRONMENTS: usize = 50;
const SMALL_ENVIRONMENTS: usize = 10;

/// The environments of a policy of about `flows / 10` rules (4·e² of them), in whole
/// blocks, between [`SMALL_ENVIRONMENTS`] and [`ENVIRONMENTS`].
fn environments_for(flows: usize) -> usize {
    let e = (flows / 10 / (FLOW_TYPES / BLOCK)).isqrt();
    (e / BLOCK * BLOCK).clamp(SMALL_ENVIRONMENTS, ENVIRONMENTS)
}
const FLOW_TYPES: usize = 20;
const SERVICES_PER_TYPE: usize = 50;
const BLOCK: usize = 5;
/// Entities that own the domains and the flows, in turn.
const ENTITIES: usize = 20;
/// Machines of the requests 16 × 16, two by request, and their addresses.
const PAIRS: usize = 30;
const PER_MACHINE: usize = 16;
/// Rounds of the reclassification of the hot and the cold containers.
const ROUNDS: usize = 3;

/// The rule of a cell is a deny when this holds, an allow otherwise.
const fn denied(source: usize, destination: usize, flow_type: usize) -> bool {
    (source + destination / BLOCK + flow_type).is_multiple_of(5)
}

fn environment(e: usize) -> String {
    format!("e{e:02}")
}

/// `<module>.<name>` from its JSON fields.
fn op(module: &str, name: &str, fields: Value) -> anyhow::Result<Operation> {
    let mut object = fields;
    object["op"] = format!("{module}.{name}").into();
    Ok(serde_json::from_value(object)?)
}

fn item(operation: Operation) -> ImportItem {
    ImportItem {
        external_id: None,
        operation,
    }
}

/// An item that creates an object, which an import names (`import.rs`).
fn named(external_id: String, operation: Operation) -> ImportItem {
    ImportItem {
        external_id: Some(external_id),
        operation,
    }
}

/// A first container below the root of a domain, its environment and its addresses.
struct Zone {
    id: Uuid,
    domain: usize,
    environment: usize,
    addresses: Vec<Uuid>,
}

/// The first containers below the roots, each address in the one that holds it.
fn zones(data: &Dataset, environments: usize) -> Vec<Zone> {
    let mut lengths: HashMap<usize, Vec<u8>> = HashMap::new();
    for p in data
        .prefixes
        .iter()
        .filter(|p| p.nature == Nature::Container)
    {
        lengths.entry(p.domain).or_default().push(p.prefix.len);
    }
    // The second length of the containers of a domain: the level below its root.
    let level: HashMap<usize, u8> = lengths
        .into_iter()
        .filter_map(|(d, mut l)| {
            l.sort_unstable();
            l.dedup();
            l.get(1).map(|&len| (d, len))
        })
        .collect();
    let mut zones = Vec::new();
    let mut index: HashMap<(usize, Prefix), usize> = HashMap::new();
    for p in &data.prefixes {
        if p.nature == Nature::Container && level.get(&p.domain) == Some(&p.prefix.len) {
            index.insert((p.domain, p.prefix), zones.len());
            zones.push(Zone {
                id: p.id,
                domain: p.domain,
                environment: zones.len() % environments,
                addresses: Vec::new(),
            });
        }
    }
    for a in &data.addresses {
        let Some(&len) = level.get(&a.domain) else {
            continue;
        };
        let full = Prefix {
            addr: a.addr,
            len: a.addr.family.width(),
        };
        if let Some(&z) = index.get(&(a.domain, full.truncate(len))) {
            zones[z].addresses.push(a.id);
        }
    }
    zones.retain(|z| !z.addresses.is_empty());
    zones
}

/// A changeset of `author`, timed, whatever its outcome.
async fn timed(
    store: &PgStore,
    author: Uuid,
    operations: Vec<Operation>,
) -> anyhow::Result<(Duration, Outcome)> {
    let request = ChangesetRequest {
        author,
        motive: "bench".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
    };
    let started = Instant::now();
    let outcome = submit(store, &Unrestricted, &request, MAX_OPERATIONS).await?;
    Ok((started.elapsed(), outcome))
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

/// Loads the dataset, deploys the modules, the policy and the flows, and measures;
/// returns the report and whether every budget holds.
///
/// # Errors
///
/// Database, pipeline or router failures.
#[allow(clippy::too_many_lines)] // one linear scenario, step by step
#[allow(clippy::many_single_char_names)] // the indices of the grid of the policy
pub async fn run(database: &str, data: &Dataset, seed: u64) -> anyhow::Result<(String, bool)> {
    let started = Instant::now();
    let Prepared {
        pool,
        author,
        mut report,
        ..
    } = prepare(database, data, seed).await?;
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(11));
    // The flows: 10⁵ on the reference dataset, else a tenth of the addresses, as there.
    let (flows, hot_flows) = if data.addresses.len() >= FLOWS * 2 {
        (FLOWS, HOT)
    } else {
        let flows = data.addresses.len() / 10;
        (flows, flows / 10)
    };
    let environments = environments_for(flows);

    // The deployment of a build with the four modules, as `vlan_budgets` does: the
    // declarations without their obligations, an owner on each loaded domain, then all.
    let modules: Vec<Arc<dyn Module>> = vec![
        Arc::new(kartenn_vlan::Vlan),
        Arc::new(kartenn_machines::Machines),
        Arc::new(kartenn_policy::Policy),
        Arc::new(kartenn_flow::Flows),
    ];
    let registry = Arc::new(ModuleRegistry::new(modules)?);
    kartenn_store_pg::modules::migrate(&pool, &registry).await?;
    let store = PgStore::new(pool.clone()).with_modules(registry.clone());
    let declarations: Vec<_> = std::iter::once(core_declarations_for(&registry))
        .chain(registry.modules().filter_map(|m| m.declarations()))
        .collect();
    let mut relaxed = declarations.clone();
    for module in &mut relaxed {
        for (declaration, _) in &mut module.properties {
            declaration.obligations.clear();
        }
    }
    sync(&store, &relaxed).await?;
    let mut entities = Vec::new();
    for e in 0..ENTITIES {
        let slug = format!("bench-flow-{e}");
        unitary(
            &store,
            author,
            Operation::CreateEntity {
                local: None,
                slug: slug.clone(),
                parent: None,
            },
        )
        .await?;
        entities.push(
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM entity WHERE slug = $1 AND valid_period @> now()",
            )
            .bind(slug)
            .fetch_one(&pool)
            .await?,
        );
    }
    let property = |key: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM property_declaration WHERE key = $1")
                .bind(key)
                .fetch_one(&pool)
                .await
                .with_context(|| format!("the property {key}"))
        }
    };
    let owner = property("core.owner").await?;
    let domains: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM addressing_domain WHERE upper_inf(valid_period)")
            .fetch_all(&pool)
            .await?;
    let items = domains
        .iter()
        .enumerate()
        .map(|(i, domain)| {
            item(Operation::AssignProperty {
                object: Ref::Id(*domain),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(entities[i % ENTITIES].to_string())],
            })
        })
        .collect();
    import(&store, author, "bench-flow-owners", items).await?;
    sync(&store, &declarations).await?;
    // The approval policies installed by the modules would hold a changeset that breaks a
    // flow (011-6): the measures are of the write itself.
    sqlx::query("UPDATE approval_policy SET enabled = false")
        .execute(&pool)
        .await?;
    progress(&format!("flow modules: {:?}", started.elapsed()));

    // The policy: environments, flow types and their services, the rules.
    let t = Instant::now();
    let mut items = Vec::new();
    for e in 0..environments {
        items.push(item(op(
            "policy",
            "environment.create",
            json!({"key": environment(e)}),
        )?));
    }
    for k in 0..FLOW_TYPES {
        items.push(named(
            format!("type-{k}"),
            op(
                "policy",
                "flow_type.create",
                json!({"local": k + 1, "key": format!("type{k:02}"),
                       "label": format!("Type {k}")}),
            )?,
        ));
        for s in 0..SERVICES_PER_TYPE {
            items.push(named(
                format!("service-{k}-{s}"),
                op(
                    "policy",
                    "service.create",
                    json!({"key": format!("svc{k:02}-{s:02}"),
                           "label": format!("Service {k}-{s}"),
                           "flow_type": {"local": k + 1},
                           "entries": [{"transport": "tcp",
                                        "ports": 1000 + k * SERVICES_PER_TYPE + s}]}),
                )?,
            ));
        }
    }
    for k in 0..FLOW_TYPES {
        for i in 0..environments {
            for b in 0..environments.div_ceil(BLOCK) {
                let deny = denied(i, b * BLOCK, k);
                let destinations: Vec<String> = (b * BLOCK..((b + 1) * BLOCK).min(environments))
                    .map(environment)
                    .collect();
                let mut rule = json!({
                    "key": format!("r-{k}-{i}-{b}"),
                    "label": format!("{} vers le bloc {b} ({k})", environment(i)),
                    "effect": if deny { "deny" } else { "allow" },
                    "flow_types": [{"local": k + 1}],
                    "source": {"policy.environment": [environment(i)]},
                    "destination": {"policy.environment": destinations},
                });
                if deny {
                    rule["texts"] = json!({"fr": {"body": "Flux interdit par le banc."}});
                }
                items.push(named(
                    format!("rule-{k}-{i}-{b}"),
                    op("policy", "rule.create", rule)?,
                ));
            }
        }
    }
    let rules = FLOW_TYPES * environments * environments.div_ceil(BLOCK);
    import(&store, author, "bench-flow-policy", items).await?;
    writeln!(
        report,
        "INFO policy: {environments} environments, {FLOW_TYPES} flow types, {} services, \
         {rules} rules (no pattern, condition nor signal), loaded in {:?}",
        FLOW_TYPES * SERVICES_PER_TYPE,
        t.elapsed()
    )?;
    progress(&format!("policy: {:?}", started.elapsed()));

    // The environments on the first containers below the roots.
    let mut zones = zones(data, environments);
    if zones.len() < 4 {
        bail!(
            "{} containers to classify, 4 at least expected",
            zones.len()
        );
    }
    let classification = property("policy.environment").await?;
    let classify = |zone: Uuid, e: usize| Operation::AssignProperty {
        object: Ref::Id(zone),
        kind: ObjectKind::Prefix,
        property: Ref::Id(classification),
        values: vec![RawInput::Text(environment(e))],
    };
    let t = Instant::now();
    let items = zones
        .iter()
        .map(|z| item(classify(z.id, z.environment)))
        .collect();
    import(&store, author, "bench-flow-classes", items).await?;
    writeln!(
        report,
        "INFO classification: {} containers, {} addresses, in {:?}",
        zones.len(),
        zones.iter().map(|z| z.addresses.len()).sum::<usize>(),
        t.elapsed()
    )?;
    progress(&format!("classification: {:?}", started.elapsed()));

    // The hot container 10⁴ flows leave and the cold one no flow touches: two of the
    // largest containers of one domain, of comparable size.
    zones.sort_by_key(|z| (z.domain, std::cmp::Reverse(z.addresses.len())));
    let (hot, cold) = (0, 1);
    if zones[hot].domain != zones[cold].domain {
        bail!("no domain with two classified containers");
    }
    let services: Vec<Uuid> = sqlx::query_scalar(
        "SELECT s.id FROM policy.policy_service s WHERE s.key LIKE 'svc%' ORDER BY s.key",
    )
    .fetch_all(&pool)
    .await?;
    if services.len() != FLOW_TYPES * SERVICES_PER_TYPE {
        bail!(
            "{} services, {} expected",
            services.len(),
            FLOW_TYPES * SERVICES_PER_TYPE
        );
    }
    // A service of the flow type `k`.
    let service = |k: usize, rng: &mut ChaCha8Rng| {
        services[k * SERVICES_PER_TYPE + rng.random_range(0..SERVICES_PER_TYPE)]
    };

    // The flows, allowed by construction (a denied one needs a waiver).
    let others: Vec<usize> = (2..zones.len()).collect();
    let mut drawn = HashSet::new();
    let mut items = Vec::with_capacity(flows);
    while items.len() < flows {
        let n = items.len();
        let source_zone = if n < hot_flows {
            hot
        } else {
            others[rng.random_range(0..others.len())]
        };
        let destination_zone = others[rng.random_range(0..others.len())];
        let (s, d) = (&zones[source_zone], &zones[destination_zone]);
        let k = rng.random_range(0..FLOW_TYPES);
        if denied(s.environment, d.environment, k) {
            continue;
        }
        let source = s.addresses[rng.random_range(0..s.addresses.len())];
        let destination = d.addresses[rng.random_range(0..d.addresses.len())];
        let used = service(k, &mut rng);
        if source == destination || !drawn.insert((source, destination, used)) {
            continue;
        }
        items.push(named(
            format!("flow-{n}"),
            op(
                "flow",
                "register",
                json!({
                    "source": {"kind": "address", "id": {"id": source}},
                    "destination": {"kind": "address", "id": {"id": destination}},
                    "service": {"id": used},
                    "external_refs": [{"system": Uuid::nil(), "id": format!("BENCH-{n}")}],
                    "entity": {"id": entities[n % ENTITIES]},
                }),
            )?,
        ));
    }
    let t = Instant::now();
    import(&store, author, "bench-flow-flows", items).await?;
    let loaded = t.elapsed();
    sqlx::raw_sql(
        "VACUUM ANALYZE flow.flow, flow.flow_endpoint_address, flow.flow_evaluation, \
         flow.flow_summary, flow.flow_registration",
    )
    .execute(&pool)
    .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM flow.flow")
        .fetch_one(&pool)
        .await?;
    if count != i64::try_from(flows)? {
        bail!("{count} flows registered, {flows} expected");
    }
    writeln!(
        report,
        "INFO flows: {flows} registered ({hot_flows} leaving one container), in {loaded:?}"
    )?;
    progress(&format!("flows: {:?}", started.elapsed()));

    // Machines of 16 addresses each, in pairs whose cells allow one of the services.
    let mut machine_items = Vec::new();
    let mut pairs = Vec::new();
    let mut local = 0;
    // Each machine takes the next addresses of its container: an address has one interface.
    let mut taken = vec![0; zones.len()];
    let mut add_machine = |addresses: &[Uuid], m: usize, items: &mut Vec<ImportItem>| {
        local += 2;
        let (machine, interface) = (local - 1, local);
        items.push(ImportItem {
            external_id: Some(format!("bench-flow-machine-{m}")),
            operation: op(
                "machines",
                "create_machine",
                json!({"local": machine, "name": format!("flowhost{m}"), "kind": "physical",
                       "owner": {"id": entities[m % ENTITIES]}}),
            )?,
        });
        items.push(named(
            format!("bench-flow-interface-{m}"),
            op(
                "machines",
                "create_interface",
                json!({"local": interface, "machine": {"local": machine}, "name": "eth0"}),
            )?,
        ));
        for a in addresses {
            items.push(item(op(
                "machines",
                "attach_address",
                json!({"interface": {"local": interface}, "address": {"id": a}}),
            )?));
        }
        anyhow::Ok(())
    };
    let mut m = 0;
    'pairs: for x in 2..zones.len() {
        for y in 2..zones.len() {
            if pairs.len() == PAIRS {
                break 'pairs;
            }
            let (a, b) = (&zones[x], &zones[y]);
            let free = |z: usize| zones[z].addresses.len() >= taken[z] + PER_MACHINE;
            if x == y || !free(x) || !free(y) {
                continue;
            }
            let Some(k) = (0..FLOW_TYPES).find(|&k| !denied(a.environment, b.environment, k))
            else {
                continue;
            };
            for (z, n) in [(x, m), (y, m + 1)] {
                let addresses = &zones[z].addresses[taken[z]..taken[z] + PER_MACHINE];
                taken[z] += PER_MACHINE;
                add_machine(addresses, n, &mut machine_items)?;
            }
            pairs.push((m, m + 1, k));
            m += 2;
        }
    }
    if pairs.is_empty() {
        bail!("no pair of containers for the machines 16 × 16");
    }
    import(&store, author, "bench-flow-machines", machine_items).await?;
    // Measured in a steady state: without fresh statistics after these imports, the
    // planner of the resolution of the properties may choose a plan seconds slow.
    sqlx::raw_sql(
        "VACUUM ANALYZE machines.machine, machines.interface, machines.attachment, \
         module_object, property_origin, property_assignment",
    )
    .execute(&pool)
    .await?;
    let machines: HashMap<String, Uuid> = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT e.external_id, e.object_id FROM external_id e
          WHERE e.source = 'bench-flow-machines'",
    )
    .fetch_all(&pool)
    .await?
    .into_iter()
    .collect();
    let machine = |m: usize| {
        machines
            .get(&format!("bench-flow-machine-{m}"))
            .copied()
            .context("a machine of the requests is missing")
    };
    progress(&format!("machines: {:?}", started.elapsed()));
    let mut budgets = Vec::new();

    // The request of a flow machine × machine, its 256 pairs evaluated and triaged.
    let mut samples = Vec::new();
    let mut applied = 0;
    for (n, &(a, b, k)) in pairs.iter().enumerate() {
        let operation = op(
            "flow",
            "request",
            json!({
                "local": 1,
                "source": {"kind": "machine", "id": {"id": machine(a)?}},
                "destination": {"kind": "machine", "id": {"id": machine(b)?}},
                "service": {"id": services[k * SERVICES_PER_TYPE]},
                "external_refs": [{"system": Uuid::nil(), "id": format!("BENCH-M-{n}")}],
                "entity": {"id": entities[n % ENTITIES]},
            }),
        )?;
        let (elapsed, outcome) = timed(&store, author, vec![operation]).await?;
        match outcome {
            Outcome::Applied { .. } => applied += 1,
            Outcome::Pending { .. } => {}
            other => bail!("a request machine × machine not taken: {other:?}"),
        }
        samples.push(elapsed);
    }
    budgets.push(Budget {
        name: "request a flow machine x machine 16 x 16, triage included, p95",
        limit: Duration::from_millis(300),
        measured: p95(samples),
        note: format!(
            "{} requests of 256 pairs, {applied} applied, the others held for review",
            pairs.len()
        ),
    });
    progress(&format!("requests: {:?}", started.elapsed()));

    // The reclassification of the hot container against the cold one, in turn.
    let mut warm = Vec::new();
    let mut idle = Vec::new();
    for round in 0..ROUNDS {
        for (z, samples) in [(cold, &mut idle), (hot, &mut warm)] {
            let e = (zones[z].environment + 1 + round) % environments;
            let (elapsed, outcome) = timed(&store, author, vec![classify(zones[z].id, e)]).await?;
            if !matches!(outcome, Outcome::Applied { .. }) {
                bail!("a reclassification not applied: {outcome:?}");
            }
            samples.push(elapsed);
        }
    }
    let (warm, idle) = (median(warm), median(idle));
    budgets.push(Budget {
        name: "re-evaluate the flows of a reclassified container, added",
        limit: Duration::from_secs(3),
        measured: warm.saturating_sub(idle),
        note: format!(
            "{hot_flows} flows: {warm:?} against {idle:?} for a container of {} addresses \
             no flow touches (medians of {ROUNDS})",
            zones[cold].addresses.len()
        ),
    });
    progress(&format!("reclassification: {:?}", started.elapsed()));

    // Writes of the policy: a rule re-evaluates the flows of its flow type, a new
    // environment every living flow (no background job re-evaluates them otherwise, F14).
    let (first, concerned): (Uuid, i64) = sqlx::query_as(
        "SELECT t.id, (SELECT count(*) FROM flow.flow f JOIN policy.policy_service s
                         ON s.id = f.service_id WHERE s.flow_type_id = t.id)
           FROM policy.policy_flow_type t WHERE t.key = 'type00'",
    )
    .fetch_one(&pool)
    .await?;
    let rule = op(
        "policy",
        "rule.create",
        json!({"key": "bench-new", "label": "Nouvelle règle", "effect": "allow",
               "flow_types": [{"id": first}],
               "source": {"policy.environment": [environment(0)]},
               "destination": {"policy.environment": [environment(1)]}}),
    )?;
    let environment = op("policy", "environment.create", json!({"key": "bench-new"}))?;
    for (name, operation, note) in [
        (
            "re-evaluate the flows of a flow type on a rule written",
            rule,
            format!("{concerned} flows of the 50 services of the flow type of a new rule"),
        ),
        (
            "re-evaluate every flow on a write of the policy",
            environment,
            format!("{flows} living flows, a new environment"),
        ),
    ] {
        let (elapsed, outcome) = timed(&store, author, vec![operation]).await?;
        if !matches!(outcome, Outcome::Applied { .. }) {
            bail!("a write of the policy not applied: {outcome:?}");
        }
        budgets.push(Budget {
            name,
            limit: Duration::from_mins(1),
            measured: elapsed,
            note,
        });
    }
    progress(&format!("policy write: {:?}", started.elapsed()));

    // Filtered pages through the router of the module.
    let identity = reader(
        &pool,
        &[
            "flow.read",
            "core.address.read",
            "core.prefix.read",
            "machines.read",
            "machines.interface.read",
            "policy.catalogue.read",
        ],
    )
    .await?;
    let app = app(
        &pool,
        registry,
        vec![kartenn_flow::rest::routes()],
        identity,
    )?;
    for _ in 0..10 {
        get(
            &app,
            &format!("/v1/flows?entity={}&page_size=100", entities[0]),
        )
        .await?;
    }
    let mut samples = Vec::new();
    let mut rows = 0;
    for _ in 0..100 {
        let entity = entities[rng.random_range(0..ENTITIES)];
        let service = services[rng.random_range(0..services.len())];
        let uri = format!("/v1/flows?entity={entity}&service={service}&page_size=100");
        let (elapsed, page) = get(&app, &uri).await?;
        rows += page["items"].as_array().map_or(0, Vec::len);
        samples.push(elapsed);
    }
    if rows == 0 {
        bail!("the filtered pages of flows are empty");
    }
    budgets.push(Budget {
        name: "filtered page of 100 flows, p95",
        limit: Duration::from_millis(150),
        measured: p95(samples),
        note: format!(
            "100 draws of the flows of an entity for a service, {} rows a page on average",
            rows / 100
        ),
    });
    let mut samples = Vec::new();
    let mut rows = 0;
    for _ in 0..100 {
        let entity = entities[rng.random_range(0..ENTITIES)];
        let uri = format!("/v1/flow-nonconformities?entity={entity}&page_size=100");
        let (elapsed, page) = get(&app, &uri).await?;
        rows += page["items"].as_array().map_or(0, Vec::len);
        samples.push(elapsed);
    }
    budgets.push(Budget {
        name: "page of non-conformities, p95",
        limit: Duration::from_millis(150),
        measured: p95(samples),
        note: format!(
            "100 draws of an entity, {} rows a page on average",
            rows / 100
        ),
    });
    progress(&format!("pages: {:?}", started.elapsed()));

    let (lines, ok) = verdict(&budgets)?;
    report.push_str(&lines);
    Ok((report, ok))
}
