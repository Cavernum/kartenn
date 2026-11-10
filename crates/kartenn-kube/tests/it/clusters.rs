//! Clusters, nodes and namespaces through the write pipeline (spec 019, REQ-019-4 to 7;
//! task 019-3b): a cluster slug unique at every instant, its ranges in the domains it
//! designates, its batch window and connector group checked; the nodes and namespaces of
//! the connector found by their name in the cluster, unique there, a node designating one
//! physical or virtual machine; the workloads of a namespace found by their kind and name
//! there (task 019-4a).

use std::sync::Arc;

use kartenn_core::objects::Nature;
use kartenn_kube::Kube;
use kartenn_machines::Machines;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{ModuleDeclarations, core_declarations_for, sync_modules};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

struct Env {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    acme: Uuid,
    pods: Uuid,
    services: Uuid,
    container: Uuid,
    network: Uuid,
}

impl Env {
    /// Both modules migrated, the entity `acme`, a pod domain with the container
    /// `198.51.100.0/24` and a service domain with the network `203.0.113.0/26`.
    async fn new(pool: &PgPool) -> Self {
        let modules: Vec<Arc<dyn Module>> = vec![Arc::new(Machines), Arc::new(Kube)];
        let registry = Arc::new(ModuleRegistry::new(modules).unwrap());
        kartenn_store_pg::modules::migrate(pool, &registry)
            .await
            .unwrap();
        let store = PgStore::new(pool.clone()).with_modules(registry.clone());
        let declarations: Vec<ModuleDeclarations> =
            std::iter::once(core_declarations_for(&registry))
                .chain(registry.modules().filter_map(|m| m.declarations()))
                .collect();
        for outcome in sync_modules(&store, &Unrestricted, &declarations, MAX_OPERATIONS)
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
        let mut env = Self {
            pool: pool.clone(),
            store,
            author,
            acme: Uuid::nil(),
            pods: Uuid::nil(),
            services: Uuid::nil(),
            container: Uuid::nil(),
            network: Uuid::nil(),
        };
        env.applied(vec![Operation::CreateEntity {
            local: Some(1),
            slug: "acme".into(),
            parent: None,
        }])
        .await;
        env.acme = env.id("entity", "slug", "acme").await;
        env.pods = env
            .domain("pods", "198.51.100.0/24", Nature::Container)
            .await;
        env.services = env
            .domain("services", "203.0.113.0/26", Nature::Network)
            .await;
        env.container = env.prefix(env.pods).await;
        env.network = env.prefix(env.services).await;
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
            approval_self: true,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>) {
        let outcome = self.submit(operations).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    async fn refused(&self, operations: Vec<Operation>, rule: &str) {
        let outcome = self.submit(operations).await;
        assert!(rules(&outcome).contains(&rule), "{rule}: {outcome:?}");
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

    /// A domain `slug` owned by `acme`, with the prefix `prefix` of `nature`.
    async fn domain(&self, slug: &str, prefix: &str, nature: Nature) -> Uuid {
        let owner: Uuid =
            sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
                .fetch_one(&self.pool)
                .await
                .unwrap();
        self.applied(vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: slug.into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(self.acme.to_string())],
            },
            Operation::CreatePrefix {
                local: None,
                domain: Ref::Local(1),
                prefix: prefix.into(),
                nature,
                point_to_point: false,
                anycast: false,
            },
        ])
        .await;
        self.id("addressing_domain", "slug", slug).await
    }

    /// The prefix of `domain`.
    async fn prefix(&self, domain: Uuid) -> Uuid {
        sqlx::query_scalar("SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()")
            .bind(domain)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// The fields of a valid cluster `slug`, with `extra`.
    fn cluster(&self, slug: &str, extra: Value) -> Operation {
        let mut fields = json!({
            "slug": slug,
            "owner": {"id": self.acme},
            "pod_domain": {"id": self.pods},
            "service_domain": {"id": self.services},
            "pod_containers": [{"id": self.container}],
            "service_networks": [{"id": self.network}],
        });
        merge(&mut fields, extra);
        op("create_cluster", fields)
    }

    /// A new cluster `slug`.
    async fn new_cluster(&self, slug: &str) -> Uuid {
        self.applied(vec![self.cluster(slug, json!({}))]).await;
        self.id("kube.cluster", "slug", slug).await
    }

    /// The living objects of `table` of `cluster` named `name`.
    async fn named(&self, table: &str, cluster: Uuid, name: &str) -> Vec<Uuid> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE cluster_id = $1 AND name = $2 AND valid_period @> now()"
        )))
        .bind(cluster)
        .bind(name)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    /// A machine `name` of `kind` owned by `acme`.
    async fn machine(&self, name: &str, kind: &str) -> Uuid {
        self.applied(vec![machine_op(
            "create_machine",
            json!({"name": name, "kind": kind, "owner": {"id": self.acme}}),
        )])
        .await;
        self.id("machines.machine", "name", name).await
    }
}

