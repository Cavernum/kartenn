//! Clusters, nodes and namespaces through the write pipeline (spec 019, REQ-019-4 to 7;
//! task 019-3b): a cluster slug unique at every instant, its ranges in the domains it
//! designates, its batch window and connector group checked; the nodes and namespaces of
//! the connector found by their name in the cluster, unique there, a node designating one
//! physical or virtual machine; the workloads of a namespace found by their kind and name
//! there (task 019-4a), its services (task 019-4b) and ingresses (task 019-4c) by their
//! name.

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
    registry: Arc<ModuleRegistry>,
    author: Uuid,
    /// The service account of the connector, which submits the batches.
    connector: Uuid,
    /// The connector group of every cluster, the connector its only member.
    group: Uuid,
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
        let (connector, group) = crate::support::connector(pool, "connector").await;
        let mut env = Self {
            pool: pool.clone(),
            store,
            registry,
            author,
            connector,
            group,
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

    /// `operations` submitted by the connector when they are a batch, by the author
    /// otherwise.
    async fn submit(&self, operations: Vec<Operation>) -> Outcome {
        let author = if crate::support::batch(&operations) {
            self.connector
        } else {
            self.author
        };
        self.submit_by(author, operations).await
    }

    /// `operations` submitted by `author`, without RBAC.
    async fn submit_by(&self, author: Uuid, operations: Vec<Operation>) -> Outcome {
        let request = ChangesetRequest {
            author,
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

    /// `operations` submitted by `subject`, with its issuer and the groups of its session,
    /// without RBAC.
    async fn submit_in(
        &self,
        subject: &kartenn_service::authz::Subject,
        operations: Vec<Operation>,
    ) -> Outcome {
        let request = ChangesetRequest {
            author: subject.id,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        kartenn_service::submit_in(
            &self.store,
            &Unrestricted,
            &kartenn_service::expected::Sources::none(),
            &kartenn_service::audit::RequestContext::of(subject),
            &request,
            MAX_OPERATIONS,
        )
        .await
        .unwrap()
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
            approval_self: true,
        };
        let authorizer = kartenn_store_pg::rbac::PgAuthorizer::new(self.pool.clone())
            .with_modules(self.registry.clone());
        submit(&self.store, &authorizer, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    /// A new service account of the connector group, holding `permissions` on `acme` and
    /// what it owns.
    async fn granted(&self, permissions: &[&str]) -> Uuid {
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name, kind)
             VALUES (gen_random_uuid(), 'r-' || gen_random_uuid(), 'service_account')
             RETURNING id",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        crate::support::member(&self.pool, self.group, identity).await;
        let role = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO role (id, key, labels, valid_period, changeset_id)
             VALUES ($1, $2, jsonb_build_object('fr', 'r'),
                     tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
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
        sqlx::query(kartenn_testing::BIND_IDENTITY)
            .bind(identity)
            .bind(role)
            .bind("entity")
            .bind(self.acme)
            .execute(&self.pool)
            .await
            .unwrap();
        identity
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
            "connector_group": {"id": self.group},
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
    // Deleted in one window (closed, X9), created again in the next: the same workload
    // opens again after the gap, with the new UID, under its namespace.
    env.applied(vec![workload(
        "close",
        prod,
        "payments",
        "deployment",
        "web",
        json!({}),
    )])
    .await;
    assert!(env.workloads(payments).await.is_empty());
    let third = Uuid::now_v7();
    env.applied(vec![web(json!({"uid": third}))]).await;
    assert_eq!(env.workloads(payments).await, ids);
    assert_eq!(
        env.workload_uids(ids[0]).await,
        [Some(first), Some(second), Some(third)]
    );
    let (parent, owner): (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT o.parent_id, kartenn_owner(o.id, now())
           FROM module_object o WHERE o.id = $1 AND o.valid_period @> now()",
    )
    .bind(ids[0])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((parent, owner), (Some(payments), Some(env.acme)));
    // Opened again twice in a batch: the second is a duplicate.
    env.applied(vec![workload(
        "close",
        prod,
        "payments",
        "deployment",
        "web",
        json!({}),
    )])
    .await;
    env.refused(
        vec![web(json!({})), web(json!({}))],
        "kube.workload.duplicate",
    )
    .await;
    env.applied(vec![web(json!({}))]).await;
    assert_eq!(env.workloads(payments).await, ids);
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
/// alone; never closed by hand, it closes with its cluster in cascade, its workloads with
/// it.
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
    // Its workloads closed first, a namespace closes in the same batch.
    env.applied(vec![
        workload("close", prod, "batch", "job", "export", json!({})),
        sync("close", "namespace", prod, "batch", json!({})),
    ])
    .await;
    assert!(env.workloads(batch).await.is_empty());
    assert!(env.named("kube.namespace", prod, "batch").await.is_empty());
    // Closed in cascade by hand: refused, a mirror is never closed so (Q-019-18).
    env.refused(
        vec![Operation::Close {
            kind: ObjectKind::from_name("kube.namespace").unwrap(),
            id: Ref::Id(payments),
            expected: None,
            cascade: true,
        }],
        "kube.object.managed",
    )
    .await;
    // Its cluster closed in cascade: the namespace and its workloads with it.
    env.applied(vec![close_cluster(prod)]).await;
    assert!(env.workloads(payments).await.is_empty());
    assert!(
        env.named("kube.namespace", prod, "payments")
            .await
            .is_empty()
    );
}

/// The closure in cascade of the cluster `id`, which takes its mirrors with it.
fn close_cluster(id: Uuid) -> Operation {
    Operation::Close {
        kind: ObjectKind::from_name("kube.cluster").unwrap(),
        id: Ref::Id(id),
        expected: None,
        cascade: true,
    }
}

/// `kube.sync` of `action` on the service named `name` in the namespace `namespace` of
/// `cluster`.
fn service(action: &str, cluster: Uuid, namespace: &str, name: &str, extra: Value) -> Operation {
    let mut fields = json!({"namespace": namespace});
    merge(&mut fields, extra);
    sync(action, "service", cluster, name, fields)
}

/// The closure of the address `id` of the core.
fn close_address(id: Uuid) -> Operation {
    Operation::Close {
        kind: ObjectKind::Address,
        id: Ref::Id(id),
        expected: None,
        cascade: false,
    }
}

impl Env {
    /// A cluster `slug` with the external domain `external`, and its namespace `payments`.
    async fn service_cluster(&self, slug: &str, external: Uuid) -> (Uuid, Uuid) {
        self.applied(vec![
            self.cluster(slug, json!({"external_domain": {"id": external}})),
        ])
        .await;
        let cluster = self.id("kube.cluster", "slug", slug).await;
        self.applied(vec![sync(
            "upsert",
            "namespace",
            cluster,
            "payments",
            json!({}),
        )])
        .await;
        let payments = self.named("kube.namespace", cluster, "payments").await[0];
        (cluster, payments)
    }

    /// The living services of `namespace` named `name`.
    async fn services(&self, namespace: Uuid, name: &str) -> Vec<Uuid> {
        sqlx::query_scalar(
            "SELECT id FROM kube.service
              WHERE namespace_id = $1 AND name = $2 AND valid_period @> now()",
        )
        .bind(namespace)
        .bind(name)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    /// The cluster IPs and external addresses the living service `id` cites.
    async fn service_address_ids(&self, id: Uuid) -> (Vec<Uuid>, Vec<Uuid>) {
        sqlx::query_as(
            "SELECT cluster_address_ids, external_address_ids FROM kube.service
              WHERE id = $1 AND valid_period @> now()",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The living addresses of `domain`, as `(id, value, usage)`, by value.
    async fn addresses(&self, domain: Uuid) -> Vec<(Uuid, String, String)> {
        sqlx::query_as(
            "SELECT id, host(address), usage FROM address
              WHERE domain_id = $1 AND valid_period @> now() ORDER BY address",
        )
        .bind(domain)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    /// Every address `domain` ever had.
    async fn addresses_ever(&self, domain: Uuid) -> i64 {
        sqlx::query_scalar("SELECT count(DISTINCT id) FROM address WHERE domain_id = $1")
            .bind(domain)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// The workloads the living service `id` selects.
    async fn selected(&self, id: Uuid) -> Vec<Uuid> {
        sqlx::query_scalar(
            "SELECT workload_ids FROM kube.service WHERE id = $1 AND valid_period @> now()",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The living services that select the workload `id`: the other way of the selection.
    async fn selecting(&self, id: Uuid) -> Vec<Uuid> {
        sqlx::query_scalar(
            "SELECT id FROM kube.service
              WHERE workload_ids @> ARRAY[$1::uuid] AND valid_period @> now() ORDER BY name",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }
}

/// REQ-019-9, 14, design « Références »: the cluster IP of a service is an address of usage
/// `vip` of the service domain, its external address one of the external domain of the
/// cluster, created and cited in the same `kube.sync`; closing the cluster IP of a living
/// service is refused and names the service; an address unchanged creates nothing, an
/// address gone closes.
#[kartenn_testing::pg_test]
async fn service_vip_refuses_close(pool: PgPool) {
    let env = Env::new(&pool).await;
    let external = env
        .domain("external", "192.0.2.0/24", Nature::Network)
        .await;
    let (prod, payments) = env.service_cluster("prod", external).await;
    let web = |extra: Value| service("upsert", prod, "payments", "web", extra);
    env.applied(vec![web(json!({
        "uid": Uuid::now_v7(),
        "service_type": "load_balancer",
        "ports": [
            {"port": 443, "target_port": "https", "node_port": 30443, "app_protocol": "https"},
            {"port": 53, "target_port": 5353, "protocol": "udp", "name": "dns"},
        ],
        "cluster_ips": ["203.0.113.10"],
        "external_ips": ["192.0.2.10"],
    }))])
    .await;
    let ids = env.services(payments, "web").await;
    assert_eq!(ids.len(), 1);
    let (kind, ports, parent, domain): (String, Value, Option<Uuid>, Option<Uuid>) =
        sqlx::query_as(
            "SELECT s.type, s.ports, o.parent_id, o.domain_id
               FROM kube.service s
               JOIN module_object o ON o.id = s.id AND o.valid_period @> now()
              WHERE s.id = $1 AND s.valid_period @> now()",
        )
        .bind(ids[0])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kind, "load_balancer");
    // A set sorted by port, `tcp` by default.
    assert_eq!(
        ports,
        json!([
            {"port": 53, "target_port": 5353, "protocol": "udp", "name": "dns"},
            {"port": 443, "target_port": "https", "protocol": "tcp", "app_protocol": "https",
             "node_port": 30443},
        ])
    );
    // Under its namespace, in the service domain of the cluster.
    assert_eq!((parent, domain), (Some(payments), Some(env.services)));
    let cluster_ip = env.addresses(env.services).await;
    let external_ip = env.addresses(external).await;
    assert_eq!(cluster_ip.len(), 1, "{cluster_ip:?}");
    assert_eq!(
        (cluster_ip[0].1.as_str(), cluster_ip[0].2.as_str()),
        ("203.0.113.10", "vip")
    );
    assert_eq!(
        (external_ip[0].1.as_str(), external_ip[0].2.as_str()),
        ("192.0.2.10", "vip")
    );
    assert_eq!(
        env.service_address_ids(ids[0]).await,
        (vec![cluster_ip[0].0], vec![external_ip[0].0])
    );
    // The cluster IP of a living service does not close alone, and the refusal names the
    // service.
    let outcome = env.submit(vec![close_address(cluster_ip[0].0)]).await;
    let Outcome::Refused { violations, .. } = &outcome else {
        panic!("not refused: {outcome:?}");
    };
    let dependents = violations
        .iter()
        .find(|v| v.rule.id == "core.object.dependents")
        .unwrap_or_else(|| panic!("{outcome:?}"));
    assert!(dependents.subjects.contains(&ids[0]), "{dependents:?}");
    // The same addresses again, another field changed: nothing created.
    env.applied(vec![web(json!({
        "cluster_ips": ["203.0.113.10"],
        "external_ips": ["192.0.2.10"],
        "headless": false,
    }))])
    .await;
    assert_eq!(env.addresses_ever(env.services).await, 1);
    assert_eq!(env.addresses_ever(external).await, 1);
    // The external address replaced: the old one closes, the new one is cited.
    env.applied(vec![web(json!({"external_ips": ["192.0.2.11"]}))])
        .await;
    let replaced = env.addresses(external).await;
    assert_eq!(replaced.len(), 1, "{replaced:?}");
    assert_eq!(replaced[0].1, "192.0.2.11");
    assert_eq!(
        env.service_address_ids(ids[0]).await,
        (vec![cluster_ip[0].0], vec![replaced[0].0])
    );
    // An address out of syntax or a port out of bounds refuse the batch.
    env.refused(
        vec![web(json!({"cluster_ips": ["203.0.113.300"]}))],
        "kube.operation.invalid",
    )
    .await;
    env.refused(
        vec![web(json!({"ports": [{"port": 80, "node_port": 0}]}))],
        "kube.operation.invalid",
    )
    .await;
}

/// REQ-019-14, design « Données » (2026-11-10): without an external domain, the external
/// addresses of a service live in its service domain; a value that is both a cluster IP
/// and an external address is one address, cited by both columns.
#[kartenn_testing::pg_test]
async fn external_ips_fall_back_to_service_domain(pool: PgPool) {
    let env = Env::new(&pool).await;
    let dev = env.new_cluster("dev").await;
    env.applied(vec![sync(
        "upsert",
        "namespace",
        dev,
        "payments",
        json!({}),
    )])
    .await;
    let payments = env.named("kube.namespace", dev, "payments").await[0];
    env.applied(vec![service(
        "upsert",
        dev,
        "payments",
        "web",
        json!({
            "service_type": "load_balancer",
            "cluster_ips": ["203.0.113.20"],
            "external_ips": ["203.0.113.20", "203.0.113.21"],
        }),
    )])
    .await;
    let web = env.services(payments, "web").await[0];
    let addresses = env.addresses(env.services).await;
    let values: Vec<&str> = addresses.iter().map(|(_, v, _)| v.as_str()).collect();
    assert_eq!(values, ["203.0.113.20", "203.0.113.21"]);
    let (shared, other) = (addresses[0].0, addresses[1].0);
    let mut external = vec![shared, other];
    external.sort_unstable();
    assert_eq!(env.service_address_ids(web).await, (vec![shared], external));
    // The shared value no longer external: the address stays, cited as a cluster IP.
    env.applied(vec![service(
        "upsert",
        dev,
        "payments",
        "web",
        json!({"external_ips": ["203.0.113.21"]}),
    )])
    .await;
    assert_eq!(env.addresses(env.services).await, addresses);
    assert_eq!(
        env.service_address_ids(web).await,
        (vec![shared], vec![other])
    );
    // Closed: both addresses with it.
    env.applied(vec![service("close", dev, "payments", "web", json!({}))])
        .await;
    assert!(env.addresses(env.services).await.is_empty());
}

/// REQ-019-14, design « Références » (`OnClose::Owned`): a closed service closes its
/// addresses, by `kube.sync` or in the cascade of its cluster; deleted then created
/// again under the same name, it is the same service (019/X3), with new addresses.
#[kartenn_testing::pg_test]
async fn service_closed_closes_its_addresses(pool: PgPool) {
    let env = Env::new(&pool).await;
    let external = env
        .domain("external", "192.0.2.0/24", Nature::Network)
        .await;
    let (prod, payments) = env.service_cluster("prod", external).await;
    let web = service(
        "upsert",
        prod,
        "payments",
        "web",
        json!({
            "service_type": "load_balancer",
            "cluster_ips": ["203.0.113.10"],
            "external_ips": ["192.0.2.10"],
        }),
    );
    env.applied(vec![web.clone()]).await;
    let ids = env.services(payments, "web").await;
    assert_eq!(ids.len(), 1);
    // Closed by the connector: its addresses with it.
    env.applied(vec![service("close", prod, "payments", "web", json!({}))])
        .await;
    assert!(env.services(payments, "web").await.is_empty());
    assert!(env.addresses(env.services).await.is_empty());
    assert!(env.addresses(external).await.is_empty());
    // Created again: the same service, new addresses.
    env.applied(vec![web]).await;
    assert_eq!(env.services(payments, "web").await, ids);
    let (cluster_ips, external_ips) = env.service_address_ids(ids[0]).await;
    assert_eq!((cluster_ips.len(), external_ips.len()), (1, 1));
    assert_eq!(env.addresses_ever(env.services).await, 2);
    assert_eq!(env.addresses_ever(external).await, 2);
    // Its cluster closed in cascade: the service, then its addresses.
    env.applied(vec![close_cluster(prod)]).await;
    assert!(env.services(payments, "web").await.is_empty());
    assert!(env.addresses(env.services).await.is_empty());
    assert!(env.addresses(external).await.is_empty());
}

/// REQ-019-9: a service selecting two workloads lists them, each workload lists the
/// service; a workload closed by the connector leaves the selection
/// (design « Références », mise à nul), a workload unknown to the namespace refuses the
/// batch.
#[kartenn_testing::pg_test]
async fn selection_lists_both_ways(pool: PgPool) {
    let env = Env::new(&pool).await;
    let external = env
        .domain("external", "192.0.2.0/24", Nature::Network)
        .await;
    let (prod, payments) = env.service_cluster("prod", external).await;
    env.applied(vec![
        workload("upsert", prod, "payments", "deployment", "web", json!({})),
        workload("upsert", prod, "payments", "cronjob", "report", json!({})),
    ])
    .await;
    let workloads = env.workloads(payments).await;
    let (report, web) = (workloads[0], workloads[1]);
    let selecting = |name: &str, selection: Value| {
        service(
            "upsert",
            prod,
            "payments",
            name,
            json!({"workloads": selection}),
        )
    };
    env.applied(vec![
        selecting(
            "frontend",
            json!([
                {"kind": "deployment", "name": "web"},
                {"kind": "cronjob", "name": "report"},
            ]),
        ),
        selecting("metrics", json!([{"kind": "deployment", "name": "web"}])),
    ])
    .await;
    let frontend = env.services(payments, "frontend").await[0];
    let metrics = env.services(payments, "metrics").await[0];
    let mut both = vec![report, web];
    both.sort_unstable();
    assert_eq!(env.selected(frontend).await, both);
    assert_eq!(env.selected(metrics).await, [web]);
    assert_eq!(env.selecting(web).await, [frontend, metrics]);
    assert_eq!(env.selecting(report).await, [frontend]);
    // A workload the namespace does not hold: refused.
    env.refused(
        vec![selecting(
            "metrics",
            json!([{"kind": "statefulset", "name": "web"}]),
        )],
        "kube.operation.invalid",
    )
    .await;
    // An `unselect` sent by a client is not an action of `sync`.
    env.refused(
        vec![op(
            "sync",
            json!({"action": "unselect", "completed": {"unselect": [frontend, web, env.services]}}),
        )],
        "kube.operation.invalid",
    )
    .await;
    // Closed by the connector while selected: it leaves the selection of both services,
    // which live on.
    env.applied(vec![workload(
        "close",
        prod,
        "payments",
        "deployment",
        "web",
        json!({}),
    )])
    .await;
    assert_eq!(env.selected(frontend).await, [report]);
    assert!(env.selected(metrics).await.is_empty());
    assert!(env.selecting(web).await.is_empty());
    // A direct closure without cascade stays refused while a service selects it (D33).
    let close = |cascade: bool| Operation::Close {
        kind: ObjectKind::from_name("kube.workload").unwrap(),
        id: Ref::Id(report),
        expected: None,
        cascade,
    };
    env.refused(vec![close(false)], "core.object.dependents")
        .await;
    // In cascade by hand: refused, a mirror is never closed so (Q-019-18).
    env.refused(vec![close(true)], "kube.object.managed").await;
    assert_eq!(env.selected(frontend).await, [report]);
    // With its cluster closed in cascade, the selecting services close too.
    env.applied(vec![close_cluster(prod)]).await;
    assert!(env.services(payments, "frontend").await.is_empty());
}

/// `kube.sync` of `action` on the ingress named `name` in the namespace `namespace` of
/// `cluster`.
fn ingress(action: &str, cluster: Uuid, namespace: &str, name: &str, extra: Value) -> Operation {
    let mut fields = json!({"namespace": namespace});
    merge(&mut fields, extra);
    sync(action, "ingress", cluster, name, fields)
}

impl Env {
    /// The living ingresses of `namespace` named `name`.
    async fn ingresses(&self, namespace: Uuid, name: &str) -> Vec<Uuid> {
        sqlx::query_scalar(
            "SELECT id FROM kube.ingress
              WHERE namespace_id = $1 AND name = $2 AND valid_period @> now()",
        )
        .bind(namespace)
        .bind(name)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    /// The addresses the living ingress `id` cites.
    async fn ingress_address_ids(&self, id: Uuid) -> Vec<Uuid> {
        sqlx::query_scalar(
            "SELECT address_ids FROM kube.ingress WHERE id = $1 AND valid_period @> now()",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The parent and the anchor of the living object `id` of the module.
    async fn placed(&self, id: Uuid) -> (Option<Uuid>, Option<Uuid>) {
        sqlx::query_as(
            "SELECT parent_id, domain_id FROM module_object WHERE id = $1 AND valid_period @> now()",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

/// REQ-019-10: a route to a service absent from the namespace of the ingress warns
/// (`kube.ingress.backend_missing`) and is kept, as the cluster accepts it; the anomaly
/// closes once the service is created, opens again once it closes, closes once the route
/// is gone. Routes and TLS hosts are sets; a malformed route refuses the batch.
#[kartenn_testing::pg_test]
async fn ingress_backend_missing_warns(pool: PgPool) {
    let env = Env::new(&pool).await;
    let external = env
        .domain("external", "192.0.2.0/24", Nature::Network)
        .await;
    let (prod, payments) = env.service_cluster("prod", external).await;
    env.applied(vec![service(
        "upsert",
        prod,
        "payments",
        "web",
        json!({"ports": [{"port": 443, "name": "https"}]}),
    )])
    .await;
    let shop = |extra: Value| ingress("upsert", prod, "payments", "shop", extra);
    let outcome = env
        .submit(vec![shop(json!({
            "uid": Uuid::now_v7(),
            "class": "public",
            "rules": [
                {"host": "shop.example.org", "path": "/api", "path_type": "prefix",
                 "service": "api", "port": 8080},
                {"host": "shop.example.org", "path": "/", "path_type": "prefix",
                 "service": "web", "port": "https"},
                {"service": "web", "port": 443},
            ],
            "tls_hosts": ["shop.example.org", "*.example.net"],
        }))])
        .await;
    assert_eq!(warnings(&outcome), ["kube.ingress.backend_missing"]);
    let ids = env.ingresses(payments, "shop").await;
    assert_eq!(ids.len(), 1);
    let shop_id = ids[0];
    assert!(env.anomaly(shop_id, "kube.ingress.backend_missing").await);
    let (class, rules, tls): (Option<String>, Value, Vec<String>) = sqlx::query_as(
        "SELECT class, rules, tls_hosts FROM kube.ingress WHERE id = $1 AND valid_period @> now()",
    )
    .bind(shop_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(class.as_deref(), Some("public"));
    // A set: the default backend, without host, first.
    assert_eq!(
        rules,
        json!([
            {"service": "web", "port": 443},
            {"host": "shop.example.org", "path": "/", "path_type": "prefix",
             "service": "web", "port": "https"},
            {"host": "shop.example.org", "path": "/api", "path_type": "prefix",
             "service": "api", "port": 8080},
        ])
    );
    assert_eq!(tls, ["*.example.net", "shop.example.org"]);
    // Under its namespace, in the external domain of the cluster.
    assert_eq!(env.placed(shop_id).await, (Some(payments), Some(external)));
    // The service created: no warning, the anomaly closes.
    let outcome = env
        .submit(vec![service("upsert", prod, "payments", "api", json!({}))])
        .await;
    assert!(warnings(&outcome).is_empty(), "{outcome:?}");
    assert!(!env.anomaly(shop_id, "kube.ingress.backend_missing").await);
    // The service closed: the warning again.
    let outcome = env
        .submit(vec![service("close", prod, "payments", "api", json!({}))])
        .await;
    assert_eq!(warnings(&outcome), ["kube.ingress.backend_missing"]);
    assert!(env.anomaly(shop_id, "kube.ingress.backend_missing").await);
    // The route gone: no warning, the anomaly closes.
    let outcome = env
        .submit(vec![shop(
            json!({"rules": [{"service": "web", "port": 443}]}),
        )])
        .await;
    assert!(warnings(&outcome).is_empty(), "{outcome:?}");
    assert!(!env.anomaly(shop_id, "kube.ingress.backend_missing").await);
    // A host that is an IP, a relative path, a route of a host without path type, a field
    // of a service: refused.
    let route = |route: Value| shop(json!({ "rules": [route] }));
    env.refused(
        vec![route(
            json!({"host": "192.0.2.1", "path": "/", "path_type": "prefix",
                          "service": "web", "port": 443}),
        )],
        "kube.name.invalid",
    )
    .await;
    env.refused(
        vec![route(
            json!({"path": "api", "path_type": "exact", "service": "web", "port": 443}),
        )],
        "kube.operation.invalid",
    )
    .await;
    env.refused(
        vec![route(
            json!({"host": "shop.example.org", "service": "web", "port": 443}),
        )],
        "kube.operation.invalid",
    )
    .await;
    env.refused(
        vec![shop(json!({"external_ips": ["192.0.2.40"]}))],
        "kube.operation.invalid",
    )
    .await;
}

/// REQ-019-14, design « Références » (`OnClose::Owned`): the addresses of an ingress are
/// addresses of usage `vip` of the external domain of its cluster, created and cited in the
/// same `kube.sync`; one does not close alone while the ingress cites it; an address gone
/// closes; a closed ingress closes its addresses, by `kube.sync` or in the cascade of its
/// cluster; deleted then created again under the same name, it is the same ingress
/// (019/X3), with new addresses.
#[kartenn_testing::pg_test]
async fn ingress_closed_closes_its_addresses(pool: PgPool) {
    let env = Env::new(&pool).await;
    let external = env
        .domain("external", "192.0.2.0/24", Nature::Network)
        .await;
    let (prod, payments) = env.service_cluster("prod", external).await;
    env.applied(vec![service("upsert", prod, "payments", "web", json!({}))])
        .await;
    let shop = |addresses: Value| {
        ingress(
            "upsert",
            prod,
            "payments",
            "shop",
            json!({"addresses": addresses, "rules": [{"service": "web", "port": 80}]}),
        )
    };
    env.applied(vec![shop(json!(["192.0.2.30"]))]).await;
    let ids = env.ingresses(payments, "shop").await;
    assert_eq!(ids.len(), 1);
    let held = env.addresses(external).await;
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(
        (held[0].1.as_str(), held[0].2.as_str()),
        ("192.0.2.30", "vip")
    );
    assert_eq!(env.ingress_address_ids(ids[0]).await, [held[0].0]);
    // Its address does not close alone, and the refusal names the ingress.
    let outcome = env.submit(vec![close_address(held[0].0)]).await;
    let Outcome::Refused { violations, .. } = &outcome else {
        panic!("not refused: {outcome:?}");
    };
    let dependents = violations
        .iter()
        .find(|v| v.rule.id == "core.object.dependents")
        .unwrap_or_else(|| panic!("{outcome:?}"));
    assert!(dependents.subjects.contains(&ids[0]), "{dependents:?}");
    // The same address: nothing created; replaced: the old one closes.
    env.applied(vec![shop(json!(["192.0.2.30"]))]).await;
    assert_eq!(env.addresses_ever(external).await, 1);
    env.applied(vec![shop(json!(["192.0.2.31"]))]).await;
    let replaced = env.addresses(external).await;
    assert_eq!(replaced.len(), 1, "{replaced:?}");
    assert_eq!(replaced[0].1, "192.0.2.31");
    assert_eq!(env.ingress_address_ids(ids[0]).await, [replaced[0].0]);
    // Closed by the connector: its address with it.
    env.applied(vec![ingress("close", prod, "payments", "shop", json!({}))])
        .await;
    assert!(env.ingresses(payments, "shop").await.is_empty());
    assert!(env.addresses(external).await.is_empty());
    // Created again: the same ingress, a new address.
    env.applied(vec![shop(json!(["192.0.2.31"]))]).await;
    assert_eq!(env.ingresses(payments, "shop").await, ids);
    assert_eq!(env.ingress_address_ids(ids[0]).await.len(), 1);
    assert_eq!(env.addresses_ever(external).await, 3);
    // Its cluster closed in cascade: the ingress, then its address.
    env.applied(vec![close_cluster(prod)]).await;
    assert!(env.ingresses(payments, "shop").await.is_empty());
    assert!(env.addresses(external).await.is_empty());
}

/// REQ-019-14, design « Types d'objets »: without an external domain, the addresses of an
/// ingress live in the service domain of its cluster, where the ingress is anchored.
#[kartenn_testing::pg_test]
async fn ingress_addresses_fall_back_to_service_domain(pool: PgPool) {
    let env = Env::new(&pool).await;
    let dev = env.new_cluster("dev").await;
    env.applied(vec![sync(
        "upsert",
        "namespace",
        dev,
        "payments",
        json!({}),
    )])
    .await;
    let payments = env.named("kube.namespace", dev, "payments").await[0];
    env.applied(vec![ingress(
        "upsert",
        dev,
        "payments",
        "shop",
        json!({"addresses": ["203.0.113.40"]}),
    )])
    .await;
    let shop = env.ingresses(payments, "shop").await[0];
    let addresses = env.addresses(env.services).await;
    assert_eq!(addresses.len(), 1, "{addresses:?}");
    assert_eq!(addresses[0].1, "203.0.113.40");
    assert_eq!(env.ingress_address_ids(shop).await, [addresses[0].0]);
    assert_eq!(env.placed(shop).await, (Some(payments), Some(env.services)));
}

/// The `kube.sync` of a service or an ingress, as `service` and `ingress`.
type Living = fn(&str, Uuid, &str, &str, Value) -> Operation;

/// Services and ingresses are anchored in the service and external domains of their
/// cluster: neither domain changes under living ones, setting the external domain
/// included (Q-019-17, review of 019-4c; the rule of Q-019-15).
#[kartenn_testing::pg_test]
async fn service_domains_change_refused_with_living_objects(pool: PgPool) {
    let env = Env::new(&pool).await;
    let other = env
        .domain("services-b", "192.0.2.0/26", Nature::Network)
        .await;
    let network = env.prefix(other).await;
    let external = env
        .domain("external", "192.0.2.64/26", Nature::Network)
        .await;
    let dev = env.new_cluster("dev").await;
    let update = |fields: Value| {
        let mut body = json!({"id": {"id": dev}});
        merge(&mut body, fields);
        op("update_cluster", body)
    };
    let services_moved =
        || update(json!({"service_domain": {"id": other}, "service_networks": [{"id": network}]}));
    let services_back = || {
        update(json!({"service_domain": {"id": env.services},
                      "service_networks": [{"id": env.network}]}))
    };
    let external_set = || update(json!({"external_domain": {"id": external}}));
    let external_none = || update(json!({"external_domain": null}));
    // Without services nor ingresses, a living namespace aside: accepted, and back.
    env.applied(vec![sync(
        "upsert",
        "namespace",
        dev,
        "payments",
        json!({}),
    )])
    .await;
    env.applied(vec![services_moved()]).await;
    env.applied(vec![services_back()]).await;
    env.applied(vec![external_set()]).await;
    env.applied(vec![external_none()]).await;
    let living: [(Living, &str); 2] = [(service, "web"), (ingress, "shop")];
    for (living, name) in living {
        env.applied(vec![living("upsert", dev, "payments", name, json!({}))])
            .await;
        env.refused(vec![services_moved()], "kube.operation.invalid")
            .await;
        env.refused(vec![external_set()], "kube.operation.invalid")
            .await;
        // The same domains given again are no change.
        env.applied(vec![services_back(), external_none()]).await;
        env.applied(vec![living("close", dev, "payments", name, json!({}))])
            .await;
    }
    // Its services and ingresses closed, both domains may change.
    env.applied(vec![services_moved(), external_set()]).await;
    let domains: (Uuid, Option<Uuid>) = sqlx::query_as(
        "SELECT service_domain_id, external_domain_id FROM kube.cluster
          WHERE id = $1 AND valid_period @> now()",
    )
    .bind(dev)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(domains, (other, Some(external)));
}

/// An ingress `name` of `payments` in `cluster`, routed to the service `lb`, with
/// `addresses`.
fn routed(cluster: Uuid, name: &str, addresses: Value) -> Operation {
    let mut fields = json!({"rules": [{"service": "lb", "port": 80}]});
    fields["addresses"] = addresses;
    ingress("upsert", cluster, "payments", name, fields)
}

/// Q-019-17 (task 019-4e): the ingresses of a class have the IP of the `LoadBalancer` service
/// of their controller. Given by three holders of one batch, the address is created once;
/// a later holder cites it; it outlives the holders that close or give it up, and closes
/// with the last.
#[kartenn_testing::pg_test]
async fn vip_shared_across_holders(pool: PgPool) {
    let env = Env::new(&pool).await;
    let external = env
        .domain("external", "192.0.2.0/24", Nature::Network)
        .await;
    let (prod, payments) = env.service_cluster("prod", external).await;
    let lb = |extra: Value| service("upsert", prod, "payments", "lb", extra);
    env.applied(vec![
        lb(json!({"service_type": "load_balancer", "external_ips": ["192.0.2.50"]})),
        routed(prod, "shop", json!(["192.0.2.50"])),
        routed(prod, "blog", json!(["192.0.2.50"])),
    ])
    .await;
    let living = env.addresses(external).await;
    assert_eq!(living.len(), 1, "{living:?}");
    assert_eq!(env.addresses_ever(external).await, 1);
    let vip = living[0].0;
    let balancer = env.services(payments, "lb").await[0];
    let (shop, blog) = (
        env.ingresses(payments, "shop").await[0],
        env.ingresses(payments, "blog").await[0],
    );
    assert_eq!(env.service_address_ids(balancer).await.1, [vip]);
    assert_eq!(env.ingress_address_ids(shop).await, [vip]);
    assert_eq!(env.ingress_address_ids(blog).await, [vip]);
    // A holder of a later batch cites it.
    env.applied(vec![routed(prod, "news", json!(["192.0.2.50"]))])
        .await;
    let news = env.ingresses(payments, "news").await[0];
    assert_eq!(env.ingress_address_ids(news).await, [vip]);
    assert_eq!(env.addresses_ever(external).await, 1);
    // One holder closes, another gives it up: the others hold it still.
    env.applied(vec![
        ingress("close", prod, "payments", "shop", json!({})),
        routed(prod, "blog", json!([])),
    ])
    .await;
    assert!(env.ingress_address_ids(blog).await.is_empty());
    let living = async || {
        let addresses = env.addresses(external).await;
        (
            addresses.into_iter().map(|a| a.0).collect::<Vec<_>>(),
            env.addresses_ever(external).await,
        )
    };
    assert_eq!(living().await, (vec![vip], 1));
    env.applied(vec![ingress("close", prod, "payments", "news", json!({}))])
        .await;
    assert_eq!(living().await, (vec![vip], 1));
    // The last closes: the address with it.
    env.applied(vec![service("close", prod, "payments", "lb", json!({}))])
        .await;
    assert!(env.addresses(external).await.is_empty());
}

/// Q-019-17 (tasks 019-4e0, 4e): within one batch, an address one holder gives up and
/// another takes stays open; two holders moving from one address to another together close
/// the first and create the second once. An address no object of the cluster holds,
/// declared by hand or held in another cluster, is never adopted: refused as a duplicate.
#[kartenn_testing::pg_test]
async fn vip_handover_in_one_batch(pool: PgPool) {
    let env = Env::new(&pool).await;
    let external = env
        .domain("external", "192.0.2.0/24", Nature::Network)
        .await;
    let (prod, payments) = env.service_cluster("prod", external).await;
    env.applied(vec![routed(prod, "shop", json!(["192.0.2.60"]))])
        .await;
    let first = env.addresses(external).await[0].0;
    // The handover: `shop` gives `.60` up for `.61`, `blog` takes it.
    env.applied(vec![
        routed(prod, "shop", json!(["192.0.2.61"])),
        routed(prod, "blog", json!(["192.0.2.60"])),
    ])
    .await;
    let (shop, blog) = (
        env.ingresses(payments, "shop").await[0],
        env.ingresses(payments, "blog").await[0],
    );
    assert_eq!(env.ingress_address_ids(blog).await, [first]);
    let living = env.addresses(external).await;
    assert_eq!(
        living.iter().map(|a| a.1.as_str()).collect::<Vec<_>>(),
        ["192.0.2.60", "192.0.2.61"]
    );
    assert_eq!(env.addresses_ever(external).await, 2);
    // Both move to `.62` together, then to `.63`: one creation each time, the address they
    // leave closes.
    for (step, value) in [(3, "192.0.2.62"), (4, "192.0.2.63")] {
        env.applied(vec![
            routed(prod, "shop", json!([value])),
            routed(prod, "blog", json!([value])),
        ])
        .await;
        let living = env.addresses(external).await;
        assert_eq!(living.len(), 1, "{living:?}");
        assert_eq!(living[0].1, value);
        assert_eq!(env.ingress_address_ids(shop).await, [living[0].0]);
        assert_eq!(env.ingress_address_ids(blog).await, [living[0].0]);
        assert_eq!(env.addresses_ever(external).await, step);
    }
    // Declared by hand: not adopted.
    env.applied(vec![Operation::CreateAddress {
        local: None,
        domain: Ref::Id(external),
        address: "192.0.2.70".into(),
        usage: kartenn_core::objects::AddressUsage::Vip,
    }])
    .await;
    env.refused(
        vec![routed(prod, "news", json!(["192.0.2.70"]))],
        "core.address.duplicate",
    )
    .await;
    // Held in another cluster: not adopted either.
    let (test, _) = env.service_cluster("test", external).await;
    env.refused(
        vec![routed(test, "shop", json!(["192.0.2.63"]))],
        "core.address.duplicate",
    )
    .await;
    // A new value given in one batch by two clusters of one external domain: each creates
    // its own, not a share; the duplicate is refused.
    env.refused(
        vec![
            routed(prod, "news", json!(["192.0.2.64"])),
            routed(test, "news", json!(["192.0.2.64"])),
        ],
        "core.address.duplicate",
    )
    .await;
}

/// ADR 0026 (`reads`, 009/M16): an address another holder of the cluster cites is cited
/// without being created, so read: a writer of the ingresses who does not read the
/// addresses cannot attach an ingress to it.
#[kartenn_testing::pg_test]
async fn vip_shared_read_required(pool: PgPool) {
    let env = Env::new(&pool).await;
    let external = env
        .domain("external", "192.0.2.0/24", Nature::Network)
        .await;
    let (prod, payments) = env.service_cluster("prod", external).await;
    env.applied(vec![routed(prod, "shop", json!(["192.0.2.80"]))])
        .await;
    let vip = env.addresses(external).await[0].0;
    let writer = [
        "kube.cluster.read",
        "kube.namespace.read",
        "kube.service.read",
        "kube.ingress.read",
        "kube.ingress.edit",
        "kube.ingress.approval.self",
    ];
    let blind = env.granted(&writer).await;
    let outcome = env
        .submit_as(blind, vec![routed(prod, "blog", json!(["192.0.2.80"]))])
        .await;
    let Outcome::Refused { violations, .. } = &outcome else {
        panic!("not refused: {outcome:?}");
    };
    let forbidden = violations
        .iter()
        .find(|v| v.rule.id == "core.object.forbidden")
        .unwrap_or_else(|| panic!("{outcome:?}"));
    assert!(forbidden.subjects.contains(&vip), "{forbidden:?}");
    // Reading the addresses, the same writer attaches it.
    let reader = env
        .granted(&[&writer[..], &["core.address.read"]].concat())
        .await;
    let outcome = env
        .submit_as(reader, vec![routed(prod, "blog", json!(["192.0.2.80"]))])
        .await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let blog = env.ingresses(payments, "blog").await[0];
    assert_eq!(env.ingress_address_ids(blog).await, [vip]);
}

/// The workload `web` of `cluster`, in its namespace `payments`, labelled `app: app`.
fn web(cluster: Uuid, app: &str) -> Operation {
    workload(
        "upsert",
        cluster,
        "payments",
        "deployment",
        "web",
        json!({ "labels": {"app": app} }),
    )
}

/// The declaration of `custom.environment`, a text the workloads carry.
fn environment() -> Operation {
    use kartenn_service::operations::{DeclarationInput, TypeInput};
    Operation::DeclareProperty {
        local: None,
        declaration: DeclarationInput {
            key: "custom.environment".into(),
            value_type: TypeInput::Text {
                min_len: 0,
                max_len: 32,
                pattern: None,
            },
            cardinality: kartenn_core::values::Cardinality::Single,
            carriers: vec!["kube.workload".into()],
            inherit: None,
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels: None,
        },
    }
}

/// The value `value` of `property` set on the workload `object`.
fn environment_of(object: Uuid, property: Uuid, value: &str) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(object),
        kind: ObjectKind::from_name("kube.workload").unwrap(),
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    }
}

impl Env {
    /// The namespace `payments` of `cluster` and its workload `web`, written by the
    /// connector: the workload.
    async fn web(&self, cluster: Uuid) -> Uuid {
        self.applied(vec![sync(
            "upsert",
            "namespace",
            cluster,
            "payments",
            json!({}),
        )])
        .await;
        self.applied(vec![web(cluster, "web")]).await;
        let payments = self.named("kube.namespace", cluster, "payments").await[0];
        self.workloads(payments).await[0]
    }

    /// The labels of the living version of the workload `id`.
    async fn labels(&self, id: Uuid) -> Value {
        sqlx::query_scalar(
            "SELECT labels FROM kube.workload WHERE id = $1 AND valid_period @> now()",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// `operations` by `author`, refused under `rule`.
    async fn refused_by(&self, author: Uuid, operations: Vec<Operation>, rule: &str) {
        let outcome = self.submit_by(author, operations).await;
        assert!(rules(&outcome).contains(&rule), "{rule}: {outcome:?}");
    }
}

/// REQ-019-12, 30, 019/X5: the tracked fields of a mirror are written by a service account
/// of the connector group of its cluster alone: neither by an administrator, nor by the
/// connector of another cluster, even when both clusters share their domains.
#[kartenn_testing::pg_test]
async fn managed_field_refused_for_human(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let web_id = env.web(prod).await;
    env.refused_by(env.author, vec![web(prod, "shop")], "kube.object.managed")
        .await;
    // The connector of another cluster on the same domains.
    let (other, group) = crate::support::connector(&pool, "connector-test").await;
    env.applied(vec![
        env.cluster("test", json!({"connector_group": {"id": group}})),
    ])
    .await;
    env.refused_by(other, vec![web(prod, "shop")], "kube.object.managed")
        .await;
    assert_eq!(env.labels(web_id).await, json!({"app": "web"}));
    // The connector of the cluster writes them.
    env.applied(vec![web(prod, "shop")]).await;
    assert_eq!(env.labels(web_id).await, json!({"app": "shop"}));
}

/// REQ-019-12: a person added to the connector group is no service account: refused.
#[kartenn_testing::pg_test]
async fn managed_field_refused_for_person_in_connector_group(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let web_id = env.web(prod).await;
    crate::support::member(&pool, env.group, env.author).await;
    env.refused_by(env.author, vec![web(prod, "shop")], "kube.object.managed")
        .await;
    assert_eq!(env.labels(web_id).await, json!({"app": "web"}));
}

/// REQ-019-12: a property on a mirror is written under the ordinary rights, by its
/// administrator, its tracked fields untouched.
#[kartenn_testing::pg_test]
async fn property_on_mirror_accepted(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let web_id = env.web(prod).await;
    env.applied(vec![environment()]).await;
    let property = env
        .id("property_declaration", "key", "custom.environment")
        .await;
    env.applied(vec![environment_of(web_id, property, "production")])
        .await;
    assert_eq!(env.labels(web_id).await, json!({"app": "web"}));
}

/// REQ-019-30, 019/X10, Q-019-18: the connector writes by `kube.sync` alone: an operation
/// of the core or a closure in cascade it sends by hand is refused, alone or beside a
/// batch, and so for a member of its group by a group of its issuer; the plans `kube.sync`
/// adds itself (networks and addresses created and closed, a selection set to nothing)
/// pass.
#[kartenn_testing::pg_test]
async fn connector_direct_operation_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let web_id = env.web(prod).await;
    env.applied(vec![environment()]).await;
    let property = env
        .id("property_declaration", "key", "custom.environment")
        .await;
    let set = environment_of(web_id, property, "production");
    env.refused_by(env.connector, vec![set.clone()], "kube.connector.direct")
        .await;
    // A person member of the group by a group of its issuer, as its session carries it.
    let issuer = "https://idp.example.org";
    let person: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name, issuer) VALUES (gen_random_uuid(), 'p', $1)
         RETURNING id",
    )
    .bind(issuer)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO group_member (id, group_id, member_kind, issuer, group_name, valid_period,
                                   changeset_id)
         VALUES (gen_random_uuid(), $1, 'idp_group', $2, 'kube-ops',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(env.group)
    .bind(issuer)
    .execute(&pool)
    .await
    .unwrap();
    let subject = kartenn_service::authz::Subject {
        id: person,
        issuer: Some(issuer.into()),
        groups: vec!["kube-ops".into()],
    };
    let outcome = env.submit_in(&subject, vec![set.clone()]).await;
    assert!(
        rules(&outcome).contains(&"kube.connector.direct"),
        "{outcome:?}"
    );
    // Outside the group of its issuer, the same person writes it.
    let outside = kartenn_service::authz::Subject {
        groups: vec!["others".into()],
        ..subject
    };
    let outcome = env.submit_in(&outside, vec![set.clone()]).await;
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    env.refused_by(
        env.connector,
        vec![web(prod, "shop"), set],
        "kube.connector.direct",
    )
    .await;
    let payments = env.named("kube.namespace", prod, "payments").await[0];
    env.refused_by(
        env.connector,
        vec![Operation::Close {
            kind: ObjectKind::from_name("kube.namespace").unwrap(),
            id: Ref::Id(payments),
            expected: None,
            cascade: true,
        }],
        "kube.connector.direct",
    )
    .await;
    assert_eq!(env.workloads(payments).await, [web_id]);
    assert_eq!(env.labels(web_id).await, json!({"app": "web"}));
    // A node and its pod range: the network created by the batch, closed with the node.
    let node = |action: &str| {
        sync(
            action,
            "node",
            prod,
            "worker-1",
            json!({"pod_cidrs": ["198.51.100.0/26"]}),
        )
    };
    env.applied(vec![node("upsert")]).await;
    assert_eq!(env.pod_networks().await.len(), 1);
    env.applied(vec![sync("close", "node", prod, "worker-1", json!({}))])
        .await;
    assert!(env.pod_networks().await.is_empty());
    // A service selecting the workload, its cluster IP created by the batch; then the
    // workload gone: its selection is set to nothing by the same batch.
    env.applied(vec![service(
        "upsert",
        prod,
        "payments",
        "frontend",
        json!({
            "workloads": [{"kind": "deployment", "name": "web"}],
            "cluster_ips": ["203.0.113.10"],
        }),
    )])
    .await;
    let frontend = env.services(payments, "frontend").await[0];
    assert_eq!(env.selected(frontend).await, [web_id]);
    let ips = env.addresses(env.services).await;
    assert_eq!(ips.len(), 1, "{ips:?}");
    assert_eq!(env.service_address_ids(frontend).await.0, [ips[0].0]);
    env.applied(vec![workload(
        "close",
        prod,
        "payments",
        "deployment",
        "web",
        json!({}),
    )])
    .await;
    assert!(env.selected(frontend).await.is_empty());
}

/// 019/X5, Q-019-18: an administrator never closes a mirror by hand; the restoration that
/// undoes a batch passes, and so does the closure of its cluster in cascade, which takes
/// its mirrors with it.
#[kartenn_testing::pg_test]
async fn mirror_closed_by_hand_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let web_id = env.web(prod).await;
    let payments = env.named("kube.namespace", prod, "payments").await[0];
    let close = |kind: &str, id: Uuid, cascade: bool| Operation::Close {
        kind: ObjectKind::from_name(kind).unwrap(),
        id: Ref::Id(id),
        expected: None,
        cascade,
    };
    env.refused(
        vec![close("kube.workload", web_id, false)],
        "kube.object.managed",
    )
    .await;
    env.refused(
        vec![close("kube.namespace", payments, true)],
        "kube.object.managed",
    )
    .await;
    assert_eq!(env.workloads(payments).await, [web_id]);
    // A batch undone by the administrator: its workload restored as before.
    let Outcome::Applied { changeset, .. } = env.submit(vec![web(prod, "shop")]).await else {
        panic!("batch not applied");
    };
    assert_eq!(env.labels(web_id).await, json!({"app": "shop"}));
    let inverse = kartenn_service::undo::inverse(&env.store, changeset, env.author)
        .await
        .unwrap()
        .unwrap();
    assert!(inverse.conflicts.is_empty(), "{:?}", inverse.conflicts);
    env.applied(inverse.request.operations).await;
    assert_eq!(env.labels(web_id).await, json!({"app": "web"}));
    // The cluster closed in cascade: its mirrors with it.
    env.applied(vec![close("kube.cluster", prod, true)]).await;
    assert!(env.workloads(payments).await.is_empty());
    assert!(
        env.named("kube.namespace", prod, "payments")
            .await
            .is_empty()
    );
}

/// The service `frontend` of the namespace `payments` of `cluster`, selecting the
/// workloads `selection`.
fn frontend(cluster: Uuid, selection: &Value) -> Operation {
    service(
        "upsert",
        cluster,
        "payments",
        "frontend",
        json!({ "workloads": selection }),
    )
}

/// `action` (`mark_absent`, `clear_absent`) on the workload `web` of `cluster`.
fn web_absence(action: &str, cluster: Uuid) -> Operation {
    workload(action, cluster, "payments", "deployment", "web", json!({}))
}

impl Env {
    /// The instant the living version of `id` of `table` is absent since, if it is.
    async fn absent_since(&self, table: &str, id: Uuid) -> Option<String> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT absent_since::text FROM {table} WHERE id = $1 AND valid_period @> now()"
        )))
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The message of the open anomaly of `rule` on `object`.
    async fn anomaly_message(&self, object: Uuid, rule: &str) -> String {
        sqlx::query_scalar(
            "SELECT message FROM anomaly
              WHERE object_id = $1 AND rule_id = $2 AND closed_at IS NULL",
        )
        .bind(object)
        .bind(rule)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

/// REQ-019-21, 22, 019/X9: a workload gone from the cluster but cited (by the selection
/// of a service, and by a reference property of another object, as a flow cites it) is
/// marked absent by `kube.sync`, not closed: it stays with its dependents, absent since
/// the instant of observation, kept by a second observation, and its anomaly names its
/// dependents, evaluated anew when one of them stops citing it. Only the connector marks
/// it, by its identity alone.
#[kartenn_testing::pg_test]
async fn cited_workload_goes_absent(pool: PgPool) {
    use kartenn_service::operations::{DeclarationInput, TypeInput};
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let web_id = env.web(prod).await;
    let payments = env.named("kube.namespace", prod, "payments").await[0];
    env.applied(vec![
        frontend(prod, &json!([{"kind": "deployment", "name": "web"}])),
        workload("upsert", prod, "payments", "deployment", "api", json!({})),
    ])
    .await;
    let frontend_id = env.services(payments, "frontend").await[0];
    let api = env.workloads(payments).await[0];
    // A reference to the workload from another object: an administrator's property.
    env.applied(vec![Operation::DeclareProperty {
        local: None,
        declaration: DeclarationInput {
            key: "custom.backend".into(),
            value_type: TypeInput::Ref {
                target: "kube.workload".into(),
            },
            cardinality: kartenn_core::values::Cardinality::Single,
            carriers: vec!["kube.workload".into()],
            inherit: None,
            module: None,
            obligations: Vec::new(),
            expected: None,
            labels: None,
        },
    }])
    .await;
    let backend = env
        .id("property_declaration", "key", "custom.backend")
        .await;
    env.applied(vec![Operation::AssignProperty {
        object: Ref::Id(api),
        kind: ObjectKind::from_name("kube.workload").unwrap(),
        property: Ref::Id(backend),
        values: vec![RawInput::Text(web_id.to_string())],
    }])
    .await;
    // Only the connector marks a mirror absent, by its identity alone.
    env.refused_by(
        env.author,
        vec![web_absence("mark_absent", prod)],
        "kube.object.managed",
    )
    .await;
    env.refused(
        vec![workload(
            "mark_absent",
            prod,
            "payments",
            "deployment",
            "web",
            json!({"labels": {"app": "web"}}),
        )],
        "kube.operation.invalid",
    )
    .await;
    env.refused(
        vec![workload(
            "mark_absent",
            prod,
            "payments",
            "deployment",
            "gone",
            json!({}),
        )],
        "kube.operation.invalid",
    )
    .await;
    assert_eq!(env.absent_since("kube.workload", web_id).await, None);
    let outcome = env.submit(vec![web_absence("mark_absent", prod)]).await;
    assert_eq!(warnings(&outcome), ["kube.object.absent"]);
    // Absent, not closed: still living, still selected, still cited.
    let since = env.absent_since("kube.workload", web_id).await;
    assert!(since.is_some());
    assert!(env.workloads(payments).await.contains(&web_id));
    assert_eq!(env.selected(frontend_id).await, [web_id]);
    assert!(env.anomaly(web_id, "kube.object.absent").await);
    let message = env.anomaly_message(web_id, "kube.object.absent").await;
    assert!(
        message.contains(&format!("kube.service {frontend_id}")),
        "{message}"
    );
    assert!(
        message.contains(&format!("kube.workload {api}")),
        "{message}"
    );
    // Observed absent again (a full list repeated): absent since the first observation,
    // no version written, and no more for a present one cleared.
    let versions = env.workload_uids(web_id).await.len();
    env.applied(vec![web_absence("mark_absent", prod)]).await;
    assert_eq!(env.absent_since("kube.workload", web_id).await, since);
    assert_eq!(env.workload_uids(web_id).await.len(), versions);
    assert!(env.anomaly(web_id, "kube.object.absent").await);
    env.applied(vec![workload(
        "clear_absent",
        prod,
        "payments",
        "deployment",
        "api",
        json!({}),
    )])
    .await;
    assert_eq!(env.workload_uids(api).await.len(), 1);
    // The service stops selecting it: its anomaly names the property's carrier alone.
    env.applied(vec![frontend(prod, &json!([]))]).await;
    let message = env.anomaly_message(web_id, "kube.object.absent").await;
    assert!(!message.contains(&frontend_id.to_string()), "{message}");
    assert!(
        message.contains(&format!("kube.workload {api}")),
        "{message}"
    );
}

/// REQ-019-22, 019/X3, X9: an absent workload seen again in the cluster under its identity
/// (a new UID) is the same workload, present again, its anomaly closed; an absent service
/// cleared by `clear_absent` is present again, its anomaly and that of the ingress that
/// routes to it closed; an absent workload closed by `kube.sync` closes its anomaly.
#[kartenn_testing::pg_test]
async fn absent_cleared_on_return(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.new_cluster("prod").await;
    let web_id = env.web(prod).await;
    let payments = env.named("kube.namespace", prod, "payments").await[0];
    env.applied(vec![frontend(
        prod,
        &json!([{"kind": "deployment", "name": "web"}]),
    )])
    .await;
    let frontend_id = env.services(payments, "frontend").await[0];
    env.applied(vec![web_absence("mark_absent", prod)]).await;
    assert!(env.anomaly(web_id, "kube.object.absent").await);
    // Created again in the cluster: a new UID, the same workload, present.
    let uid = Uuid::now_v7();
    let outcome = env
        .submit(vec![workload(
            "upsert",
            prod,
            "payments",
            "deployment",
            "web",
            json!({"uid": uid, "labels": {"app": "web"}}),
        )])
        .await;
    assert!(warnings(&outcome).is_empty(), "{outcome:?}");
    assert_eq!(env.workloads(payments).await, [web_id]);
    assert_eq!(env.workload_uids(web_id).await.last(), Some(&Some(uid)));
    assert_eq!(env.absent_since("kube.workload", web_id).await, None);
    assert!(!env.anomaly(web_id, "kube.object.absent").await);
    // A service routed to by an ingress, absent: the ingress misses its backend.
    let outcome = env
        .submit(vec![ingress(
            "upsert",
            prod,
            "payments",
            "shop",
            json!({"rules": [{"service": "frontend", "port": 80}]}),
        )])
        .await;
    assert!(warnings(&outcome).is_empty(), "{outcome:?}");
    let shop = env.ingresses(payments, "shop").await[0];
    let absence = |action: &str| service(action, prod, "payments", "frontend", json!({}));
    let mut seen = warnings(&env.submit(vec![absence("mark_absent")]).await);
    seen.sort_unstable();
    assert_eq!(seen, ["kube.ingress.backend_missing", "kube.object.absent"]);
    let message = env.anomaly_message(frontend_id, "kube.object.absent").await;
    assert!(
        message.contains(&format!("kube.ingress {shop}")),
        "{message}"
    );
    assert!(env.anomaly(shop, "kube.ingress.backend_missing").await);
    let outcome = env.submit(vec![absence("clear_absent")]).await;
    assert!(warnings(&outcome).is_empty(), "{outcome:?}");
    assert_eq!(env.absent_since("kube.service", frontend_id).await, None);
    assert!(!env.anomaly(frontend_id, "kube.object.absent").await);
    assert!(!env.anomaly(shop, "kube.ingress.backend_missing").await);
    // Absent, then gone without dependent: closed by the connector, its anomaly with it.
    env.applied(vec![web_absence("mark_absent", prod)]).await;
    assert!(env.anomaly(web_id, "kube.object.absent").await);
    env.applied(vec![web_absence("close", prod)]).await;
    assert!(env.workloads(payments).await.is_empty());
    assert!(!env.anomaly(web_id, "kube.object.absent").await);
}

/// A row of the section `kube.objects` of a changeset: the object, its type, before,
/// after, and whether its period is open.
type ObjectRow = (Uuid, String, Option<Value>, Option<Value>, bool);

impl Env {
    /// `operations` applied: the rows of `kube.objects` of their changeset, in order.
    async fn objects_section(&self, operations: Vec<Operation>) -> Vec<ObjectRow> {
        let outcome = self.submit(operations).await;
        let Outcome::Applied { changeset, .. } = outcome else {
            panic!("{outcome:?}")
        };
        sqlx::query_as(
            "SELECT object_id, kind, before, after, upper_inf(period)
               FROM changeset_diff_section
              WHERE changeset_id = $1 AND section = 'kube.objects' ORDER BY seq",
        )
        .bind(changeset)
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }
}

/// Design « Section de différentiel », REQ-019-36: each mirror a changeset creates,
/// changes, marks absent or present again, or closes (in cascade too) is a row of
/// `kube.objects`, with its identity and tracked fields before and after; a cluster, a
/// property on a mirror or an unchanged object makes none. A simulation shows it too.
#[kartenn_testing::pg_test]
async fn section_lists_changed_objects(pool: PgPool) {
    let env = Env::new(&pool).await;
    // A cluster is declared, not mirrored.
    let rows = env
        .objects_section(vec![env.cluster("prod", json!({}))])
        .await;
    assert!(rows.is_empty(), "{rows:?}");
    let prod = env.id("kube.cluster", "slug", "prod").await;
    // Created: after only, with its identity.
    let rows = env
        .objects_section(vec![
            sync("upsert", "namespace", prod, "payments", json!({})),
            sync("upsert", "node", prod, "worker-1", json!({})),
        ])
        .await;
    let payments = env.named("kube.namespace", prod, "payments").await[0];
    let worker = env.named("kube.node", prod, "worker-1").await[0];
    let kinds: Vec<(Uuid, &str)> = rows.iter().map(|r| (r.0, r.1.as_str())).collect();
    assert_eq!(kinds, [(worker, "kube.node"), (payments, "kube.namespace")]);
    let rows = env.objects_section(vec![web(prod, "web")]).await;
    let web_id = env.workloads(payments).await[0];
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!((rows[0].0, rows[0].1.as_str()), (web_id, "kube.workload"));
    let (_, _, before, after, open) = &rows[0];
    assert!(before.is_none() && *open, "{rows:?}");
    let created = after.as_ref().unwrap();
    // A closed list of fields under stable names, whatever the columns (Q-019-19).
    assert_eq!(
        *created,
        json!({
            "namespace": payments, "kind": "deployment", "name": "web", "uid": null,
            "labels": {"app": "web"}, "service_account": null, "ports": [],
            "host_network": false, "absent_since": null,
        })
    );
    // Changed: its tracked fields before and after, from now on.
    let rows = env.objects_section(vec![web(prod, "shop")]).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let (object, kind, before, after, open) = &rows[0];
    assert_eq!(
        (*object, kind.as_str(), *open),
        (web_id, "kube.workload", true)
    );
    assert_eq!(before.as_ref().unwrap()["labels"], json!({"app": "web"}));
    assert_eq!(after.as_ref().unwrap()["labels"], json!({"app": "shop"}));
    // The same fields again: no row.
    assert!(
        env.objects_section(vec![web(prod, "shop")])
            .await
            .is_empty()
    );
    // A property on a mirror: the mirror unchanged.
    env.applied(vec![environment()]).await;
    let property = env
        .id("property_declaration", "key", "custom.environment")
        .await;
    let rows = env
        .objects_section(vec![environment_of(web_id, property, "prod")])
        .await;
    assert!(rows.is_empty(), "{rows:?}");
    // Marked absent, then present again: `absent_since` before and after.
    let rows = env
        .objects_section(vec![web_absence("mark_absent", prod)])
        .await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].2.as_ref().unwrap()["absent_since"], Value::Null);
    // RFC 3339 in UTC, whatever the time zone of the session.
    let since = rows[0].3.as_ref().unwrap()["absent_since"]
        .as_str()
        .expect("an instant")
        .to_owned();
    assert!(
        since.ends_with('Z') && since.as_bytes()[10] == b'T',
        "{since}"
    );
    // A second observation of absence keeps the first instant: nothing changes.
    let rows = env
        .objects_section(vec![web_absence("mark_absent", prod)])
        .await;
    assert!(rows.is_empty(), "{rows:?}");
    let rows = env
        .objects_section(vec![web_absence("clear_absent", prod)])
        .await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(rows[0].2.as_ref().unwrap()["absent_since"].is_string());
    assert_eq!(rows[0].3.as_ref().unwrap()["absent_since"], Value::Null);
    // Simulated, the closure in cascade of the cluster shows its mirrors closed.
    let request = ChangesetRequest {
        author: env.author,
        motive: "test".into(),
        period: None,
        operations: vec![close_cluster(prod)],
        simulate: true,
        idempotency_key: None,
        approval_self: true,
    };
    let simulated = submit(&env.store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    let Outcome::Simulated { sections, .. } = &simulated else {
        panic!("{simulated:?}")
    };
    let section = sections
        .iter()
        .find(|s| s.name == "kube.objects")
        .expect("the section of the mirrors");
    assert_eq!(section.count, 3, "{section:?}");
    // Applied, the closures: before only, from now on.
    let rows = env.objects_section(vec![close_cluster(prod)]).await;
    let kinds: Vec<(Uuid, &str)> = rows.iter().map(|r| (r.0, r.1.as_str())).collect();
    assert_eq!(
        kinds,
        [
            (worker, "kube.node"),
            (payments, "kube.namespace"),
            (web_id, "kube.workload")
        ]
    );
    for (_, _, before, after, open) in &rows {
        assert!(before.is_some() && after.is_none() && *open, "{rows:?}");
    }
    assert_eq!(
        rows[2].2.as_ref().unwrap()["labels"],
        json!({"app": "shop"})
    );
}
