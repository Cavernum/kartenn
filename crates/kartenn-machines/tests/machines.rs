//! Clusters, machines and interfaces through the write pipeline (spec 009, REQ-009-1 to
//! 7; tasks 009-2 and 009-3): the dependency on the VLAN module, a name unique per
//! resolved owner, the hosting of a machine (M5), a cluster slug unique at every instant;
//! interfaces tied to their machine, named once per machine, with an EUI-48 stored as
//! `macaddr`, and inheriting the anchors of their machine; the 802.1Q mode of an
//! interface and its VLANs (task 009-4); addresses of the core attached to interfaces,
//! once unless shared, one primary per family, under the reading of the address (task
//! 009-5); the properties of the module and those other modules put on machines (task
//! 009-6).

use std::sync::Arc;
use std::time::Duration;

use kartenn_core::authz::Permission;
use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_core::properties::{Declarant, Declaration, PropertyKey, TypeSpec};
use kartenn_core::values::Cardinality;
use kartenn_machines::Machines;
use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::declarations::ModuleDeclarations;
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::module::{Module, ModuleRegistry, ObjectType};
use kartenn_service::operations::{ModuleKind, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::write::PgStore;
use kartenn_vlan::Vlan;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Env {
    pool: PgPool,
    registry: Arc<ModuleRegistry>,
    store: PgStore,
    author: Uuid,
}

impl Env {
    /// Both modules migrated, the properties of the core declared for their types, and the
    /// entities `acme` and `other` and the site `paris`.
    async fn new(pool: &PgPool) -> Self {
        Self::with(pool, Vec::new()).await
    }

    /// [`Env::new`] with the modules `extra` too, their declarations synced.
    async fn with(pool: &PgPool, extra: Vec<Arc<dyn Module>>) -> Self {
        let mut modules: Vec<Arc<dyn Module>> = vec![Arc::new(Vlan), Arc::new(Machines)];
        modules.extend(extra);
        let registry = Arc::new(ModuleRegistry::new(modules).unwrap());
        kartenn_store_pg::modules::migrate(pool, &registry)
            .await
            .unwrap();
        let store = PgStore::new(pool.clone()).with_modules(registry.clone());
        sync(&store, &registry).await;
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let env = Self {
            pool: pool.clone(),
            registry,
            store,
            author,
        };
        env.applied(vec![
            Operation::CreateEntity {
                local: Some(1),
                slug: "acme".into(),
                parent: None,
            },
            Operation::CreateEntity {
                local: Some(2),
                slug: "other".into(),
                parent: None,
            },
            Operation::CreateSite {
                local: Some(3),
                slug: "paris".into(),
                parent: None,
            },
        ])
        .await;
        env
    }

    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) -> Outcome {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        outcome
    }

    async fn refused(&self, operations: Vec<Operation>, rule: &str) {
        let outcome = self.submit(operations).await;
        assert!(rules(&outcome).contains(&rule), "{outcome:?}");
    }

    async fn property(&self, key: &str) -> Uuid {
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = $1")
            .bind(key)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn id(&self, table: &str, column: &str, value: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE {column} = $1 AND valid_period @> now()"
        )))
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn entity(&self, slug: &str) -> Uuid {
        self.id("entity", "slug", slug).await
    }

    /// The living machine named `name`, owned by `owner`.
    async fn machine(&self, name: &str, owner: Uuid) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM machines.machine
              WHERE name = $1 AND valid_period @> now() AND kartenn_owner(id, now()) = $2",
        )
        .bind(name)
        .bind(owner)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The interface named `name` of `machine`.
    async fn interface(&self, machine: Uuid, name: &str) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM machines.interface
              WHERE machine_id = $1 AND name = $2 AND valid_period @> now()",
        )
        .bind(machine)
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The resolved owner of `object` now.
    async fn owner(&self, object: Uuid) -> Option<Uuid> {
        sqlx::query_scalar("SELECT kartenn_owner($1, now())")
            .bind(object)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// A person granted the reading of machines and interfaces on the entity `entity`.
    async fn reader_on(&self, entity: Uuid) -> Uuid {
        self.granted(
            entity,
            &["machines.machine.read", "machines.interface.read"],
        )
        .await
    }

    /// A person granted `permissions` on the entity `entity`.
    async fn granted(&self, entity: Uuid, permissions: &[&str]) -> Uuid {
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'r') RETURNING id",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let role = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO role (id, key, label, valid_period, changeset_id)
             VALUES ($1, $2, 'r', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(format!("r_{}", role.simple()))
        .execute(&self.pool)
        .await
        .unwrap();
        for permission in permissions {
            sqlx::query(
                "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
                 VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
            )
            .bind(role)
            .bind(permission)
            .execute(&self.pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                                  valid_period, changeset_id)
             VALUES (gen_random_uuid(), 'identity', $1, $2, 'entity', $3,
                     tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(identity)
        .bind(role)
        .bind(entity)
        .execute(&self.pool)
        .await
        .unwrap();
        identity
    }

    /// Whether `reader` holds `permission` on `object`.
    async fn reads(&self, reader: Uuid, permission: &str, object: Uuid) -> bool {
        PgAuthorizer::new(self.pool.clone())
            .with_modules(self.registry.clone())
            .authorize_many(&Subject::from(reader), permission, &[object])
            .await
            .unwrap()
            .is_empty()
    }

    /// `operations` submitted by `identity`, under the RBAC of the store.
    async fn submit_as(&self, identity: Uuid, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author: identity,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        };
        let authorizer = PgAuthorizer::new(self.pool.clone()).with_modules(self.registry.clone());
        submit(&self.store, &authorizer, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    /// A group of VLANs anchored on `acme` and its VLANs `vids`, living now.
    async fn vlans(&self, vids: &[i16]) -> Vec<Uuid> {
        let acme = self.entity("acme").await;
        self.applied(vec![vop(
            "create_group",
            json!({"slug": "edge", "anchor": {"entity": {"id": acme}},
                   "vid_min": 1, "vid_max": 4094}),
        )])
        .await;
        let group = self.id("vlan.vlan_group", "slug", "edge").await;
        self.applied(
            vids.iter()
                .map(|vid| {
                    vop(
                        "create_vlan",
                        json!({"group": {"id": group}, "vid": vid, "name": format!("v{vid}")}),
                    )
                })
                .collect(),
        )
        .await;
        let mut ids = Vec::new();
        for vid in vids {
            ids.push(
                sqlx::query_scalar(
                    "SELECT id FROM vlan.vlan
                      WHERE group_id = $1 AND vid = $2 AND valid_period @> now()",
                )
                .bind(group)
                .bind(vid)
                .fetch_one(&self.pool)
                .await
                .unwrap(),
            );
        }
        ids
    }

    /// The 802.1Q columns of the interface `id` now.
    async fn vlan_columns(&self, id: Uuid) -> (String, Option<Uuid>, Option<Uuid>, Vec<Uuid>) {
        sqlx::query_as(
            "SELECT vlan_mode, access_vlan_id, native_vlan_id, tagged_vlan_ids
               FROM machines.interface WHERE id = $1 AND valid_period @> now()",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A domain `slug` owned by `owner`, with the network `network`.
    async fn domain(&self, slug: &str, owner: Uuid, network: &str) -> Uuid {
        let property = self.property("core.owner").await;
        self.applied(vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: slug.into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Domain,
                property: Ref::Id(property),
                values: vec![RawInput::Text(owner.to_string())],
            },
            network_op(Ref::Local(1), network),
        ])
        .await;
        self.id("addressing_domain", "slug", slug).await
    }

    /// The address `value` of `domain`, of `usage`, created now.
    async fn address(&self, domain: Uuid, value: &str, usage: AddressUsage) -> Uuid {
        self.applied(vec![Operation::CreateAddress {
            local: None,
            domain: Ref::Id(domain),
            address: value.into(),
            usage,
        }])
        .await;
        sqlx::query_scalar(
            "SELECT id FROM address
              WHERE domain_id = $1 AND address = $2::inet AND valid_period @> now()",
        )
        .bind(domain)
        .bind(value)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The interface `eth0` of a new machine `name` owned by `acme`.
    async fn eth0(&self, name: &str) -> Uuid {
        let m = self.create(name, "physical", json!({})).await;
        self.applied(vec![op(
            "create_interface",
            json!({"machine": {"id": m}, "name": "eth0"}),
        )])
        .await;
        self.interface(m, "eth0").await
    }

    /// The living attachments of `address`: `(interface, shared, primary)`.
    async fn attachments(&self, address: Uuid) -> Vec<(Uuid, bool, bool)> {
        sqlx::query_as(
            "SELECT interface_id, shared, primary_for_family FROM machines.attachment
              WHERE address_id = $1 AND valid_period @> now() ORDER BY interface_id",
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    fn request(&self, operations: Vec<Operation>) -> ChangesetRequest {
        ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        }
    }

    /// Creates a machine of `kind` owned by `acme`, with extra fields, and returns it.
    async fn create(&self, name: &str, kind: &str, extra: Value) -> Uuid {
        let acme = self.entity("acme").await;
        let mut fields = json!({"name": name, "kind": kind, "owner": {"id": acme}});
        merge(&mut fields, extra);
        self.applied(vec![op("create_machine", fields)]).await;
        self.machine(name, acme).await
    }
}

fn merge(into: &mut Value, extra: Value) {
    if let (Value::Object(into), Value::Object(extra)) = (into, extra) {
        into.extend(extra);
    }
}

/// Syncs the declarations of the core and of the modules of `registry`.
async fn sync(store: &PgStore, registry: &ModuleRegistry) {
    let declarations: Vec<ModuleDeclarations> = std::iter::once(core_declarations_for(registry))
        .chain(registry.modules().filter_map(|m| m.declarations()))
        .collect();
    for outcome in sync_modules(store, &Unrestricted, &declarations, MAX_OPERATIONS)
        .await
        .unwrap()
    {
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }
}

/// `machines.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// The network `network` of `domain`.
fn network_op(domain: Ref, network: &str) -> Operation {
    Operation::CreatePrefix {
        local: None,
        domain,
        prefix: network.into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    }
}

/// `machines.attach_address` of `address` to `interface`.
fn attach(interface: Uuid, address: Uuid, primary: bool) -> Operation {
    op(
        "attach_address",
        json!({"interface": {"id": interface}, "address": {"id": address}, "primary": primary}),
    )
}

/// `machines.detach_address` of `address` from `interface`.
fn detach(interface: Uuid, address: Uuid) -> Operation {
    op(
        "detach_address",
        json!({"interface": {"id": interface}, "address": {"id": address}}),
    )
}

/// `UpdateAddress` to `usage`.
fn usage(address: Uuid, usage: AddressUsage) -> Operation {
    Operation::UpdateAddress {
        id: Ref::Id(address),
        domain: None,
        usage,
        expected: None,
    }
}

/// The class of the named locks of the store (`write.rs`, REQ-008-27).
const NAMED_LOCK_CLASS: i32 = 0x6b61_726b;

/// `vlan.<name>` from its JSON fields.
fn vop(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("vlan.{name}").into();
    serde_json::from_value(object).unwrap()
}

fn rules(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn machines_needs_vlan() {
    let error = ModuleRegistry::new(vec![Arc::new(Machines)]).expect_err("vlan is missing");
    assert!(error.to_string().contains("vlan"), "{error}");
    ModuleRegistry::new(vec![Arc::new(Vlan), Arc::new(Machines)]).expect("both modules");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn name_unique_per_owner(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (acme, other) = (env.entity("acme").await, env.entity("other").await);
    // Without an owner: refused by the obligation of the core (REQ-009-3).
    env.refused(
        vec![op(
            "create_machine",
            json!({"name": "srv01", "kind": "physical"}),
        )],
        "prop.assignment.required",
    )
    .await;
    // Kept in lower case; a bad name or kind is refused.
    env.applied(vec![op(
        "create_machine",
        json!({"name": "SRV01", "kind": "physical", "owner": {"id": acme}}),
    )])
    .await;
    env.machine("srv01", acme).await;
    for bad in ["-srv", "srv_01"] {
        env.refused(
            vec![op(
                "create_machine",
                json!({"name": bad, "kind": "physical", "owner": {"id": acme}}),
            )],
            "machines.name.invalid",
        )
        .await;
    }
    env.refused(
        vec![op(
            "create_machine",
            json!({"name": "srv02", "kind": "mainframe", "owner": {"id": acme}}),
        )],
        "machines.operation.invalid",
    )
    .await;
    // The same name under another owner: accepted; twice under the same: refused.
    env.applied(vec![op(
        "create_machine",
        json!({"name": "srv01", "kind": "virtual", "owner": {"id": other}}),
    )])
    .await;
    env.refused(
        vec![op(
            "create_machine",
            json!({"name": "srv01", "kind": "appliance", "owner": {"id": acme}}),
        )],
        "machines.name.duplicate",
    )
    .await;
    // Renaming into a taken name: refused.
    let paris = env.id("site", "slug", "paris").await;
    env.applied(vec![op(
        "create_machine",
        json!({"name": "srv03", "kind": "physical", "owner": {"id": acme}, "site": {"id": paris}}),
    )])
    .await;
    let srv03 = env.machine("srv03", acme).await;
    env.refused(
        vec![op(
            "update_machine",
            json!({"id": {"id": srv03}, "name": "srv01"}),
        )],
        "machines.name.duplicate",
    )
    .await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn owner_change_into_taken_name_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (acme, other) = (env.entity("acme").await, env.entity("other").await);
    env.create("web", "physical", json!({})).await;
    env.applied(vec![op(
        "create_machine",
        json!({"name": "web", "kind": "physical", "owner": {"id": other}}),
    )])
    .await;
    let moving = env.machine("web", other).await;
    let owner = env.property("core.owner").await;
    let to = |entity: Uuid| Operation::AssignProperty {
        object: Ref::Id(moving),
        kind: ObjectKind::Module(ModuleKind::parse("machines.machine").unwrap()),
        property: Ref::Id(owner),
        values: vec![RawInput::Text(entity.to_string())],
    };
    env.refused(vec![to(acme)], "machines.name.duplicate").await;
    // Into a free name, the same move is accepted.
    env.applied(vec![
        op(
            "update_machine",
            json!({"id": {"id": moving}, "name": "web2"}),
        ),
        to(acme),
    ])
    .await;
    env.machine("web2", acme).await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn vm_on_cluster_container_on_vm(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    env.applied(vec![op(
        "create_cluster",
        json!({"slug": "c1", "owner": {"id": acme}}),
    )])
    .await;
    let cluster = env.id("machines.cluster", "slug", "c1").await;
    let host = env
        .create("host1", "physical", json!({"member_of": {"id": cluster}}))
        .await;
    let vm = env
        .create("vm1", "virtual", json!({"host": {"machine": {"id": host}}}))
        .await;
    env.create(
        "vm2",
        "virtual",
        json!({"host": {"cluster": {"id": cluster}}}),
    )
    .await;
    env.create("ct1", "container", json!({"host": {"machine": {"id": vm}}}))
        .await;
    // Neither an appliance hosted, nor a VM on a VM, nor a virtual member of a cluster.
    for (name, kind, extra) in [
        (
            "app1",
            "appliance",
            json!({"host": {"machine": {"id": host}}}),
        ),
        ("vm3", "virtual", json!({"host": {"machine": {"id": vm}}})),
        ("vm4", "virtual", json!({"member_of": {"id": cluster}})),
    ] {
        let mut fields = json!({"name": name, "kind": kind, "owner": {"id": acme}});
        merge(&mut fields, extra);
        env.refused(vec![op("create_machine", fields)], "machines.host.invalid")
            .await;
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn hosting_cycle_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let a = env.create("a", "container", json!({})).await;
    let b = env.create("b", "virtual", json!({})).await;
    // A machine does not host itself.
    env.refused(
        vec![op(
            "update_machine",
            json!({"id": {"id": b}, "host": {"machine": {"id": b}}}),
        )],
        "machines.host.invalid",
    )
    .await;
    // The container on the VM, then the VM back on the container: refused.
    env.applied(vec![op(
        "update_machine",
        json!({"id": {"id": a}, "host": {"machine": {"id": b}}}),
    )])
    .await;
    env.refused(
        vec![op(
            "update_machine",
            json!({"id": {"id": b}, "host": {"machine": {"id": a}}}),
        )],
        "machines.host.invalid",
    )
    .await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn kind_change_with_host_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let host = env.create("h", "physical", json!({})).await;
    let vm = env
        .create("v", "virtual", json!({"host": {"machine": {"id": host}}}))
        .await;
    env.refused(
        vec![op(
            "update_machine",
            json!({"id": {"id": vm}, "kind": "physical"}),
        )],
        "machines.host.invalid",
    )
    .await;
    // The host turned into an appliance: its guest is refused on the final state.
    env.refused(
        vec![op(
            "update_machine",
            json!({"id": {"id": host}, "kind": "appliance"}),
        )],
        "machines.host.invalid",
    )
    .await;
    // Without its host, the machine may become physical.
    env.applied(vec![op(
        "update_machine",
        json!({"id": {"id": vm}, "host": null, "kind": "physical"}),
    )])
    .await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn cluster_slug_unique(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let cluster = |slug: &str| {
        op(
            "create_cluster",
            json!({"slug": slug, "owner": {"id": acme}}),
        )
    };
    env.applied(vec![cluster("prod")]).await;
    env.refused(vec![cluster("prod")], "machines.cluster.duplicate")
        .await;
    // Without an owner: refused.
    env.refused(
        vec![op("create_cluster", json!({"slug": "lone"}))],
        "prop.assignment.required",
    )
    .await;
    // Renamed, the slug is free again.
    let id = env.id("machines.cluster", "slug", "prod").await;
    env.applied(vec![op(
        "update_cluster",
        json!({"id": {"id": id}, "slug": "prod-old"}),
    )])
    .await;
    env.applied(vec![cluster("prod")]).await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn interface_name_unique_per_machine(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (a, b) = (
        env.create("a", "physical", json!({})).await,
        env.create("b", "physical", json!({})).await,
    );
    let eth0 = |machine: Uuid| {
        op(
            "create_interface",
            json!({"machine": {"id": machine}, "name": "eth0"}),
        )
    };
    env.applied(vec![eth0(a)]).await;
    env.refused(vec![eth0(a)], "machines.interface.duplicate")
        .await;
    // The same name on another machine: accepted.
    env.applied(vec![eth0(b)]).await;
    // Renaming into a taken name: refused; a name with a space or too long: refused.
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": a}, "name": "eth1"}),
    )])
    .await;
    let eth1 = env.interface(a, "eth1").await;
    env.refused(
        vec![op(
            "update_interface",
            json!({"id": {"id": eth1}, "name": "eth0"}),
        )],
        "machines.interface.duplicate",
    )
    .await;
    for bad in ["eth 2", "", &"x".repeat(65)] {
        env.refused(
            vec![op(
                "create_interface",
                json!({"machine": {"id": a}, "name": bad}),
            )],
            "machines.operation.invalid",
        )
        .await;
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn mac_stored_as_macaddr(pool: PgPool) {
    let env = Env::new(&pool).await;
    let m = env.create("m", "physical", json!({})).await;
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": m}, "name": "eth0", "mac": "0000.5e00.5301"}),
    )])
    .await;
    let eth0 = env.interface(m, "eth0").await;
    let (kind, value): (String, String) = sqlx::query_as(
        "SELECT pg_typeof(mac)::text, mac::text FROM machines.interface WHERE id = $1",
    )
    .bind(eth0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "macaddr");
    assert_eq!(value, "00:00:5e:00:53:01");
    // A group address is refused; a locally administered one is accepted; `null` removes it.
    env.refused(
        vec![op(
            "update_interface",
            json!({"id": {"id": eth0}, "mac": "01-00-5E-00-00-01"}),
        )],
        "machines.mac.invalid",
    )
    .await;
    env.applied(vec![op(
        "update_interface",
        json!({"id": {"id": eth0}, "mac": "02:00:5e:00:53:01", "enabled": false}),
    )])
    .await;
    env.applied(vec![op(
        "update_interface",
        json!({"id": {"id": eth0}, "mac": null}),
    )])
    .await;
    let mac: Option<String> = sqlx::query_scalar(
        "SELECT mac::text FROM machines.interface WHERE id = $1 AND valid_period @> now()",
    )
    .bind(eth0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mac, None);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn interface_cannot_change_machine(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (a, b) = (
        env.create("a", "physical", json!({})).await,
        env.create("b", "physical", json!({})).await,
    );
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": a}, "name": "eth0"}),
    )])
    .await;
    let eth0 = env.interface(a, "eth0").await;
    env.refused(
        vec![op(
            "update_interface",
            json!({"id": {"id": eth0}, "machine": {"id": b}}),
        )],
        "machines.operation.invalid",
    )
    .await;
    // Its machine is its parent in the tree of the module.
    let parent: Option<Uuid> = sqlx::query_scalar(
        "SELECT parent_id FROM module_object WHERE id = $1 AND valid_period @> now()",
    )
    .bind(eth0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(parent, Some(a));
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn owner_change_moves_interface_scope(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (acme, other) = (env.entity("acme").await, env.entity("other").await);
    let m = env.create("m", "physical", json!({})).await;
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": m}, "name": "eth0"}),
    )])
    .await;
    let eth0 = env.interface(m, "eth0").await;
    assert_eq!(env.owner(eth0).await, Some(acme));
    let (on_acme, on_other) = (env.reader_on(acme).await, env.reader_on(other).await);
    assert!(env.reads(on_acme, "machines.machine.read", m).await);
    assert!(env.reads(on_acme, "machines.interface.read", eth0).await);
    assert!(!env.reads(on_other, "machines.interface.read", eth0).await);
    // The machine to the other entity: its interface follows, in the differential too.
    let owner = env.property("core.owner").await;
    let moved = env
        .applied(vec![Operation::AssignProperty {
            object: Ref::Id(m),
            kind: ObjectKind::Module(ModuleKind::parse("machines.machine").unwrap()),
            property: Ref::Id(owner),
            values: vec![RawInput::Text(other.to_string())],
        }])
        .await;
    let Outcome::Applied { diff, .. } = moved else {
        unreachable!()
    };
    let moved: u64 = diff
        .iter()
        .filter(|d| d.property == "core.owner")
        .map(|d| d.count)
        .sum();
    assert_eq!(moved, 2, "the machine and its interface: {diff:?}");
    assert_eq!(env.owner(eth0).await, Some(other));
    assert!(!env.reads(on_acme, "machines.interface.read", eth0).await);
    assert!(env.reads(on_other, "machines.interface.read", eth0).await);
}

/// An interface `eth<n>` of `machine` in the 802.1Q mode `vlan`.
fn interface(machine: Uuid, n: u32, vlan: Value) -> Operation {
    let mut fields = json!({"machine": {"id": machine}, "name": format!("eth{n}")});
    fields["vlan"] = vlan;
    op("create_interface", fields)
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn access_needs_one_vlan(pool: PgPool) {
    let env = Env::new(&pool).await;
    let [a, b, c] = env.vlans(&[10, 20, 30]).await[..] else {
        unreachable!()
    };
    let m = env.create("m", "physical", json!({})).await;
    for bad in [
        json!({"mode": "access"}),
        json!({"mode": "access", "access": {"id": a}, "tagged": [{"id": b}]}),
        json!({"mode": "none", "tagged": [{"id": a}]}),
        json!({"mode": "hybrid"}),
    ] {
        env.refused(vec![interface(m, 0, bad)], "machines.vlan.mode")
            .await;
    }
    // No mode: none.
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": m}, "name": "lo"}),
    )])
    .await;
    let lo = env.interface(m, "lo").await;
    assert_eq!(
        env.vlan_columns(lo).await,
        ("none".into(), None, None, vec![])
    );
    env.applied(vec![interface(
        m,
        0,
        json!({"mode": "access", "access": {"id": a}}),
    )])
    .await;
    let eth0 = env.interface(m, "eth0").await;
    assert_eq!(
        env.vlan_columns(eth0).await,
        ("access".into(), Some(a), None, vec![])
    );
    // A patch replaces the whole mode; `null` comes back to none.
    env.applied(vec![op(
        "update_interface",
        json!({"id": {"id": eth0},
               "vlan": {"mode": "trunk", "native": {"id": c}, "tagged": [{"id": b}, {"id": a}]}}),
    )])
    .await;
    let (mode, access, native, mut tagged) = env.vlan_columns(eth0).await;
    tagged.sort();
    let mut expected = vec![a, b];
    expected.sort();
    assert_eq!(
        (mode.as_str(), access, native, tagged),
        ("trunk", None, Some(c), expected)
    );
    env.applied(vec![op(
        "update_interface",
        json!({"id": {"id": eth0}, "vlan": null}),
    )])
    .await;
    assert_eq!(
        env.vlan_columns(eth0).await,
        ("none".into(), None, None, vec![])
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn native_not_in_tagged(pool: PgPool) {
    let env = Env::new(&pool).await;
    let [a, b] = env.vlans(&[10, 20]).await[..] else {
        unreachable!()
    };
    let m = env.create("m", "physical", json!({})).await;
    for bad in [
        json!({"mode": "trunk", "native": {"id": a}, "tagged": [{"id": a}, {"id": b}]}),
        json!({"mode": "trunk", "native": {"id": a}}),
        json!({"mode": "trunk", "access": {"id": a}, "tagged": [{"id": b}]}),
    ] {
        env.refused(vec![interface(m, 0, bad)], "machines.vlan.mode")
            .await;
    }
    env.applied(vec![interface(
        m,
        0,
        json!({"mode": "trunk", "native": {"id": a}, "tagged": [{"id": b}]}),
    )])
    .await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn closed_vlan_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let [a, b] = env.vlans(&[10, 20]).await[..] else {
        unreachable!()
    };
    let m = env.create("m", "physical", json!({})).await;
    env.applied(vec![Operation::Close {
        kind: ObjectKind::Module(ModuleKind::parse("vlan.vlan").unwrap()),
        id: Ref::Id(a),
        expected: None,
        cascade: false,
    }])
    .await;
    env.refused(
        vec![interface(
            m,
            0,
            json!({"mode": "access", "access": {"id": a}}),
        )],
        "machines.vlan.mode",
    )
    .await;
    env.refused(
        vec![interface(
            m,
            0,
            json!({"mode": "trunk", "tagged": [{"id": b}, {"id": a}]}),
        )],
        "machines.vlan.mode",
    )
    .await;
    // An unknown VLAN neither.
    env.refused(
        vec![interface(
            m,
            0,
            json!({"mode": "access", "access": {"id": Uuid::now_v7()}}),
        )],
        "machines.vlan.mode",
    )
    .await;
    env.applied(vec![interface(
        m,
        0,
        json!({"mode": "access", "access": {"id": b}}),
    )])
    .await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn vlan_read_required(pool: PgPool) {
    let env = Env::new(&pool).await;
    let [a] = env.vlans(&[10]).await[..] else {
        unreachable!()
    };
    let acme = env.entity("acme").await;
    let m = env.create("m", "physical", json!({})).await;
    let writer = [
        "machines.machine.read",
        "machines.interface.read",
        "machines.interface.write",
    ];
    // A writer of the interfaces of `acme` who does not read the VLAN.
    let blind = env.granted(acme, &writer).await;
    let access = json!({"mode": "access", "access": {"id": a}});
    let refused = env
        .submit_as(blind, vec![interface(m, 0, access.clone())])
        .await;
    assert!(
        rules(&refused).contains(&"core.object.forbidden"),
        "{refused:?}"
    );
    // Without a VLAN, the same writer creates the interface; reading the VLAN, too.
    let applied = env
        .submit_as(blind, vec![interface(m, 1, Value::Null)])
        .await;
    assert!(matches!(applied, Outcome::Applied { .. }), "{applied:?}");
    let seeing = env
        .granted(
            acme,
            &[
                "machines.machine.read",
                "machines.interface.read",
                "machines.interface.write",
                "vlan.vlan.read",
            ],
        )
        .await;
    let applied = env.submit_as(seeing, vec![interface(m, 0, access)]).await;
    assert!(matches!(applied, Outcome::Applied { .. }), "{applied:?}");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn two_concurrent_attachments_one_wins(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env
        .domain("lab", env.entity("acme").await, "192.0.2.0/24")
        .await;
    let address = env.address(lab, "192.0.2.10", AddressUsage::Host).await;
    let (e1, e2) = (env.eth0("m1").await, env.eth0("m2").await);
    // Hold the key of the address, as the store takes it, so that both submissions queue
    // behind it; then release them together.
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2))")
        .bind(NAMED_LOCK_CLASS)
        .bind(format!("machines.address:{address}"))
        .execute(&mut *holder)
        .await
        .unwrap();
    let spawn = |interface: Uuid| {
        let store = env.store.clone();
        let request = env.request(vec![attach(interface, address, false)]);
        tokio::spawn(async move {
            submit(&store, &Unrestricted, &request, MAX_OPERATIONS)
                .await
                .unwrap()
        })
    };
    let (first, second) = (spawn(e1), spawn(e2));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!first.is_finished() && !second.is_finished());
    holder.rollback().await.unwrap();
    let outcomes = [first.await.unwrap(), second.await.unwrap()];
    let applied = outcomes
        .iter()
        .filter(|o| matches!(o, Outcome::Applied { .. }))
        .count();
    assert_eq!(applied, 1, "{outcomes:?}");
    let refused = outcomes
        .iter()
        .find(|o| !matches!(o, Outcome::Applied { .. }))
        .unwrap();
    assert!(
        rules(refused).contains(&"machines.address.taken"),
        "{refused:?}"
    );
    assert_eq!(env.attachments(address).await.len(), 1);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn vip_on_two_machines(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env
        .domain("lab", env.entity("acme").await, "192.0.2.0/24")
        .await;
    let vip = env.address(lab, "192.0.2.10", AddressUsage::Vip).await;
    let host = env.address(lab, "192.0.2.11", AddressUsage::Host).await;
    let (e1, e2) = (env.eth0("m1").await, env.eth0("m2").await);
    env.applied(vec![attach(e1, vip, false), attach(e2, vip, false)])
        .await;
    let both = env.attachments(vip).await;
    assert_eq!(both.len(), 2, "{both:?}");
    assert!(both.iter().all(|(_, shared, _)| *shared), "{both:?}");
    // Twice on the same interface: refused.
    env.refused(vec![attach(e1, vip, false)], "machines.address.taken")
        .await;
    // A host address once only; detached, it moves.
    env.applied(vec![attach(e1, host, false)]).await;
    env.refused(vec![attach(e2, host, false)], "machines.address.taken")
        .await;
    env.refused(vec![detach(e2, host)], "machines.operation.invalid")
        .await;
    env.applied(vec![detach(e1, host), attach(e2, host, false)])
        .await;
    assert_eq!(env.attachments(host).await, vec![(e2, false, false)]);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn usage_back_to_host_refused_when_shared(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env
        .domain("lab", env.entity("acme").await, "192.0.2.0/24")
        .await;
    let vip = env.address(lab, "192.0.2.10", AddressUsage::Vip).await;
    let single = env.address(lab, "192.0.2.11", AddressUsage::Host).await;
    let (e1, e2) = (env.eth0("m1").await, env.eth0("m2").await);
    env.applied(vec![
        attach(e1, vip, false),
        attach(e2, vip, false),
        attach(e1, single, false),
    ])
    .await;
    env.refused(
        vec![usage(vip, AddressUsage::Host)],
        "machines.address.taken",
    )
    .await;
    assert!(env.attachments(vip).await.iter().all(|(_, s, _)| *s));
    // On one interface, the usage changes both ways and the copy follows.
    env.applied(vec![usage(single, AddressUsage::Vip)]).await;
    assert_eq!(env.attachments(single).await, vec![(e1, true, false)]);
    env.applied(vec![attach(e2, single, false)]).await;
    env.applied(vec![detach(e2, single)]).await;
    env.applied(vec![usage(single, AddressUsage::Host)]).await;
    assert_eq!(env.attachments(single).await, vec![(e1, false, false)]);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn two_primary_v4_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let lab = env
        .domain("lab", env.entity("acme").await, "192.0.2.0/24")
        .await;
    env.applied(vec![network_op(Ref::Id(lab), "2001:db8::/64")])
        .await;
    let a4 = env.address(lab, "192.0.2.10", AddressUsage::Host).await;
    let b4 = env.address(lab, "192.0.2.11", AddressUsage::Host).await;
    let a6 = env.address(lab, "2001:db8::10", AddressUsage::Host).await;
    let e1 = env.eth0("m1").await;
    env.applied(vec![attach(e1, a4, true)]).await;
    env.refused(vec![attach(e1, b4, true)], "machines.address.primary")
        .await;
    env.applied(vec![attach(e1, b4, false), attach(e1, a6, true)])
        .await;
    assert_eq!(env.attachments(a4).await, vec![(e1, false, true)]);
    assert_eq!(env.attachments(b4).await, vec![(e1, false, false)]);
    assert_eq!(env.attachments(a6).await, vec![(e1, false, true)]);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn attach_needs_address_read_only(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (acme, other) = (env.entity("acme").await, env.entity("other").await);
    let lab = env.domain("lab", acme, "192.0.2.0/24").await;
    let far = env.domain("far", other, "198.51.100.0/24").await;
    let seen = env.address(lab, "192.0.2.10", AddressUsage::Host).await;
    let hidden = env.address(far, "198.51.100.10", AddressUsage::Host).await;
    let e1 = env.eth0("m1").await;
    // A writer of the machines of `acme` who reads its addressing but cannot write it.
    let system = env
        .granted(
            acme,
            &[
                "machines.machine.read",
                "machines.interface.read",
                "machines.interface.write",
                "core.address.read",
            ],
        )
        .await;
    assert!(!env.reads(system, "core.address.write", seen).await);
    let applied = env.submit_as(system, vec![attach(e1, seen, true)]).await;
    assert!(matches!(applied, Outcome::Applied { .. }), "{applied:?}");
    let refused = env.submit_as(system, vec![attach(e1, hidden, false)]).await;
    assert!(
        rules(&refused).contains(&"core.object.forbidden"),
        "{refused:?}"
    );
    let applied = env.submit_as(system, vec![detach(e1, seen)]).await;
    assert!(matches!(applied, Outcome::Applied { .. }), "{applied:?}");
    assert!(env.attachments(seen).await.is_empty());
}

/// The assignment of `values` to the property `property` of `object`, of `kind`.
fn assign(object: Uuid, kind: &str, property: Uuid, value: &str) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(object),
        kind: ObjectKind::Module(ModuleKind::parse(kind).unwrap()),
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    }
}

impl Env {
    /// The resolved value of `property` for `object` now, as its explanation gives it,
    /// and the object the value comes from.
    async fn explained(&self, object: Uuid, property: Uuid) -> (Option<String>, Option<Uuid>) {
        let explanation = kartenn_store_pg::explain::explain(
            &self.pool,
            object,
            property,
            sqlx::types::time::OffsetDateTime::now_utc(),
            None,
        )
        .await
        .unwrap()
        .expect("a declared property");
        (explanation.value, explanation.origin)
    }

    /// The keys of the values of the closed list `property`, living now, sorted.
    async fn values(&self, property: Uuid) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT key FROM property_enum_value
              WHERE property_id = $1 AND valid_period @> now() ORDER BY key",
        )
        .bind(property)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn interface_role_values(pool: PgPool) {
    let env = Env::new(&pool).await;
    let role = env.property("machines.interface_role").await;
    let mut shipped: Vec<String> = kartenn_machines::module::INTERFACE_ROLES
        .iter()
        .map(|&v| v.to_owned())
        .collect();
    shipped.sort();
    assert_eq!(env.values(role).await, shipped);
    let m = env.create("m", "physical", json!({})).await;
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": m}, "name": "eth1"}),
    )])
    .await;
    let eth1 = env.interface(m, "eth1").await;
    env.applied(vec![assign(
        eth1,
        "machines.interface",
        role,
        "administration",
    )])
    .await;
    let (value, origin) = env.explained(eth1, role).await;
    assert!(value.is_some_and(|v| v.contains("administration")));
    assert_eq!(origin, Some(eth1));
    // Outside the list, or on a machine: refused.
    let refused = env
        .submit(vec![assign(eth1, "machines.interface", role, "dmz")])
        .await;
    assert!(matches!(refused, Outcome::Refused { .. }), "{refused:?}");
    let refused = env
        .submit(vec![assign(m, "machines.machine", role, "backup")])
        .await;
    assert!(matches!(refused, Outcome::Refused { .. }), "{refused:?}");
    // The FQDN of a machine.
    let fqdn = env.property("machines.fqdn").await;
    env.applied(vec![assign(m, "machines.machine", fqdn, "m.example.org")])
        .await;
    assert!(
        env.explained(m, fqdn)
            .await
            .0
            .is_some_and(|v| v.contains("m.example.org"))
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn admin_adds_a_role_value(pool: PgPool) {
    let env = Env::new(&pool).await;
    let role = env.property("machines.interface_role").await;
    env.applied(vec![Operation::AddEnumValue {
        local: None,
        property: Ref::Id(role),
        key: "dmz".into(),
    }])
    .await;
    let m = env.create("m", "physical", json!({})).await;
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": m}, "name": "eth1"}),
    )])
    .await;
    let eth1 = env.interface(m, "eth1").await;
    env.applied(vec![assign(eth1, "machines.interface", role, "dmz")])
        .await;
    // A new sync of the declarations keeps the value of the administrator.
    sync(&env.store, &env.registry).await;
    assert!(env.values(role).await.contains(&"dmz".to_owned()));
    assert!(
        env.explained(eth1, role)
            .await
            .0
            .is_some_and(|v| v.contains("dmz"))
    );
}

