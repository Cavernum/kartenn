//! Budgets of the machines and interfaces module (spec 009, REQ-009-21; task 009-12) on
//! the reference dataset augmented with 10⁵ machines (60 % virtual, hosted by 10³
//! clusters), 3·10⁵ interfaces and 3·10⁵ attachments to the addresses of the dataset:
//! reading a machine with its interfaces and addresses, a filtered page and the machines
//! of a /24 through the router of the module, and the unitary changeset that creates a
//! machine through the write pipeline. Then the import of the acceptance of REQ-009-19:
//! 10⁴ machines with two interfaces and two new addresses each, by local names.
//!
//! Runs after the VLAN budgets, on the same database: the machines module depends on the
//! VLAN module, whose schema, declarations and domain owners are already there.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::bail;
use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_service::declarations::core_declarations_for;
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::RawInput;
use kartenn_service::port::ImportItem;
use kartenn_service::{ObjectKind, Operation, Ref};
use kartenn_store_pg::write::PgStore;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::Dataset;
use crate::core_budgets::unitary;
use crate::props::{Budget, p95, progress};
use crate::vlan_budgets::{app, get, import, reader, sync};

/// Machines, clusters and interfaces per machine of the reference dataset (REQ-009-21);
/// a smaller dataset keeps the reference ratio of a machine per ten of its addresses, as
/// many clusters as a hundredth of the machines, and imports a tenth of the machines.
const MACHINES: usize = 100_000;
const CLUSTERS: usize = 1_000;
const INTERFACES: usize = 3;
/// Physical members of each cluster, among the first physical machines.
const MEMBERS: usize = 4;
/// Machines of the import of the acceptance of REQ-009-19.
const IMPORTED: usize = 10_000;

/// `machines.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> anyhow::Result<Operation> {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    Ok(serde_json::from_value(object)?)
}

/// Clusters, machines, their interfaces and one attachment per interface to an address
/// of the dataset, in that order, by local names: `addresses[m * INTERFACES + i]` on the
/// interface `i` of the machine `m`.
fn dataset_items(
    entities: &[Uuid],
    addresses: &[Uuid],
    machines: usize,
    clusters: usize,
) -> anyhow::Result<Vec<ImportItem>> {
    let cluster = |c: usize| c + 1;
    let machine = |m: usize| clusters + m + 1;
    let interface = |m: usize, i: usize| clusters + machines + m * INTERFACES + i + 1;
    let physical = machines * 2 / 5;
    let mut items = Vec::with_capacity(clusters + machines * (1 + 2 * INTERFACES));
    for c in 0..clusters {
        items.push(ImportItem {
            external_id: Some(format!("cluster-{c}")),
            operation: op(
                "create_cluster",
                json!({"local": cluster(c), "slug": format!("bench-{c}"),
                       "owner": {"id": entities[c % entities.len()]}}),
            )?,
        });
    }
    for m in 0..machines {
        let mut fields = json!({"local": machine(m), "name": format!("host{m}"),
                                "owner": {"id": entities[m % entities.len()]}});
        if m < physical {
            fields["kind"] = "physical".into();
            if m < clusters * MEMBERS {
                fields["member_of"] = json!({"local": cluster(m / MEMBERS)});
            }
        } else {
            fields["kind"] = "virtual".into();
            fields["host"] = json!({"cluster": {"local": cluster(m % clusters)}});
        }
        items.push(ImportItem {
            external_id: Some(format!("machine-{m}")),
            operation: op("create_machine", fields)?,
        });
    }
    for m in 0..machines {
        for i in 0..INTERFACES {
            items.push(ImportItem {
                external_id: Some(format!("interface-{m}-{i}")),
                operation: op(
                    "create_interface",
                    json!({"local": interface(m, i), "machine": {"local": machine(m)},
                           "name": format!("eth{i}")}),
                )?,
            });
        }
    }
    for m in 0..machines {
        for i in 0..INTERFACES {
            items.push(ImportItem {
                external_id: None,
                operation: op(
                    "attach_address",
                    json!({"interface": {"local": interface(m, i)},
                           "address": {"id": addresses[m * INTERFACES + i]}}),
                )?,
            });
        }
    }
    Ok(items)
}

