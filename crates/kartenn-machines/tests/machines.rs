//! Clusters, machines and interfaces through the write pipeline (spec 009, REQ-009-1 to
//! 7; tasks 009-2 and 009-3): the dependency on the VLAN module, a name unique per
//! resolved owner, the hosting of a machine (M5), a cluster slug unique at every instant;
//! interfaces tied to their machine, named once per machine, with an EUI-48 stored as
//! `macaddr`, and inheriting the anchors of their machine; the 802.1Q mode of an
//! interface and its VLANs (task 009-4).

use std::sync::Arc;

use kartenn_machines::Machines;
use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::module::ModuleRegistry;
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
        let registry =
            Arc::new(ModuleRegistry::new(vec![Arc::new(Vlan), Arc::new(Machines)]).unwrap());
        kartenn_store_pg::modules::migrate(pool, &registry)
            .await
            .unwrap();
        let store = PgStore::new(pool.clone()).with_modules(registry.clone());
        for outcome in sync_modules(
            &store,
            &Unrestricted,
            &[core_declarations_for(&registry)],
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
        {
            assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        }
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

/// `machines.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

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
    op(
        "create_interface",
        json!({"machine": {"id": machine}, "name": format!("eth{n}"), "vlan": vlan}),
    )
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
