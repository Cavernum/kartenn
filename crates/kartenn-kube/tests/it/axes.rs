//! The workloads of a namespace, a service or a cluster, and the values of their axes
//! (spec 019, REQ-019-32, 019/X11; task 019-7a): along the tree of the module (workload,
//! namespace, cluster), else along the addressing tree on the pod containers of the
//! cluster, with the carrier that served.

use std::sync::Arc;

use kartenn_core::objects::Nature;
use kartenn_core::period::Instant;
use kartenn_kube::Kube;
use kartenn_kube::read::{Scope, Source, Sourced, axis_values, workloads_of};
use kartenn_machines::Machines;
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{ModuleDeclarations, core_declarations_for, sync_modules};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::{DeclarationInput, RawInput, TypeInput};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;

/// The axis of the tests: a closed list carried by the four carriers of the module and by
/// the addressing tree, heritable along it.
const EXPOSURE: &str = "custom.exposure";

struct Env {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    /// The service account of the connector, which submits the batches.
    connector: Uuid,
    /// The connector group of the clusters, the connector its only member.
    group: Uuid,
    acme: Uuid,
}

impl Env {
    /// Both modules migrated and declared, the entity `acme`.
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
            author,
            connector,
            group,
            acme: Uuid::nil(),
        };
        env.applied(vec![Operation::CreateEntity {
            local: None,
            slug: "acme".into(),
            parent: None,
        }])
        .await;
        env.acme = env.id("entity", "slug", "acme").await;
        env
    }

    /// `operations` applied, submitted by the connector when they are a batch.
    async fn applied(&self, operations: Vec<Operation>) {
        let author = if crate::support::batch(&operations) {
            self.connector
        } else {
            self.author
        };
        let request = ChangesetRequest {
            author,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        let outcome = submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
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

    /// A domain `slug` owned by `acme`, with the prefix `prefix` of `nature`: `(domain,
    /// prefix)`.
    async fn domain(&self, slug: &str, prefix: &str, nature: Nature) -> (Uuid, Uuid) {
        let owner = self.id("property_declaration", "key", "core.owner").await;
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
        let domain = self.id("addressing_domain", "slug", slug).await;
        (domain, self.id("prefix", "prefix::text", prefix).await)
    }

    /// A second container `prefix` in `domain`.
    async fn container(&self, domain: Uuid, prefix: &str) -> Uuid {
        self.applied(vec![Operation::CreatePrefix {
            local: None,
            domain: Ref::Id(domain),
            prefix: prefix.into(),
            nature: Nature::Container,
            point_to_point: false,
            anycast: false,
        }])
        .await;
        self.id("prefix", "prefix::text", prefix).await
    }

    /// A cluster `slug` on the pod domain `pods` with the pod containers `containers`,
    /// and a service domain of its own.
    async fn cluster(&self, slug: &str, pods: Uuid, containers: &[Uuid]) -> Uuid {
        let (services, network) = self
            .domain(
                &format!("{slug}-services"),
                "203.0.113.0/26",
                Nature::Network,
            )
            .await;
        let containers: Vec<Value> = containers.iter().map(|c| json!({"id": c})).collect();
        self.applied(vec![op(
            "kube.create_cluster",
            json!({
                "slug": slug,
                "owner": {"id": self.acme},
                "pod_domain": {"id": pods},
                "service_domain": {"id": services},
                "pod_containers": containers,
                "service_networks": [{"id": network}],
                "connector_group": {"id": self.group},
            }),
        )])
        .await;
        self.id("kube.cluster", "slug", slug).await
    }

    /// The living object of `table` whose `column` is `scope`, named `name`.
    async fn child(&self, table: &str, column: &str, scope: Uuid, name: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE {column} = $1 AND name = $2 AND valid_period @> now()"
        )))
        .bind(scope)
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// The instant of the database clock, after the writes so far.
    async fn now(&self) -> Instant {
        Instant(
            sqlx::query_scalar("SELECT (extract(epoch FROM clock_timestamp()) * 1000000)::bigint")
                .fetch_one(&self.pool)
                .await
                .unwrap(),
        )
    }

    /// The workloads of `scope` at `at`, as `(id, namespace, cluster, kind, name)`.
    async fn workloads(
        &self,
        scope: Scope,
        at: Instant,
    ) -> Vec<(Uuid, Uuid, Uuid, String, String)> {
        let mut conn = self.pool.acquire().await.unwrap();
        workloads_of(&mut conn, scope, at, None)
            .await
            .unwrap()
            .into_iter()
            .map(|w| (w.id, w.namespace, w.cluster, w.kind, w.name))
            .collect()
    }

    /// `custom.exposure` declared with the values `values`: its identifier.
    async fn exposure(&self, values: &[&str]) -> Uuid {
        let mut operations = vec![Operation::DeclareProperty {
            local: Some(1),
            declaration: DeclarationInput {
                key: EXPOSURE.into(),
                value_type: TypeInput::Enum { values_of: None },
                cardinality: kartenn_core::values::Cardinality::Single,
                carriers: [
                    "kube.cluster",
                    "kube.namespace",
                    "kube.workload",
                    "kube.service",
                    "domain",
                    "prefix",
                ]
                .iter()
                .map(|&c| c.to_owned())
                .collect(),
                inherit: Some("addressing".into()),
                module: None,
                obligations: Vec::new(),
                expected: None,
                labels: None,
            },
        }];
        operations.extend(values.iter().map(|&key| Operation::AddEnumValue {
            local: None,
            property: Ref::Local(1),
            key: key.into(),
            labels: std::collections::BTreeMap::new(),
        }));
        self.applied(operations).await;
        self.id("property_declaration", "key", EXPOSURE).await
    }

    /// The values of `custom.exposure` for `workloads` now, as `(value, source, origin,
    /// through)` per workload.
    async fn exposures(&self, workloads: &[Uuid], as_of: Option<Instant>) -> Vec<Vec<Sourced>> {
        let now = self.now().await;
        let mut conn = self.pool.acquire().await.unwrap();
        axis_values(&mut conn, workloads, &[EXPOSURE], now, as_of)
            .await
            .unwrap()
            .into_iter()
            .map(|(_, mut axes)| {
                assert_eq!(axes.len(), 1, "{axes:?}");
                let axis = axes.remove(0);
                assert_eq!(axis.key, EXPOSURE);
                axis.values
            })
            .collect()
    }
}

