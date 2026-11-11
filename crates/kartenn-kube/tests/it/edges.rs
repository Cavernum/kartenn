//! The edges of the module through the router (spec 019, REQ-019-11; task 019-4d): an
//! ingress routes to a service, a service selects its workloads and holds its addresses,
//! a node runs on its machine and holds its pod ranges; with `machines.hosted_on`, the
//! chain from an ingress to the hypervisor of a node, but for the link from a workload to
//! its nodes, which is observed (task 019-6).

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_core::objects::Nature;
use kartenn_kube::Kube;
use kartenn_machines::Machines;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{ModuleDeclarations, core_declarations_for, sync_modules};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::operations::RawInput;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use tower::ServiceExt;

/// Any `Bearer` token is this principal.
struct As(Principal);

impl Authenticator for As {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(self.0.clone().into()),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }
}

struct Env {
    pool: PgPool,
    registry: Arc<ModuleRegistry>,
    store: PgStore,
    author: Uuid,
    /// The service account of the connector, which submits the batches.
    connector: Uuid,
    /// The connector group of the cluster, the connector its only member.
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
            registry,
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
        let domain = self.id("addressing_domain", "slug", slug).await;
        (domain, self.id("prefix", "prefix::text", prefix).await)
    }

    /// The living object of `table` whose `column` is `value` in `scope`.
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

    /// A person granted `entries` everywhere, by a role of their own.
    async fn person(&self, entries: &[&str]) -> Uuid {
        let pool = &self.pool;
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let role = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO role (id, key, labels, valid_period, changeset_id)
             VALUES ($1, $2, jsonb_build_object('fr', 'r'), tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(format!("r_{}", role.simple()))
        .execute(pool)
        .await
        .unwrap();
        for entry in entries {
            sqlx::query(
                "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
                 VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
            )
            .bind(role)
            .bind(entry)
            .execute(pool)
            .await
            .unwrap();
        }
        sqlx::query(kartenn_testing::BIND_IDENTITY)
            .bind(identity)
            .bind(role)
            .bind("global")
            .bind(None::<Uuid>)
            .execute(pool)
            .await
            .unwrap();
        identity
    }

    /// The router of an instance with both modules, as the person `identity`.
    fn app(&self, identity: Uuid) -> Router {
        let api = Api {
            pool: self.pool.clone(),
            ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
            bound: Duration::from_secs(60),
            journal: PgAuditJournal::new(self.pool.clone(), Instance::new("test")),
            audit: ReadAudit::default(),
            sources: kartenn_service::expected::Sources::none(),
            outputs: Arc::from([]),
            modules: self.registry.clone(),
        };
        kartenn_rest::router(
            kartenn_rest::v1_with(api, Vec::new()),
            Arc::new(PgReadiness::new(self.pool.clone(), AuditStatus::default())),
            Settings::default(),
            Arc::new(Quotas::new(Rates::default())),
            Arc::new(As(Principal {
                identity,
                kind: IdentityKind::Person,
                issuer: None,
                groups: Vec::new(),
                method: Method::Bearer,
            })),
        )
    }
}

/// `<module>.<name>` from its JSON fields.
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

/// The edges of `object` read by `app` (with `dates`, a query string or empty), as
/// `(type, from, to)`, sorted.
async fn edges(app: &Router, object: Uuid, dates: &str) -> Vec<(String, Uuid, Uuid)> {
    let uri = kartenn_testing::applied_by_author(
        "GET",
        &format!("/v1/graph/edges?object={object}{dates}"),
    );
    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .header(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test"),
        )
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body}");
    let id = |end: &Value| end["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    let mut out: Vec<(String, Uuid, Uuid)> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["type"].as_str().unwrap().to_owned(),
                id(&e["from"]),
                id(&e["to"]),
            )
        })
        .collect();
    out.sort();
    out
}

fn edge(name: &str, from: Uuid, to: Uuid) -> (String, Uuid, Uuid) {
    (name.to_owned(), from, to)
}

