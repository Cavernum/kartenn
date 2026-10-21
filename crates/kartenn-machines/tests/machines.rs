//! Clusters and machines through the write pipeline (spec 009, REQ-009-1 to 5; task
//! 009-2): the dependency on the VLAN module, a name unique per resolved owner, the
//! hosting of a machine (M5) and a cluster slug unique at every instant.

use std::sync::Arc;

use kartenn_machines::Machines;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{core_declarations_for, sync_modules};
use kartenn_service::module::ModuleRegistry;
use kartenn_service::operations::{ModuleKind, RawInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use kartenn_vlan::Vlan;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Env {
    pool: PgPool,
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