/// The import of the acceptance of REQ-009-19: `count` machines with two interfaces and
/// two new addresses each in `domain`, attachments first so that the import orders them
/// after what they cite.
fn acceptance_items(owner: Uuid, domain: Uuid, count: usize) -> anyhow::Result<Vec<ImportItem>> {
    let local = |m: usize, k: usize| m * 5 + k + 1;
    let address = |m: usize, n: usize| {
        let index = m * 2 + n;
        format!(
            "10.{}.{}.{}",
            index / 65_024,
            index / 254 % 256,
            index % 254 + 1
        )
    };
    let mut items = Vec::with_capacity(count * 7);
    for m in 0..count {
        for n in 0..2 {
            items.push(ImportItem {
                external_id: None,
                operation: op(
                    "attach_address",
                    json!({"interface": {"local": local(m, 1 + n)},
                           "address": {"local": local(m, 3 + n)}}),
                )?,
            });
        }
    }
    for m in 0..count {
        items.push(ImportItem {
            external_id: Some(format!("machine-{m}")),
            operation: op(
                "create_machine",
                json!({"local": local(m, 0), "name": format!("imported{m}"),
                       "kind": "physical", "owner": {"id": owner}}),
            )?,
        });
        for n in 0..2 {
            items.push(ImportItem {
                external_id: Some(format!("interface-{m}-{n}")),
                operation: op(
                    "create_interface",
                    json!({"local": local(m, 1 + n), "machine": {"local": local(m, 0)},
                           "name": format!("eth{n}")}),
                )?,
            });
            items.push(ImportItem {
                external_id: Some(format!("address-{m}-{n}")),
                operation: Operation::CreateAddress {
                    local: Some(u32::try_from(local(m, 3 + n))?),
                    domain: Ref::Id(domain),
                    address: address(m, n),
                    usage: AddressUsage::Host,
                },
            });
        }
    }
    Ok(items)
}