fn op(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = name.into();
    serde_json::from_value(object).unwrap()
}

/// `kube.sync` upserting the object of `kind` named `name` in `cluster`.
fn upsert(kind: &str, cluster: Uuid, name: &str, fields: Value) -> Operation {
    let mut object = json!({
        "action": "upsert",
        "type": format!("kube.{kind}"),
        "cluster": {"id": cluster},
        "name": name,
    });
    if let (Value::Object(into), Value::Object(extra)) = (&mut object, fields) {
        into.extend(extra);
    }
    op("kube.sync", object)
}

/// The deployment `name` of the namespace `namespace` of `cluster`.
fn deployment(cluster: Uuid, namespace: &str, name: &str) -> Operation {
    upsert(
        "workload",
        cluster,
        name,
        json!({"namespace": namespace, "kind": "deployment"}),
    )
}

/// The service `frontend` of the namespace `payments` of `cluster`, which selects the
/// deployment `web`.
fn frontend(cluster: Uuid) -> Operation {
    upsert(
        "service",
        cluster,
        "frontend",
        json!({"namespace": "payments", "workloads": [{"kind": "deployment", "name": "web"}]}),
    )
}

/// The value `value` of `property` on `object` of `kind`.
fn assign(object: Uuid, kind: ObjectKind, property: Uuid, value: &str) -> Operation {
    Operation::AssignProperty {
        object: Ref::Id(object),
        kind,
        property: Ref::Id(property),
        values: vec![RawInput::Text(value.into())],
    }
}

fn kube(kind: &str) -> ObjectKind {
    ObjectKind::from_name(&format!("kube.{kind}")).unwrap()
}