/// A module that puts its own properties on machines (the policy of the 010, M3, M8).
#[derive(Debug)]
struct Inventory;

impl Module for Inventory {
    fn name(&self) -> &'static str {
        "inventory"
    }
    fn version(&self) -> &'static str {
        "0.0.0"
    }
    fn depends_on(&self) -> &'static [&'static str] {
        &["machines"]
    }
    fn object_types(&self) -> &'static [ObjectType] {
        &[]
    }
    fn permissions(&self) -> &'static [&'static Permission] {
        &[]
    }
    fn declarations(&self) -> Option<ModuleDeclarations> {
        let property = |key: &str| Declaration {
            key: PropertyKey::parse(key).unwrap(),
            declarant: Declarant::Module("inventory".into()),
            spec: TypeSpec::Enum,
            cardinality: Cardinality::Single,
            carriers: std::collections::BTreeSet::from(["machines.machine".to_owned()]),
            inherit: None,
            obligations: std::collections::BTreeSet::new(),
            expected: None,
        };
        Some(ModuleDeclarations {
            module: "inventory".into(),
            version: "0.0.0".into(),
            properties: vec![
                (
                    property("inventory.criticality"),
                    vec!["low".into(), "high".into()],
                ),
                (
                    property("inventory.sensitivity"),
                    vec!["public".into(), "secret".into()],
                ),
            ],
        })
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn foreign_module_property_on_a_machine(pool: PgPool) {
    let env = Env::with(&pool, vec![Arc::new(Inventory)]).await;
    let criticality = env.property("inventory.criticality").await;
    let sensitivity = env.property("inventory.sensitivity").await;
    let m = env.create("m", "physical", json!({})).await;
    env.applied(vec![
        assign(m, "machines.machine", criticality, "high"),
        assign(m, "machines.machine", sensitivity, "secret"),
    ])
    .await;
    let (value, origin) = env.explained(m, criticality).await;
    assert!(value.is_some_and(|v| v.contains("high")));
    assert_eq!(origin, Some(m));
    let (value, origin) = env.explained(m, sensitivity).await;
    assert!(value.is_some_and(|v| v.contains("secret")));
    assert_eq!(origin, Some(m));
    // Each changes on its own.
    env.applied(vec![assign(m, "machines.machine", criticality, "low")])
        .await;
    assert!(
        env.explained(m, criticality)
            .await
            .0
            .is_some_and(|v| v.contains("low"))
    );
    assert!(
        env.explained(m, sensitivity)
            .await
            .0
            .is_some_and(|v| v.contains("secret"))
    );
    // Not carried by the interfaces, which the module did not name.
    env.applied(vec![op(
        "create_interface",
        json!({"machine": {"id": m}, "name": "eth0"}),
    )])
    .await;
    let eth0 = env.interface(m, "eth0").await;
    let refused = env
        .submit(vec![assign(
            eth0,
            "machines.interface",
            criticality,
            "high",
        )])
        .await;
    assert!(matches!(refused, Outcome::Refused { .. }), "{refused:?}");
}

impl Env {
    /// Whether an anomaly of `rule` is open on `object`.
    async fn anomaly(&self, object: Uuid, rule: &str) -> bool {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM anomaly
                             WHERE object_id = $1 AND rule_id = $2 AND closed_at IS NULL)",
        )
        .bind(object)
        .bind(rule)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The rules of the warnings of a simulation of `operations`.
    async fn simulated_warnings(&self, operations: Vec<Operation>) -> Vec<&'static str> {
        let mut request = self.request(operations);
        request.simulate = true;
        match submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
        {
            Outcome::Simulated { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
            other => panic!("not simulated: {other:?}"),
        }
    }

    /// `vlan.vlan` of the network `network` set to `vlan`.
    async fn network_vlan(&self, network: Uuid, vlan: Uuid) -> Operation {
        Operation::AssignProperty {
            object: Ref::Id(network),
            kind: ObjectKind::Prefix,
            property: Ref::Id(self.property("vlan.vlan").await),
            values: vec![RawInput::Text(vlan.to_string())],
        }
    }

    /// The network `prefix` of `domain`, living now.
    async fn network(&self, domain: Uuid, prefix: &str) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM prefix
              WHERE domain_id = $1 AND prefix = $2::cidr AND valid_period @> now()",
        )
        .bind(domain)
        .bind(prefix)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