/// Deploys the module beside the VLAN module, loads its machines, interfaces and
/// attachments, and measures; appends to `report` and returns the budgets.
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
    // The deployment of a build with both modules: the schema of the new one, then the
    // core completed by the types of both and their declarations.
    let modules: Vec<Arc<dyn Module>> = vec![
        Arc::new(kartenn_vlan::Vlan),
        Arc::new(kartenn_machines::Machines),
    ];
    let registry = Arc::new(ModuleRegistry::new(modules)?);
    kartenn_store_pg::modules::migrate(pool, &registry).await?;
    let store = PgStore::new(pool.clone()).with_modules(registry.clone());
    let declarations: Vec<_> = std::iter::once(core_declarations_for(&registry))
        .chain(registry.modules().filter_map(|m| m.declarations()))
        .collect();
    sync(&store, &declarations).await?;

    // The owners: the entities of the VLAN budgets.
    let entities: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM entity WHERE slug LIKE 'bench-vlan-%' AND valid_period @> now() ORDER BY slug",
    )
    .fetch_all(pool)
    .await?;
    if entities.is_empty() {
        bail!("no entity of the VLAN budgets to own the machines");
    }
    // Three addresses of the dataset per machine, in the order of their networks.
    let (machines, clusters) = if data.addresses.len() >= MACHINES * INTERFACES {
        (MACHINES, CLUSTERS)
    } else {
        let machines = data.addresses.len() / 10;
        (machines, machines.div_ceil(100))
    };
    let addresses: Vec<Uuid> = data
        .addresses
        .iter()
        .take(machines * INTERFACES)
        .map(|a| a.id)
        .collect();
    import(
        &store,
        author,
        "bench-machines",
        dataset_items(&entities, &addresses, machines, clusters)?,
    )
    .await?;
    sqlx::raw_sql(
        "VACUUM ANALYZE machines.cluster, machines.machine, machines.interface, \
         machines.attachment, module_object",
    )
    .execute(pool)
    .await?;
    writeln!(
        report,
        "INFO machines dataset: {clusters} clusters, {machines} machines ({} virtual), \
         {} interfaces and attachments, loaded in {:?}",
        machines - machines * 2 / 5,
        machines * INTERFACES,
        started.elapsed()
    )?;
    progress(&format!("machines dataset: {:?}", started.elapsed()));

    let machine_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT m.id FROM machines.machine m JOIN external_id e ON e.object_id = m.id
          WHERE e.source = 'bench-machines' AND upper_inf(m.valid_period)",
    )
    .fetch_all(pool)
    .await?;
    if machine_ids.len() != machines {
        bail!("{} machines loaded, {machines} expected", machine_ids.len());
    }
    // The /24 networks that hold an attached address.
    let networks: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT p.id FROM machines.attachment t
           JOIN address a ON a.id = t.address_id AND upper_inf(a.valid_period)
           JOIN prefix p ON p.domain_id = a.domain_id AND p.prefix >> a.address
                        AND p.nature = 'network' AND masklen(p.prefix) = 24
                        AND upper_inf(p.valid_period)
          WHERE upper_inf(t.valid_period)
          LIMIT 1000",
    )
    .fetch_all(pool)
    .await?;
    if networks.is_empty() {
        bail!("no /24 network holds an attached address");
    }
    let identity = reader(
        pool,
        &[
            "machines.cluster.read",
            "machines.machine.read",
            "machines.interface.read",
            "core.address.read",
            "core.prefix.read",
            "vlan.vlan.read",
        ],
    )
    .await?;
    let app = app(
        pool,
        registry,
        vec![
            kartenn_vlan::rest::routes(),
            kartenn_machines::rest::routes(),
        ],
        identity,
    )?;
    let mut budgets = Vec::new();

    // Warm-up of the connections and statements.
    for _ in 0..20 {
        let machine = machine_ids[rng.random_range(0..machine_ids.len())];
        get(&app, &format!("/v1/machines/{machine}")).await?;
    }

    // Reading a machine, its card with its interfaces and their addresses.
    let mut samples = Vec::new();
    let mut interfaces = 0;
    for _ in 0..200 {
        let machine = machine_ids[rng.random_range(0..machine_ids.len())];
        let (elapsed, card) = get(&app, &format!("/v1/machines/{machine}")).await?;
        interfaces += card["interfaces"].as_array().map_or(0, Vec::len);
        samples.push(elapsed);
    }
    if interfaces == 0 {
        bail!("the machine cards carry no interface");
    }
    budgets.push(Budget {
        name: "read a machine with its interfaces and addresses, p95",
        limit: Duration::from_millis(50),
        measured: p95(samples),
        note: format!(
            "200 draws, through the router, {} interfaces a card on average",
            interfaces / 200
        ),
    });

    // Filtered pages of 100: the virtual machines of an owner.
    let mut samples = Vec::new();
    let mut rows = 0;
    for _ in 0..100 {
        let owner = entities[rng.random_range(0..entities.len())];
        let uri = format!("/v1/machines?kind=virtual&owner={owner}&page_size=100");
        let (elapsed, page) = get(&app, &uri).await?;
        rows += page["items"].as_array().map_or(0, Vec::len);
        samples.push(elapsed);
    }
    budgets.push(Budget {
        name: "filtered page of 100 machines, p95",
        limit: Duration::from_millis(150),
        measured: p95(samples),
        note: format!(
            "100 draws of the virtual machines of an owner, {} rows a page on average",
            rows / 100
        ),
    });

    // The machines of a /24.
    let mut samples = Vec::new();
    let mut rows = 0;
    for _ in 0..200 {
        let network = networks[rng.random_range(0..networks.len())];
        let (elapsed, page) = get(&app, &format!("/v1/machines?network={network}")).await?;
        rows += page["items"].as_array().map_or(0, Vec::len);
        samples.push(elapsed);
    }
    if rows == 0 {
        bail!("no machine found in the drawn networks");
    }
    budgets.push(Budget {
        name: "machines of a /24, p95",
        limit: Duration::from_millis(150),
        measured: p95(samples),
        note: format!("200 draws, {} machines a network on average", rows / 200),
    });

    // Unitary changesets: a new machine for a random owner.
    let mut samples = Vec::new();
    for i in 0..100 {
        let owner = entities[rng.random_range(0..entities.len())];
        let operation = op(
            "create_machine",
            json!({"name": format!("new{i}"), "kind": "physical", "owner": {"id": owner}}),
        )?;
        samples.push(unitary(&store, author, operation).await?);
    }
    budgets.push(Budget {
        name: "create a machine, p95",
        limit: Duration::from_millis(200),
        measured: p95(samples),
        note: "100 unitary changesets, validated and applied".into(),
    });
    progress(&format!("machines measures: {:?}", started.elapsed()));

    // The import of the acceptance of REQ-009-19, in a domain of its own.
    let imported = (machines * IMPORTED / MACHINES).max(1);
    let owner: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
            .fetch_one(pool)
            .await?;
    // A domain carries its owner from its creation (REQ-003-16): one changeset.
    let operations = [
        Operation::CreateDomain {
            local: Some(1),
            slug: "bench-machines-import".into(),
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(entities[0].to_string())],
        },
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Local(1),
            prefix: "10.0.0.0/8".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
    ];
    let items = operations
        .into_iter()
        .zip(["domain", "", "network"])
        .map(|(operation, name)| ImportItem {
            external_id: (!name.is_empty()).then(|| name.to_owned()),
            operation,
        })
        .collect();
    import(&store, author, "bench-machines-domain", items).await?;
    let domain: Uuid = sqlx::query_scalar(
        "SELECT id FROM addressing_domain WHERE slug = 'bench-machines-import' AND valid_period @> now()",
    )
    .fetch_one(pool)
    .await?;
    let t = Instant::now();
    import(
        &store,
        author,
        "bench-machines-import",
        acceptance_items(entities[0], domain, imported)?,
    )
    .await?;
    let elapsed = t.elapsed();
    let attached: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM machines.attachment t
           JOIN address a ON a.id = t.address_id WHERE a.domain_id = $1",
    )
    .bind(domain)
    .fetch_one(pool)
    .await?;
    if attached != i64::try_from(imported * 2)? {
        bail!(
            "{attached} addresses attached by the import, {} expected",
            imported * 2
        );
    }
    writeln!(
        report,
        "INFO machines import: {imported} machines, {} interfaces, addresses and attachments \
         by local names, in {elapsed:?}",
        imported * 2
    )?;
    progress(&format!("machines import: {:?}", started.elapsed()));
    Ok(budgets)
}
