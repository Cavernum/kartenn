//! The operations of the module in the imports in batches (spec 009, REQ-009-19; task
//! 009-11): machines with two interfaces and two addresses each, created and attached by
//! local names, end; a replay after a stop duplicates nothing. 10³ machines here (1.1·10⁴
//! operations, as the import of the VLANs); the volume of the acceptance, 10⁴ machines,
//! is the bench's (task 12).

use std::sync::Arc;

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_machines::Machines;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{ModuleDeclarations, core_declarations_for, sync_modules};
use kartenn_service::import::{create_import, run_import};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::ImportItem;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use kartenn_vlan::Vlan;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

const SOURCE: &str = "legacy-inventory";
const MACHINES: u32 = 1_000;

/// `machines.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// The store with both modules, its author, the entity `acme` and the domain `lab` with
/// the network 10.0.0.0/16.
async fn setup(pool: &PgPool) -> (PgStore, Uuid, Uuid, Uuid) {
    let modules: Vec<Arc<dyn Module>> = vec![Arc::new(Vlan), Arc::new(Machines)];
    let registry = Arc::new(ModuleRegistry::new(modules).unwrap());
    kartenn_store_pg::modules::migrate(pool, &registry)
        .await
        .unwrap();
    let store = PgStore::new(pool.clone()).with_modules(registry.clone());
    let declarations: Vec<ModuleDeclarations> = std::iter::once(core_declarations_for(&registry))
        .chain(registry.modules().filter_map(|m| m.declarations()))
        .collect();
    for outcome in sync_modules(&store, &Unrestricted, &declarations, MAX_OPERATIONS)
        .await
        .unwrap()
    {
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let applied = |operations: Vec<Operation>| {
        let store = &store;
        async move {
            let request = ChangesetRequest {
                author,
                motive: "test".into(),
                period: None,
                operations,
                simulate: false,
                idempotency_key: None,
                apply_own: true,
            };
            let outcome = submit(store, &Unrestricted, &request, MAX_OPERATIONS)
                .await
                .unwrap();
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
    };
    applied(vec![Operation::CreateEntity {
        local: None,
        slug: "acme".into(),
        parent: None,
    }])
    .await;
    let acme: Uuid =
        sqlx::query_scalar("SELECT id FROM entity WHERE slug = 'acme' AND valid_period @> now()")
            .fetch_one(pool)
            .await
            .unwrap();
    let owner: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
            .fetch_one(pool)
            .await
            .unwrap();
    applied(vec![
        Operation::CreateDomain {
            local: Some(1),
            slug: "lab".into(),
        },
        Operation::AssignProperty {
            object: Ref::Local(1),
            kind: ObjectKind::Domain,
            property: Ref::Id(owner),
            values: vec![RawInput::Text(acme.to_string())],
        },
        Operation::CreatePrefix {
            local: None,
            domain: Ref::Local(1),
            prefix: "10.0.0.0/16".into(),
            nature: Nature::Network,
            point_to_point: false,
            anycast: false,
        },
    ])
    .await;
    let domain: Uuid = sqlx::query_scalar(
        "SELECT id FROM addressing_domain WHERE slug = 'lab' AND valid_period @> now()",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    (store, author, acme, domain)
}

/// Per machine `m`: the machine, its interfaces `eth0` and `eth1`, an address for each,
/// and their attachments, all by local names; attachments given first, so that the import
/// orders them after what they cite.
fn items(acme: Uuid, domain: Uuid) -> Vec<ImportItem> {
    let local = |m: u32, k: u32| m * 10 + k + 1;
    let address = |m: u32, n: u32| {
        let index = m * 2 + n;
        format!("10.0.{}.{}", index / 254, index % 254 + 1)
    };
    let attachments = (0..MACHINES).flat_map(move |m| {
        (0..2).map(move |n| ImportItem {
            external_id: None,
            operation: op(
                "attach_address",
                json!({"interface": {"local": local(m, 1 + n)},
                       "address": {"local": local(m, 3 + n)}}),
            ),
        })
    });
    let objects = (0..MACHINES).flat_map(move |m| {
        let machine = ImportItem {
            external_id: Some(format!("machine-{m}")),
            operation: op(
                "create_machine",
                json!({"local": local(m, 0), "name": format!("host{m}"), "kind": "physical",
                       "owner": {"id": acme}}),
            ),
        };
        let interfaces = (0..2).map(move |n| ImportItem {
            external_id: Some(format!("interface-{m}-{n}")),
            operation: op(
                "create_interface",
                json!({"local": local(m, 1 + n), "machine": {"local": local(m, 0)},
                       "name": format!("eth{n}")}),
            ),
        });
        let addresses = (0..2).map(move |n| ImportItem {
            external_id: Some(format!("address-{m}-{n}")),
            operation: Operation::CreateAddress {
                local: Some(local(m, 3 + n)),
                domain: Ref::Id(domain),
                address: address(m, n),
                usage: AddressUsage::Host,
            },
        });
        std::iter::once(machine).chain(interfaces).chain(addresses)
    });
    attachments.chain(objects).collect()
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn import_machines_interfaces_attachments_by_local_names(pool: PgPool) {
    let (store, author, acme, domain) = setup(&pool).await;
    let import = create_import(&store, author, SOURCE, items(acme, domain), 2_000)
        .await
        .unwrap();
    let done = run_import(&store, &Unrestricted, import, 2_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.state, "done", "{}", done.report);
    let machines = "SELECT count(*) FROM machines.machine";
    let interfaces = "SELECT count(*) FROM machines.interface";
    let attachments = "SELECT count(*) FROM machines.attachment";
    let bound = "SELECT count(*) FROM external_id WHERE source = 'legacy-inventory'";
    let n = i64::from(MACHINES);
    assert_eq!(count(&pool, machines).await, n);
    assert_eq!(count(&pool, interfaces).await, 2 * n);
    assert_eq!(count(&pool, attachments).await, 2 * n);
    assert_eq!(count(&pool, bound).await, 5 * n);
    // Each address on the interface of its own machine, by the names of the source.
    let misplaced = "SELECT count(*) FROM machines.attachment a
                      JOIN external_id i ON i.object_id = a.interface_id
                      JOIN external_id x ON x.object_id = a.address_id AND x.source = i.source
                     WHERE substr(i.external_id, 11) <> substr(x.external_id, 9)";
    assert_eq!(count(&pool, misplaced).await, 0);
    let typed = "SELECT count(*) FROM external_id WHERE source = 'legacy-inventory'
                    AND object_type = CASE split_part(external_id, '-', 1)
                                        WHEN 'machine' THEN 'machines.machine'
                                        WHEN 'interface' THEN 'machines.interface'
                                        ELSE 'address' END";
    assert_eq!(count(&pool, typed).await, 5 * n);

    // A stop after the batches were applied but before they were recorded: the replay
    // leaves out every creation already known to the source, and an attachment already
    // made is not made twice.
    sqlx::query("UPDATE import SET state = 'running' WHERE id = $1")
        .bind(import)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE import_batch SET state = 'queued', changeset_id = NULL WHERE import_id = $1",
    )
    .bind(import)
    .execute(&pool)
    .await
    .unwrap();
    let replayed = run_import(&store, &Unrestricted, import, 2_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replayed.state, "done", "{}", replayed.report);
    assert_eq!(count(&pool, machines).await, n);
    assert_eq!(count(&pool, interfaces).await, 2 * n);
    assert_eq!(count(&pool, attachments).await, 2 * n);
    assert_eq!(count(&pool, bound).await, 5 * n);
}