fn merge(into: &mut Value, extra: Value) {
    if let (Value::Object(into), Value::Object(extra)) = (into, extra) {
        into.extend(extra);
    }
}

/// `kube.<name>` from its JSON fields.
fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("kube.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// `machines.<name>` from its JSON fields.
fn machine_op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// `kube.sync` of `action` on the object of `kind` named `name` in `cluster`.
fn sync(action: &str, kind: &str, cluster: Uuid, name: &str, extra: Value) -> Operation {
    let mut fields = json!({
        "action": action,
        "type": format!("kube.{kind}"),
        "cluster": {"id": cluster},
        "name": name,
    });
    merge(&mut fields, extra);
    op("sync", fields)
}

fn rules(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Refused { violations, .. } => violations.iter().map(|v| v.rule.id).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

/// REQ-019-5: a cluster slug is unique at every instant; renamed, it is free again.
#[kartenn_testing::pg_test]
async fn cluster_slug_unique(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    env.refused(vec![env.cluster("prod", json!({}))], "core.slug.duplicate")
        .await;
    env.refused(vec![env.cluster("Prod", json!({}))], "core.slug.syntax")
        .await;
    // Without an owner: refused by the core (`owner_required`); a null owner, here.
    env.refused(
        vec![op(
            "create_cluster",
            json!({"slug": "lone", "pod_domain": {"id": env.pods},
                   "service_domain": {"id": env.services}}),
        )],
        "prop.assignment.required",
    )
    .await;
    env.refused(
        vec![env.cluster("lone", json!({"owner": null}))],
        "kube.operation.invalid",
    )
    .await;
    env.applied(vec![op(
        "update_cluster",
        json!({"id": {"id": prod}, "slug": "prod-old"}),
    )])
    .await;
    env.new_cluster("prod").await;
}

/// REQ-019-5: the ranges of a cluster are of the domains it designates, its batch window
/// and retention bounded, its connector group a living group of kartenn.
#[kartenn_testing::pg_test]
async fn range_in_designated_domain(pool: PgPool) {
    let env = Env::new(&pool).await;
    // A container of the service domain as a pod range, a network as a container.
    env.refused(
        vec![env.cluster("a", json!({"pod_containers": [{"id": env.network}]}))],
        "kube.cluster.range_domain",
    )
    .await;
    env.refused(
        vec![env.cluster("a", json!({"service_networks": [{"id": env.container}]}))],
        "kube.cluster.range_domain",
    )
    .await;
    for (field, value) in [
        ("batch_window_seconds", 0),
        ("batch_window_seconds", 301),
        ("batch_max_operations", 0),
        ("batch_max_operations", 10_001),
        ("retention_days", 401),
    ] {
        env.refused(
            vec![env.cluster("a", json!({ field: value }))],
            "kube.operation.invalid",
        )
        .await;
    }
    env.refused(
        vec![env.cluster("a", json!({"tracked_label_keys": ["team/"]}))],
        "kube.label.invalid",
    )
    .await;
    // A group that does not exist, then one closed before the cluster ends.
    env.refused(
        vec![env.cluster("a", json!({"connector_group": {"id": Uuid::now_v7()}}))],
        "kube.cluster.connector_group",
    )
    .await;
    let group: Uuid = sqlx::query_scalar(
        "INSERT INTO rbac_group (id, key, labels, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'kube-connectors', '{}',
                 tstzrange(now() - interval '1 day', now() + interval '1 day'), gen_random_uuid())
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    env.refused(
        vec![env.cluster("a", json!({"connector_group": {"id": group}}))],
        "kube.cluster.connector_group",
    )
    .await;
    sqlx::query("UPDATE rbac_group SET valid_period = tstzrange(lower(valid_period), NULL)")
        .execute(&pool)
        .await
        .unwrap();
    env.applied(vec![env.cluster(
        "a",
        json!({
            "connector_group": {"id": group},
            "batch_window_seconds": 300,
            "batch_max_operations": 10_000,
            "tracked_label_keys": ["app.kubernetes.io/name", "team"],
        }),
    )])
    .await;
    let row: (i16, i32, Vec<String>, i16, Option<Uuid>) = sqlx::query_as(
        "SELECT batch_window_seconds, batch_max_operations, tracked_label_keys, retention_days,
                connector_group_id
           FROM kube.cluster WHERE slug = 'a'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            300,
            10_000,
            vec!["app.kubernetes.io/name".into(), "team".into()],
            30,
            Some(group)
        )
    );
    // Moved to a container of another domain: refused on update too.
    let a = env.id("kube.cluster", "slug", "a").await;
    env.refused(
        vec![op(
            "update_cluster",
            json!({"id": {"id": a}, "pod_containers": [{"id": env.network}]}),
        )],
        "kube.cluster.range_domain",
    )
    .await;
    // A designated range cannot be closed under the cluster.
    env.refused(
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(env.container),
            expected: None,
            cascade: false,
        }],
        "core.object.dependents",
    )
    .await;
}