fn sourced(value: &str, source: Source, origin: Uuid, through: Uuid) -> Sourced {
    Sourced {
        value: json!(value),
        source,
        origin,
        through,
    }
}

/// REQ-019-32: a workload resolves its own value, else its namespace's, else its
/// cluster's, else the value of the addressing tree on each pod container of its cluster;
/// the source says which carrier served, at the dates of the read. The four carriers of
/// the module take a value.
#[kartenn_testing::pg_test]
async fn axis_from_namespace_then_tree(pool: PgPool) {
    let env = Env::new(&pool).await;
    let exposure = env.exposure(&["prod", "rec", "dev", "ops"]).await;
    let (pods, first) = env
        .domain("pods", "198.51.100.0/24", Nature::Container)
        .await;
    let second = env.container(pods, "192.0.2.0/24").await;
    let prod = env.cluster("prod", pods, &[first, second]).await;
    env.applied(vec![
        upsert("namespace", prod, "payments", json!({})),
        upsert("namespace", prod, "tools", json!({})),
    ])
    .await;
    env.applied(vec![
        deployment(prod, "payments", "web"),
        deployment(prod, "payments", "api"),
        deployment(prod, "tools", "probe"),
    ])
    .await;
    env.applied(vec![frontend(prod)]).await;
    let payments = env
        .child("kube.namespace", "cluster_id", prod, "payments")
        .await;
    let tools = env
        .child("kube.namespace", "cluster_id", prod, "tools")
        .await;
    let web = env
        .child("kube.workload", "namespace_id", payments, "web")
        .await;
    let api = env
        .child("kube.workload", "namespace_id", payments, "api")
        .await;
    let probe = env
        .child("kube.workload", "namespace_id", tools, "probe")
        .await;
    let frontend = env
        .child("kube.service", "namespace_id", payments, "frontend")
        .await;
    let all = [web, api, probe];
    // Nothing anywhere: no value.
    assert!(env.exposures(&all, None).await.iter().all(Vec::is_empty));
    // The pod domain: each container inherits it, and the workloads take it from both.
    env.applied(vec![assign(pods, ObjectKind::Domain, exposure, "dev")])
        .await;
    let tree = vec![
        sourced("dev", Source::Tree, pods, first),
        sourced("dev", Source::Tree, pods, second),
    ];
    assert_eq!(env.exposures(&[probe], None).await, [tree]);
    // A container of its own: each container gives its value.
    env.applied(vec![assign(second, ObjectKind::Prefix, exposure, "ops")])
        .await;
    let both = vec![
        sourced("dev", Source::Tree, pods, first),
        sourced("ops", Source::Tree, second, second),
    ];
    assert_eq!(
        env.exposures(&[probe], None).await,
        std::slice::from_ref(&both)
    );
    // The namespace `{prod}`: its workloads resolve `{prod}` (source: namespace); the
    // tree no longer serves them.
    env.applied(vec![assign(payments, kube("namespace"), exposure, "prod")])
        .await;
    let before_web = env.now().await;
    let from_namespace = vec![sourced("prod", Source::Namespace, payments, web)];
    assert_eq!(
        env.exposures(&[web], None).await,
        std::slice::from_ref(&from_namespace)
    );
    // The workload `{rec}`: `{rec}` (replacement, D26); its neighbour keeps `{prod}`.
    env.applied(vec![assign(web, kube("workload"), exposure, "rec")])
        .await;
    let before_cluster = env.now().await;
    // The cluster: the workload of a namespace without a value takes it.
    env.applied(vec![
        assign(prod, kube("cluster"), exposure, "ops"),
        assign(frontend, kube("service"), exposure, "prod"),
    ])
    .await;
    assert_eq!(
        env.exposures(&all, None).await,
        [
            vec![sourced("rec", Source::Workload, web, web)],
            vec![sourced("prod", Source::Namespace, payments, api)],
            vec![sourced("ops", Source::Cluster, prod, probe)],
        ]
    );
    // As asserted before the cluster had a value, then before the workload had one.
    assert_eq!(
        env.exposures(&[web, probe], Some(before_cluster)).await,
        [vec![sourced("rec", Source::Workload, web, web)], both]
    );
    assert_eq!(
        env.exposures(&[web], Some(before_web)).await,
        [from_namespace]
    );
}