/// REQ-019-11: `GET /v1/graph/edges?object=<workload>` gives the services that select
/// it; from an ingress, the edges lead to its service and addresses, from the service to
/// its workload and addresses, from a node to its machine and pod range, from the machine
/// to its hypervisor (`machines.hosted_on`). An edge whose other end the reader may not
/// read is left out; a past system time gives the edges asserted then.
#[kartenn_testing::pg_test]
async fn execution_chain_edges(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (pods, container) = env
        .domain("pods", "198.51.100.0/24", Nature::Container)
        .await;
    let (services, network) = env
        .domain("services", "203.0.113.0/26", Nature::Network)
        .await;
    let (external, _) = env
        .domain("external", "192.0.2.0/24", Nature::Network)
        .await;
    let owner = json!({"id": env.acme});
    env.applied(vec![
        op(
            "machines.create_machine",
            json!({"local": 1, "name": "hv-1", "kind": "physical", "owner": owner}),
        ),
        op(
            "machines.create_machine",
            json!({"name": "vm-1", "kind": "virtual", "owner": owner,
                   "host": {"machine": {"local": 1}}}),
        ),
        op(
            "kube.create_cluster",
            json!({
                "slug": "prod",
                "owner": owner,
                "pod_domain": {"id": pods},
                "service_domain": {"id": services},
                "external_domain": {"id": external},
                "pod_containers": [{"id": container}],
                "service_networks": [{"id": network}],
                "connector_group": {"id": env.group},
            }),
        ),
    ])
    .await;
    let hypervisor = env.id("machines.machine", "name", "hv-1").await;
    let vm = env.id("machines.machine", "name", "vm-1").await;
    let prod = env.id("kube.cluster", "slug", "prod").await;
    env.applied(vec![
        upsert(
            "node",
            prod,
            "worker-1",
            json!({"machine": {"id": vm}, "pod_cidrs": ["198.51.100.0/26"]}),
        ),
        upsert("namespace", prod, "payments", json!({})),
    ])
    .await;
    // A service selects the workloads living before its batch.
    env.applied(vec![upsert(
        "workload",
        prod,
        "web",
        json!({"namespace": "payments", "kind": "deployment"}),
    )])
    .await;
    let payments = env
        .child("kube.namespace", "cluster_id", prod, "payments")
        .await;
    let frontend = |workloads: Value| {
        upsert(
            "service",
            prod,
            "frontend",
            json!({
                "namespace": "payments",
                "workloads": workloads,
                "cluster_ips": ["203.0.113.10"],
                "external_ips": ["192.0.2.10"],
            }),
        )
    };
    env.applied(vec![
        frontend(json!([{"kind": "deployment", "name": "web"}])),
        upsert(
            "ingress",
            prod,
            "shop",
            json!({
                "namespace": "payments",
                "rules": [
                    {"host": "shop.example.org", "path": "/", "path_type": "prefix",
                     "service": "frontend", "port": 80},
                    {"service": "frontend", "port": 80},
                    {"service": "absent", "port": 80},
                ],
                "addresses": ["192.0.2.20"],
            }),
        ),
    ])
    .await;
    let node = env.child("kube.node", "cluster_id", prod, "worker-1").await;
    let web = env
        .child("kube.workload", "namespace_id", payments, "web")
        .await;
    let service = env
        .child("kube.service", "namespace_id", payments, "frontend")
        .await;
    let shop = env
        .child("kube.ingress", "namespace_id", payments, "shop")
        .await;
    let range = env.id("prefix", "prefix::text", "198.51.100.0/26").await;
    let address = |value: &'static str| env.id("address", "host(address)", value);
    let (cluster_ip, external_ip, ingress_ip) = (
        address("203.0.113.10").await,
        address("192.0.2.10").await,
        address("192.0.2.20").await,
    );
    let admin = env.app(
        env.person(&[
            "kube.node.read",
            "kube.workload.read",
            "kube.service.read",
            "kube.ingress.read",
            "machines.read",
            "core.prefix.read",
            "core.address.read",
        ])
        .await,
    );
    // The workload: the services that select it.
    assert_eq!(
        edges(&admin, web, "").await,
        [edge("kube.selects", service, web)]
    );
    // The ingress: its service, once for its two routes to it (the absent one makes no
    // edge), and its address.
    let mut expected = vec![
        edge("kube.ingress_address", shop, ingress_ip),
        edge("kube.routes_to", shop, service),
    ];
    expected.sort();
    assert_eq!(edges(&admin, shop, "").await, expected);
    // The service: the ingress that routes to it, its workload and its addresses.
    let mut expected = vec![
        edge("kube.routes_to", shop, service),
        edge("kube.selects", service, web),
        edge("kube.service_address", service, cluster_ip),
        edge("kube.service_address", service, external_ip),
    ];
    expected.sort();
    assert_eq!(edges(&admin, service, "").await, expected);
    assert_eq!(
        edges(&admin, cluster_ip, "").await,
        [edge("kube.service_address", service, cluster_ip)]
    );
    // The node: its machine and its pod range; the machine: its node and its hypervisor.
    let mut expected = vec![
        edge("kube.node_range", node, range),
        edge("kube.runs_on", node, vm),
    ];
    expected.sort();
    assert_eq!(edges(&admin, node, "").await, expected);
    let mut expected = vec![
        edge("kube.runs_on", node, vm),
        edge("machines.hosted_on", vm, hypervisor),
    ];
    expected.sort();
    assert_eq!(edges(&admin, vm, "").await, expected);
    // A reader of the workloads only: the service end is not readable.
    let reader = env.app(env.person(&["kube.workload.read"]).await);
    assert!(edges(&reader, web, "").await.is_empty());
    // The workload left out of the selection: no edge now, the edge as asserted before.
    let before: String = sqlx::query_scalar("SELECT to_json(now())#>>'{}'")
        .fetch_one(&pool)
        .await
        .unwrap();
    env.applied(vec![frontend(json!([]))]).await;
    assert!(edges(&admin, web, "").await.is_empty());
    let dates = format!("&valid_at={0}&as_of={0}", before.replace('+', "%2B"));
    assert_eq!(
        edges(&admin, web, &dates).await,
        [edge("kube.selects", service, web)]
    );
}