/// REQ-019-6, 7: a node and a namespace are found by their name in their cluster, unique
/// there at every instant, under the cluster in the tree and anchored in its pod domain.
#[kartenn_testing::pg_test]
async fn node_name_unique_per_cluster(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (prod, test) = (env.new_cluster("prod").await, env.new_cluster("test").await);
    let node = |cluster: Uuid, extra: Value| sync("upsert", "node", cluster, "worker-1", extra);
    // Twice the same name in a batch: the second is a duplicate.
    env.refused(
        vec![node(prod, json!({})), node(prod, json!({}))],
        "kube.node.duplicate",
    )
    .await;
    env.applied(vec![
        node(
            prod,
            json!({"uid": Uuid::now_v7(), "zone": "zone-a",
                   "addresses": [{"type": "InternalIP", "value": "192.0.2.10"}]}),
        ),
        node(test, json!({})),
    ])
    .await;
    let worker = env.named("kube.node", prod, "worker-1").await;
    assert_eq!(worker.len(), 1);
    assert_eq!(env.named("kube.node", test, "worker-1").await.len(), 1);
    // Upserted again: the same node, changed.
    env.applied(vec![node(prod, json!({"zone": "zone-b"}))])
        .await;
    assert_eq!(env.named("kube.node", prod, "worker-1").await, worker);
    // Under its cluster, it inherits its owner (REQ-019-5).
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT kartenn_owner($1, now())")
        .bind(worker[0])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, Some(env.acme));
    let (zone, parent, domain): (Option<String>, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT n.zone, o.parent_id, o.domain_id
           FROM kube.node n JOIN module_object o ON o.id = n.id AND o.valid_period @> now()
          WHERE n.id = $1 AND n.valid_period @> now()",
    )
    .bind(worker[0])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (zone.as_deref(), parent, domain),
        (Some("zone-b"), Some(prod), Some(env.pods))
    );
    // A name out of syntax.
    env.refused(
        vec![sync("upsert", "node", prod, "Worker_1", json!({}))],
        "kube.name.invalid",
    )
    .await;
    // Closed, the name is free: a new node takes it.
    env.applied(vec![sync("close", "node", prod, "worker-1", json!({}))])
        .await;
    assert!(env.named("kube.node", prod, "worker-1").await.is_empty());
    env.applied(vec![node(prod, json!({}))]).await;
    let again = env.named("kube.node", prod, "worker-1").await;
    assert_eq!(again.len(), 1);
    assert_ne!(again, worker);
    // Namespaces likewise, with their labels.
    let ns = |extra: Value| sync("upsert", "namespace", prod, "payments", extra);
    env.refused(
        vec![ns(json!({})), ns(json!({}))],
        "kube.namespace.duplicate",
    )
    .await;
    env.refused(
        vec![ns(json!({"labels": {"team/": "a"}}))],
        "kube.label.invalid",
    )
    .await;
    env.refused(
        vec![ns(json!({"zone": "zone-a"}))],
        "kube.operation.invalid",
    )
    .await;
    env.applied(vec![ns(json!({"labels": {"team": "billing"}}))])
        .await;
    let labels: Value = sqlx::query_scalar(
        "SELECT labels FROM kube.namespace WHERE name = 'payments' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(labels, json!({"team": "billing"}));
    // Closing the cluster without cascade is refused; with it, its objects close.
    env.refused(
        vec![Operation::Close {
            kind: ObjectKind::from_name("kube.cluster").unwrap(),
            id: Ref::Id(prod),
            expected: None,
            cascade: false,
        }],
        "core.object.dependents",
    )
    .await;
    env.applied(vec![Operation::Close {
        kind: ObjectKind::from_name("kube.cluster").unwrap(),
        id: Ref::Id(prod),
        expected: None,
        cascade: true,
    }])
    .await;
    assert!(env.named("kube.node", prod, "worker-1").await.is_empty());
    assert!(
        env.named("kube.namespace", prod, "payments")
            .await
            .is_empty()
    );
}