/// `update_interface` of `interface` with `fields`.
fn update_interface(interface: Uuid, fields: Value) -> Operation {
    let mut object = fields;
    object["id"] = json!({"id": interface});
    op("update_interface", object)
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn each_warning_opens_and_closes(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    // A universal MAC on two interfaces: a warning in the simulation, then an anomaly on
    // each, closed once one of them changes.
    let (e1, e2) = (env.eth0("m1").await, env.eth0("m2").await);
    env.applied(vec![update_interface(
        e1,
        json!({"mac": "00:1b:21:0a:0b:0c"}),
    )])
    .await;
    let same = update_interface(e2, json!({"mac": "00:1b:21:0a:0b:0c"}));
    assert!(
        env.simulated_warnings(vec![same.clone()])
            .await
            .contains(&"machines.mac.duplicate")
    );
    env.applied(vec![same]).await;
    assert!(env.anomaly(e1, "machines.mac.duplicate").await);
    assert!(env.anomaly(e2, "machines.mac.duplicate").await);
    env.applied(vec![update_interface(
        e2,
        json!({"mac": "00:1b:21:0a:0b:0d"}),
    )])
    .await;
    assert!(!env.anomaly(e1, "machines.mac.duplicate").await);
    assert!(!env.anomaly(e2, "machines.mac.duplicate").await);
    // A locally administered MAC may repeat (RFC 9542 §2.1).
    env.applied(vec![
        update_interface(e1, json!({"mac": "02:00:00:00:00:01"})),
        update_interface(e2, json!({"mac": "02:00:00:00:00:01"})),
    ])
    .await;
    assert!(!env.anomaly(e1, "machines.mac.duplicate").await);

    // An address whose network carries a VLAN the interface does not.
    let vlans = env.vlans(&[10, 20]).await;
    let lab = env.domain("lab", acme, "192.0.2.0/24").await;
    let network = env.network(lab, "192.0.2.0/24").await;
    env.applied(vec![env.network_vlan(network, vlans[0]).await])
        .await;
    let address = env.address(lab, "192.0.2.10", AddressUsage::Host).await;
    env.applied(vec![
        update_interface(
            e1,
            json!({"vlan": {"mode": "access", "access": {"id": vlans[1]}}}),
        ),
        attach(e1, address, false),
    ])
    .await;
    assert!(env.anomaly(e1, "machines.address.vlan_mismatch").await);
    env.applied(vec![update_interface(
        e1,
        json!({"vlan": {"mode": "access", "access": {"id": vlans[0]}}}),
    )])
    .await;
    assert!(!env.anomaly(e1, "machines.address.vlan_mismatch").await);

    // A machine hosted on another site than its host.
    let site = env.property("core.site").await;
    env.applied(vec![Operation::CreateSite {
        local: None,
        slug: "lyon".into(),
        parent: None,
    }])
    .await;
    let (paris, lyon) = (
        env.id("site", "slug", "paris").await,
        env.id("site", "slug", "lyon").await,
    );
    let host = env
        .create("host1", "physical", json!({"site": {"id": paris}}))
        .await;
    let vm = env
        .create(
            "vm1",
            "virtual",
            json!({"host": {"machine": {"id": host}}, "site": {"id": lyon}}),
        )
        .await;
    assert!(env.anomaly(vm, "machines.host.site_mismatch").await);
    // Corrected on the host: the anomaly of the guest closes.
    env.applied(vec![assign(
        host,
        "machines.machine",
        site,
        &lyon.to_string(),
    )])
    .await;
    assert!(!env.anomaly(vm, "machines.host.site_mismatch").await);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn vlan_change_on_network_reevaluates(pool: PgPool) {
    let env = Env::new(&pool).await;
    let acme = env.entity("acme").await;
    let vlans = env.vlans(&[10, 20]).await;
    let lab = env.domain("lab", acme, "192.0.2.0/24").await;
    let network = env.network(lab, "192.0.2.0/24").await;
    let address = env.address(lab, "192.0.2.10", AddressUsage::Host).await;
    let e1 = env.eth0("m1").await;
    env.applied(vec![
        update_interface(
            e1,
            json!({"vlan": {"mode": "trunk", "tagged": [{"id": vlans[0]}]}}),
        ),
        attach(e1, address, false),
    ])
    .await;
    // The network gets a VLAN the trunk carries: nothing; another: an anomaly, though the
    // changeset touches no interface.
    env.applied(vec![env.network_vlan(network, vlans[0]).await])
        .await;
    assert!(!env.anomaly(e1, "machines.address.vlan_mismatch").await);
    let change = env.network_vlan(network, vlans[1]).await;
    assert!(
        env.simulated_warnings(vec![change.clone()])
            .await
            .contains(&"machines.address.vlan_mismatch")
    );
    env.applied(vec![change]).await;
    assert!(env.anomaly(e1, "machines.address.vlan_mismatch").await);
    // Back to the VLAN of the trunk: closed.
    env.applied(vec![env.network_vlan(network, vlans[0]).await])
        .await;
    assert!(!env.anomaly(e1, "machines.address.vlan_mismatch").await);
}