/// REQ-019-32 (design « Port de lecture »): the workloads of a namespace, of a service and
/// of a cluster, and the values of a batch of them in one query: a workload unknown then
/// has no axis, a key not declared is absent, a key without a value has none.
#[kartenn_testing::pg_test]
async fn axis_values_one_query(pool: PgPool) {
    let env = Env::new(&pool).await;
    let exposure = env.exposure(&["prod"]).await;
    let (pods, container) = env
        .domain("pods", "198.51.100.0/24", Nature::Container)
        .await;
    let prod = env.cluster("prod", pods, &[container]).await;
    env.applied(vec![
        upsert("namespace", prod, "payments", json!({})),
        upsert("namespace", prod, "tools", json!({})),
    ])
    .await;
    let empty = env.now().await;
    env.applied(vec![
        deployment(prod, "payments", "web"),
        deployment(prod, "payments", "api"),
        deployment(prod, "tools", "probe"),
    ])
    .await;
    env.applied(vec![frontend(prod)]).await;
    let payments = env
        .child("kube.namespace", "cluster_id", prod, "payments")
        .await;
    let tools = env
        .child("kube.namespace", "cluster_id", prod, "tools")
        .await;
    let web = env
        .child("kube.workload", "namespace_id", payments, "web")
        .await;
    let api = env
        .child("kube.workload", "namespace_id", payments, "api")
        .await;
    let probe = env
        .child("kube.workload", "namespace_id", tools, "probe")
        .await;
    let frontend = env
        .child("kube.service", "namespace_id", payments, "frontend")
        .await;
    env.applied(vec![assign(web, kube("workload"), exposure, "prod")])
        .await;
    let now = env.now().await;
    let ids = |scope: Scope, at: Instant| env.workloads(scope, at);
    let listed = |id: Uuid, namespace: Uuid, name: &str| {
        (
            id,
            namespace,
            prod,
            "deployment".to_owned(),
            name.to_owned(),
        )
    };
    assert_eq!(
        ids(Scope::Namespace(payments), now).await,
        [listed(api, payments, "api"), listed(web, payments, "web")]
    );
    assert_eq!(
        ids(Scope::Service(frontend), now).await,
        [listed(web, payments, "web")]
    );
    assert_eq!(
        ids(Scope::Cluster(prod), now).await,
        [
            listed(api, payments, "api"),
            listed(web, payments, "web"),
            listed(probe, tools, "probe"),
        ]
    );
    // Before the workloads, or for an object of another kind: none.
    assert!(ids(Scope::Cluster(prod), empty).await.is_empty());
    assert!(ids(Scope::Namespace(prod), now).await.is_empty());
    // A batch of 512: the three workloads, then identifiers that name none.
    let mut conn = pool.acquire().await.unwrap();
    let mut batch = vec![web, api, probe];
    batch.extend((3..512).map(|_| Uuid::now_v7()));
    let found = axis_values(
        &mut conn,
        &batch,
        &[EXPOSURE, "custom.undeclared"],
        now,
        None,
    )
    .await
    .unwrap();
    assert_eq!(found.len(), 512);
    assert!(found.iter().zip(&batch).all(|((w, _), b)| w == b));
    let axes = |i: usize| {
        found[i]
            .1
            .iter()
            .map(|a| (a.key.as_str(), a.values.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        axes(0),
        [(EXPOSURE, vec![sourced("prod", Source::Workload, web, web)])]
    );
    assert_eq!(axes(1), [(EXPOSURE, Vec::new())]);
    assert_eq!(axes(2), [(EXPOSURE, Vec::new())]);
    assert!(found[3..].iter().all(|(_, axes)| axes.is_empty()));
    // Before the workloads lived: no axis.
    let before = axis_values(&mut conn, &batch[..3], &[EXPOSURE], empty, None)
        .await
        .unwrap();
    assert!(before.iter().all(|(_, axes)| axes.is_empty()));
}