/// REQ-019-6: a node designates a physical or virtual machine, one living node at a time.
#[kartenn_testing::pg_test]
async fn node_machine_kind(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let container = env.machine("pod-like", "container").await;
    let appliance = env.machine("firewall", "appliance").await;
    let server = env.machine("server-1", "physical").await;
    let vm = env.machine("vm-1", "virtual").await;
    for machine in [container, appliance] {
        env.refused(
            vec![sync(
                "upsert",
                "node",
                prod,
                "worker-1",
                json!({"machine": {"id": machine}}),
            )],
            "kube.node.machine_kind",
        )
        .await;
    }
    env.applied(vec![
        sync(
            "upsert",
            "node",
            prod,
            "worker-1",
            json!({"machine": {"id": server}}),
        ),
        sync(
            "upsert",
            "node",
            prod,
            "worker-2",
            json!({"machine": {"id": vm}}),
        ),
    ])
    .await;
    // A machine designated by a living node already.
    env.refused(
        vec![sync(
            "upsert",
            "node",
            prod,
            "worker-3",
            json!({"machine": {"id": server}}),
        )],
        "kube.node.machine_kind",
    )
    .await;
    // The machine closed: no longer designated.
    env.applied(vec![Operation::Close {
        kind: ObjectKind::from_name("machines.machine").unwrap(),
        id: Ref::Id(vm),
        expected: None,
        cascade: true,
    }])
    .await;
    let machine: Option<Uuid> = sqlx::query_scalar(
        "SELECT machine_id FROM kube.node WHERE name = 'worker-2' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(machine, None);
}

/// The objects of a cluster are anchored in its pod domain: the domain does not change
/// under living nodes or namespaces (review of 019-3b).
#[kartenn_testing::pg_test]
async fn pod_domain_change_refused_with_living_objects(pool: PgPool) {
    let env = Env::new(&pool).await;
    let other = env
        .domain("pods-b", "192.0.2.0/24", Nature::Container)
        .await;
    let container = env.prefix(other).await;
    let prod = env.new_cluster("prod").await;
    let moved = || {
        op(
            "update_cluster",
            json!({"id": {"id": prod}, "pod_domain": {"id": other},
                   "pod_containers": [{"id": container}]}),
        )
    };
    // Without living objects: accepted, and back.
    env.applied(vec![moved()]).await;
    env.applied(vec![op(
        "update_cluster",
        json!({"id": {"id": prod}, "pod_domain": {"id": env.pods},
               "pod_containers": [{"id": env.container}]}),
    )])
    .await;
    env.applied(vec![sync(
        "upsert",
        "namespace",
        prod,
        "payments",
        json!({}),
    )])
    .await;
    env.refused(vec![moved()], "kube.operation.invalid").await;
    // The same domain given again is no change.
    env.applied(vec![op(
        "update_cluster",
        json!({"id": {"id": prod}, "pod_domain": {"id": env.pods}, "retention_days": 60}),
    )])
    .await;
    // Its objects closed, the domain may change.
    env.applied(vec![sync(
        "close",
        "namespace",
        prod,
        "payments",
        json!({}),
    )])
    .await;
    env.applied(vec![moved()]).await;
    let domain: Uuid = sqlx::query_scalar(
        "SELECT pod_domain_id FROM kube.cluster WHERE id = $1 AND valid_period @> now()",
    )
    .bind(prod)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(domain, other);
}

impl Env {
    /// The pod ranges and the cited networks of the living node `node`.
    async fn ranges(&self, node: Uuid) -> (Vec<String>, Vec<Uuid>) {
        sqlx::query_as(
            "SELECT pod_cidrs::text[], range_ids FROM kube.node
              WHERE id = $1 AND valid_period @> now()",
        )
        .bind(node)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The living networks of the pod domain, as `(id, prefix)`.
    async fn pod_networks(&self) -> Vec<(Uuid, String)> {
        sqlx::query_as(
            "SELECT id, prefix::text FROM prefix
              WHERE domain_id = $1 AND nature = 'network' AND valid_period @> now()
              ORDER BY prefix",
        )
        .bind(self.pods)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    /// Every network the pod domain ever had.
    async fn pod_networks_ever(&self) -> i64 {
        sqlx::query_scalar(
            "SELECT count(DISTINCT id) FROM prefix WHERE domain_id = $1 AND nature = 'network'",
        )
        .bind(self.pods)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

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
}

/// The warnings of the module (the core warns of the documentation prefixes of the tests).
fn warnings(outcome: &Outcome) -> Vec<&'static str> {
    match outcome {
        Outcome::Applied { warnings, .. } => warnings
            .iter()
            .map(|v| v.rule.id)
            .filter(|id| id.starts_with("kube."))
            .collect(),
        other => panic!("not applied: {other:?}"),
    }
}

/// REQ-019-13: the pod ranges of a node are networks of the pod domain, under a pod
/// container, created and cited by the node in the same `kube.sync`; a range unchanged
/// creates nothing, a range gone closes its network, a closed node closes its own; a
/// cited network does not close alone.
#[kartenn_testing::pg_test]
async fn node_range_created_and_closed(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let node = |cidrs: Value| {
        sync(
            "upsert",
            "node",
            prod,
            "worker-1",
            json!({"pod_cidrs": cidrs}),
        )
    };
    env.applied(vec![node(json!(["198.51.100.0/26"]))]).await;
    let worker = env.named("kube.node", prod, "worker-1").await[0];
    let networks = env.pod_networks().await;
    assert_eq!(networks.len(), 1, "{networks:?}");
    let (first, value) = networks[0].clone();
    assert_eq!(value, "198.51.100.0/26");
    assert_eq!(
        env.ranges(worker).await,
        (vec!["198.51.100.0/26".to_owned()], vec![first])
    );
    // The same ranges again, any other field changed: nothing created.
    env.applied(vec![sync(
        "upsert",
        "node",
        prod,
        "worker-1",
        json!({"pod_cidrs": ["198.51.100.0/26"], "zone": "zone-a"}),
    )])
    .await;
    assert_eq!(env.pod_networks().await, networks);
    assert_eq!(env.pod_networks_ever().await, 1);
    // A cited network does not close alone (D33).
    env.refused(
        vec![Operation::Close {
            kind: ObjectKind::Prefix,
            id: Ref::Id(first),
            expected: None,
            cascade: false,
        }],
        "core.object.dependents",
    )
    .await;
    // A range replaced: its network closes, the new one is created and cited.
    env.applied(vec![node(json!(["198.51.100.64/26", "2001:db8:0:1::/64"]))])
        .await;
    let networks = env.pod_networks().await;
    assert_eq!(networks.len(), 1, "the IPv6 range is outside: {networks:?}");
    let (second, value) = networks[0].clone();
    assert_eq!(value, "198.51.100.64/26");
    assert_ne!(second, first);
    assert_eq!(
        env.ranges(worker).await,
        (
            vec![
                "198.51.100.64/26".to_owned(),
                "2001:db8:0:1::/64".to_owned()
            ],
            vec![second]
        )
    );
    // The node closed: its networks with it.
    env.applied(vec![sync("close", "node", prod, "worker-1", json!({}))])
        .await;
    assert!(env.pod_networks().await.is_empty());
    assert_eq!(env.pod_networks_ever().await, 2);
    // A range out of syntax (host bits) refuses the batch.
    env.refused(
        vec![node(json!(["198.51.100.1/26"]))],
        "kube.operation.invalid",
    )
    .await;
}

/// REQ-019-13, design « Références » (task 019-3d): a cluster closed in a cascade closes
/// its nodes, and each node the networks of its pod ranges (`OnClose::Owned`); the cluster
/// declared again on the same domain and containers, `kube.sync` takes the ranges back.
#[kartenn_testing::pg_test]
async fn cluster_cascade_closes_node_networks(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let node = |cluster: Uuid, name: &str, cidr: &str| {
        sync(
            "upsert",
            "node",
            cluster,
            name,
            json!({"pod_cidrs": [cidr]}),
        )
    };
    env.applied(vec![
        node(prod, "worker-1", "198.51.100.0/26"),
        node(prod, "worker-2", "198.51.100.64/26"),
    ])
    .await;
    assert_eq!(env.pod_networks().await.len(), 2);
    env.applied(vec![Operation::Close {
        kind: ObjectKind::from_name("kube.cluster").unwrap(),
        id: Ref::Id(prod),
        expected: None,
        cascade: true,
    }])
    .await;
    assert!(env.named("kube.node", prod, "worker-1").await.is_empty());
    assert!(env.named("kube.node", prod, "worker-2").await.is_empty());
    assert!(env.pod_networks().await.is_empty());
    // Declared again, the cluster takes its ranges back: no duplicate network.
    let again = env.new_cluster("prod").await;
    assert_ne!(again, prod);
    env.applied(vec![
        node(again, "worker-1", "198.51.100.0/26"),
        node(again, "worker-2", "198.51.100.64/26"),
    ])
    .await;
    let networks: Vec<String> = env
        .pod_networks()
        .await
        .into_iter()
        .map(|(_, prefix)| prefix)
        .collect();
    assert_eq!(networks, ["198.51.100.0/26", "198.51.100.64/26"]);
    assert_eq!(env.pod_networks_ever().await, 4);
}

/// REQ-019-13: a pod range outside every pod container of the cluster gets no network and
/// warns, an anomaly on the node until the range is gone or inside.
#[kartenn_testing::pg_test]
async fn node_range_outside_warns(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let node = |cidrs: Value| {
        sync(
            "upsert",
            "node",
            prod,
            "worker-1",
            json!({"pod_cidrs": cidrs}),
        )
    };
    let outcome = env
        .submit(vec![node(json!(["192.0.2.0/26", "198.51.100.0/26"]))])
        .await;
    assert_eq!(warnings(&outcome), ["kube.node.range_outside"]);
    let worker = env.named("kube.node", prod, "worker-1").await[0];
    let networks = env.pod_networks().await;
    assert_eq!(
        networks.iter().map(|(_, p)| p.as_str()).collect::<Vec<_>>(),
        ["198.51.100.0/26"]
    );
    assert_eq!(
        env.ranges(worker).await,
        (
            vec!["192.0.2.0/26".to_owned(), "198.51.100.0/26".to_owned()],
            vec![networks[0].0]
        )
    );
    assert!(env.anomaly(worker, "kube.node.range_outside").await);
    // The range equal to the container is not inside it either.
    let outcome = env.submit(vec![node(json!(["198.51.100.0/24"]))]).await;
    assert_eq!(warnings(&outcome), ["kube.node.range_outside"]);
    assert!(env.pod_networks().await.is_empty());
    // The range gone: no warning, the anomaly closes.
    let outcome = env.submit(vec![node(json!(["198.51.100.128/26"]))]).await;
    assert!(warnings(&outcome).is_empty(), "{outcome:?}");
    assert!(!env.anomaly(worker, "kube.node.range_outside").await);
}

/// `kube.sync` of `action` on the workload of `kind` named `name` in the namespace
/// `namespace` of `cluster`.
fn workload(
    action: &str,
    cluster: Uuid,
    namespace: &str,
    kind: &str,
    name: &str,
    extra: Value,
) -> Operation {
    let mut fields = json!({"namespace": namespace, "kind": kind});
    merge(&mut fields, extra);
    sync(action, "workload", cluster, name, fields)
}

impl Env {
    /// The living workloads of the namespace `namespace`.
    async fn workloads(&self, namespace: Uuid) -> Vec<Uuid> {
        sqlx::query_scalar(
            "SELECT id FROM kube.workload
              WHERE namespace_id = $1 AND valid_period @> now() ORDER BY name, kind",
        )
        .bind(namespace)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    /// The UIDs of the versions of the workload `id`, oldest first.
    async fn workload_uids(&self, id: Uuid) -> Vec<Option<Uuid>> {
        sqlx::query_scalar(
            "SELECT uid FROM kube.workload WHERE id = $1 ORDER BY lower(valid_period)",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }
}

/// REQ-019-8, 019/X3: a workload is found by (namespace, kind, name); deleted and created
/// again under the same name (new UID), it stays the same workload and its history shows
/// the change of UID. Its tracked fields are the labels of its pod template, its service
/// account, its declared ports and `hostNetwork`.
#[kartenn_testing::pg_test]
async fn workload_recreated_keeps_identity(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    env.applied(vec![sync(
        "upsert",
        "namespace",
        prod,
        "payments",
        json!({}),
    )])
    .await;
    let payments = env.named("kube.namespace", prod, "payments").await[0];
    let web = |extra: Value| workload("upsert", prod, "payments", "deployment", "web", extra);
    let (first, second) = (Uuid::now_v7(), Uuid::now_v7());
    env.applied(vec![web(json!({
        "uid": first,
        "labels": {"app": "web"},
        "service_account": "web",
        "ports": [{"port": 8080, "name": "http"}, {"port": 53, "protocol": "udp"}],
        "host_network": false,
    }))])
    .await;
    let ids = env.workloads(payments).await;
    assert_eq!(ids.len(), 1);
    let (labels, account, ports, parent, domain): (
        Value,
        Option<String>,
        Value,
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT w.labels, w.service_account, w.ports, o.parent_id, o.domain_id
           FROM kube.workload w
           JOIN module_object o ON o.id = w.id AND o.valid_period @> now()
          WHERE w.id = $1 AND w.valid_period @> now()",
    )
    .bind(ids[0])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(labels, json!({"app": "web"}));
    assert_eq!(account.as_deref(), Some("web"));
    // Declared ports, a set sorted by port, `tcp` by default.
    assert_eq!(
        ports,
        json!([
            {"port": 53, "protocol": "udp"},
            {"port": 8080, "protocol": "tcp", "name": "http"},
        ])
    );
    // Under its namespace, in the pod domain of the cluster, it inherits its owner.
    assert_eq!((parent, domain), (Some(payments), Some(env.pods)));
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT kartenn_owner($1, now())")
        .bind(ids[0])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, Some(env.acme));
    // Deleted and created again in Kubernetes: a new UID, the same workload.
    env.applied(vec![web(json!({"uid": second}))]).await;
    assert_eq!(env.workloads(payments).await, ids);
    assert_eq!(env.workload_uids(ids[0]).await, [Some(first), Some(second)]);
    // The same kind and name twice in a batch: the second is a duplicate.
    env.refused(
        vec![
            workload("upsert", prod, "payments", "job", "migrate", json!({})),
            workload("upsert", prod, "payments", "job", "migrate", json!({})),
        ],
        "kube.workload.duplicate",
    )
    .await;
    // Another kind under the same name is another workload.
    env.applied(vec![workload(
        "upsert",
        prod,
        "payments",
        "statefulset",
        "web",
        json!({"host_network": true}),
    )])
    .await;
    assert_eq!(env.workloads(payments).await.len(), 2);
    // A namespace that does not live in the cluster, a kind, a port name, a protocol or a
    // field out of the type.
    let bad_kind = workload(
        "upsert",
        prod,
        "payments",
        "replicationcontroller",
        "web",
        json!({}),
    );
    for (operation, rule) in [
        (
            workload("upsert", prod, "billing", "deployment", "web", json!({})),
            "kube.operation.invalid",
        ),
        (bad_kind, "kube.operation.invalid"),
        (
            web(json!({"ports": [{"port": 8080, "name": "http--alt"}]})),
            "kube.name.invalid",
        ),
        (
            web(json!({"ports": [{"port": 8080, "protocol": "icmp"}]})),
            "kube.operation.invalid",
        ),
        (web(json!({"zone": "zone-a"})), "kube.operation.invalid"),
        (
            sync(
                "upsert",
                "namespace",
                prod,
                "payments",
                json!({"kind": "pod"}),
            ),
            "kube.operation.invalid",
        ),
    ] {
        env.refused(vec![operation], rule).await;
    }
}

/// REQ-019-8, design « Références »: a namespace with living workloads does not close
/// alone; closed in cascade, it closes them.
#[kartenn_testing::pg_test]
async fn namespace_cascade_closes_workloads(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    env.applied(vec![
        sync("upsert", "namespace", prod, "payments", json!({})),
        sync("upsert", "namespace", prod, "batch", json!({})),
    ])
    .await;
    let payments = env.named("kube.namespace", prod, "payments").await[0];
    let batch = env.named("kube.namespace", prod, "batch").await[0];
    env.applied(vec![
        workload("upsert", prod, "payments", "deployment", "web", json!({})),
        workload("upsert", prod, "payments", "cronjob", "report", json!({})),
        workload("upsert", prod, "batch", "job", "export", json!({})),
    ])
    .await;
    assert_eq!(env.workloads(payments).await.len(), 2);
    // The connector closes a namespace whose workloads live: refused.
    env.refused(
        vec![sync("close", "namespace", prod, "payments", json!({}))],
        "core.object.dependents",
    )
    .await;
    // In cascade, its workloads close with it.
    env.applied(vec![Operation::Close {
        kind: ObjectKind::from_name("kube.namespace").unwrap(),
        id: Ref::Id(payments),
        expected: None,
        cascade: true,
    }])
    .await;
    assert!(env.workloads(payments).await.is_empty());
    assert!(
        env.named("kube.namespace", prod, "payments")
            .await
            .is_empty()
    );
    // Its workloads closed first, a namespace closes in the same batch.
    env.applied(vec![
        workload("close", prod, "batch", "job", "export", json!({})),
        sync("close", "namespace", prod, "batch", json!({})),
    ])
    .await;
    assert!(env.workloads(batch).await.is_empty());
    assert!(env.named("kube.namespace", prod, "batch").await.is_empty());
}
