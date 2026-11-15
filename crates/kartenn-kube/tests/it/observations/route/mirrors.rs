//! The reads of the mirrored objects (task 019-8b; REQ-019-38, 39; design « Routes »):
//! `GET /v1/kube-nodes`, `/v1/kube-namespaces`, `/v1/workloads`, `/v1/kube-services`,
//! `/v1/ingresses` and `/{id}` of each, filtered, dated and journaled, an object the caller
//! may not read answered as an absent one.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_service::authn::IdentityKind;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::rbac::sync_roles;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use tower::ServiceExt;

use super::super::{Env, op, sync};
use super::{app, audited, person, person_on, principal, rfc3339, summary};

/// The summary of the cluster `id` whose slug is `slug` (spec 006, REQ-006-42).
fn cluster(id: Uuid, slug: &str) -> Value {
    summary("kube-clusters", id, slug)
}

/// The answer of `GET uri`.
async fn get(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Value) {
    let request = Request::builder()
        .uri(uri)
        .header(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test"),
        )
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, value)
}

/// The page of `uri`, answered 200.
async fn page(app: &Router, uri: &str) -> Value {
    let (status, _, page) = get(app, uri).await;
    assert_eq!(status, StatusCode::OK, "{uri}: {page}");
    page
}

/// The names of a page, in its order.
fn names(page: &Value) -> Vec<&str> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect()
}

/// The status of `uri`.
async fn status(app: &Router, uri: &str) -> StatusCode {
    get(app, uri).await.0
}

/// `kube.sync` doing `action` on the object of `kind` named `name` in `cluster`.
fn synced(action: &str, kind: &str, cluster: Uuid, name: &str, extra: Value) -> serde_json::Value {
    let mut fields = json!({
        "action": action,
        "type": format!("kube.{kind}"),
        "cluster": {"id": cluster},
        "name": name,
    });
    if let (Value::Object(into), Value::Object(extra)) = (&mut fields, extra) {
        into.extend(extra);
    }
    fields
}

/// The living namespace `name` of `cluster`.
async fn namespace(pool: &PgPool, cluster: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT id FROM kube.namespace
          WHERE cluster_id = $1 AND name = $2 AND valid_period @> now()",
    )
    .bind(cluster)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

impl Env {
    /// In `prod`: the namespace `shop` (label `app=shop`), the statefulset `db` of
    /// `payments` (label `app=db`), its service `frontend` (`node_port`) and ingress `shop`.
    async fn furnished(&self) {
        self.applied(
            self.connector,
            vec![sync(
                "namespace",
                self.prod,
                "shop",
                json!({"labels": {"app": "shop"}}),
            )],
        )
        .await;
        self.applied(
            self.connector,
            vec![
                sync(
                    "workload",
                    self.prod,
                    "db",
                    json!({"namespace": "payments", "kind": "statefulset",
                           "labels": {"app": "db"},
                           "ports": [{"port": 5432, "protocol": "tcp", "name": "pg"}]}),
                ),
                sync(
                    "service",
                    self.prod,
                    "frontend",
                    json!({"namespace": "payments", "service_type": "node_port",
                           "ports": [{"port": 80, "target_port": "http", "node_port": 30080}]}),
                ),
            ],
        )
        .await;
        self.applied(
            self.connector,
            vec![sync(
                "ingress",
                self.prod,
                "shop",
                json!({"namespace": "payments",
                       "rules": [{"service": "frontend", "port": 80}]}),
            )],
        )
        .await;
    }
}

/// REQ-019-38: each collection listed by name, filtered by cluster, namespace, name, kind,
/// tracked label, machine and absence, in conjunction; each object read by its `{id}`,
/// equal to its item; an unknown filter or a value that does not read refused 400.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // the five collections and their filters
async fn mirrors_listed_by_type_and_filter(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    let stage = env.cluster("stage", group).await;
    env.populated(stage, other, "198.51.100.64/26").await;
    env.furnished().await;
    let admin = person(&pool, &["*"]).await;
    let app = app(&pool, principal(admin, IdentityKind::Person), None);
    let payments = namespace(&pool, env.prod, "payments").await;

    // Nodes: by cluster, by name, by machine.
    let nodes = page(&app, "/v1/kube-nodes").await;
    assert_eq!(names(&nodes), ["worker-1", "worker-1"]);
    let nodes = page(&app, &format!("/v1/kube-nodes?cluster={}", env.prod)).await;
    let node = &nodes["items"][0];
    assert_eq!(nodes["items"].as_array().unwrap().len(), 1, "{nodes}");
    assert_eq!(node["id"], json!(env.node));
    assert_eq!(node["cluster"], cluster(env.prod, "prod"));
    assert_eq!(node["pod_cidrs"], json!(["198.51.100.0/26"]));
    assert!(
        node["addresses"]
            .as_array()
            .unwrap()
            .contains(&json!({"type": "InternalIP", "value": "192.0.2.21"})),
        "{node}"
    );
    assert_eq!(node["absent_since"], Value::Null);
    assert_eq!(node["href"], format!("/v1/kube-nodes/{}", env.node));
    let at_stage = page(
        &app,
        &format!("/v1/kube-nodes?name=worker-1&cluster={stage}"),
    )
    .await;
    assert_eq!(at_stage["items"][0]["cluster"], cluster(stage, "stage"));
    let none = page(&app, &format!("/v1/kube-nodes?machine={}", Uuid::now_v7())).await;
    assert!(names(&none).is_empty());

    // Namespaces: by cluster, by tracked label.
    let all = page(&app, "/v1/kube-namespaces").await;
    assert_eq!(names(&all), ["payments", "payments", "shop"]);
    let prod = page(&app, &format!("/v1/kube-namespaces?cluster={}", env.prod)).await;
    assert_eq!(names(&prod), ["payments", "shop"]);
    let labelled = page(&app, "/v1/kube-namespaces?label=app=shop").await;
    assert_eq!(names(&labelled), ["shop"]);
    assert_eq!(labelled["items"][0]["labels"], json!({"app": "shop"}));

    // Workloads: by kind, namespace, cluster, label, suggestion; the cluster of their
    // namespace shown.
    let all = page(&app, "/v1/workloads").await;
    assert_eq!(names(&all), ["db", "web", "web"]);
    let stateful = page(&app, "/v1/workloads?kind=statefulset").await;
    assert_eq!(names(&stateful), ["db"]);
    let db = &stateful["items"][0];
    assert_eq!(
        (&db["cluster"], &db["namespace"]),
        (
            &cluster(env.prod, "prod"),
            &summary("kube-namespaces", payments, "payments")
        )
    );
    assert_eq!(
        db["ports"],
        json!([{"port": 5432, "protocol": "tcp", "name": "pg"}])
    );
    let in_payments = page(&app, &format!("/v1/workloads?namespace={payments}")).await;
    assert_eq!(names(&in_payments), ["db", "web"]);
    let at_stage = page(&app, &format!("/v1/workloads?cluster={stage}")).await;
    assert_eq!(names(&at_stage), ["web"]);
    assert_eq!(at_stage["items"][0]["cluster"], cluster(stage, "stage"));
    let labelled = page(&app, "/v1/workloads?label=app=db").await;
    assert_eq!(names(&labelled), ["db"]);
    let suggested = page(&app, "/v1/workloads?q=d").await;
    assert_eq!(names(&suggested), ["db"]);
    let both = page(
        &app,
        &format!("/v1/workloads?cluster={}&kind=deployment", env.prod),
    )
    .await;
    assert_eq!(names(&both), ["web"]);

    // Services, by type; ingresses, by namespace.
    let services = page(&app, "/v1/kube-services?type=node_port").await;
    assert_eq!(names(&services), ["frontend"]);
    let frontend = &services["items"][0];
    assert_eq!(frontend["type"], "node_port");
    assert_eq!(frontend["ports"][0]["target_port"], "http");
    assert_eq!(frontend["ports"][0]["node_port"], 30080);
    let none = page(&app, "/v1/kube-services?type=cluster_ip").await;
    assert!(names(&none).is_empty());
    let ingresses = page(&app, &format!("/v1/ingresses?namespace={payments}")).await;
    assert_eq!(names(&ingresses), ["shop"]);
    assert_eq!(
        ingresses["items"][0]["rules"],
        json!([{"host": null, "path": null, "path_type": null,
                "service": "frontend", "port": 80}])
    );

    // Each object read by its `{id}`: its item, with its version.
    for (collection, item) in [
        ("kube-nodes", node.clone()),
        ("workloads", db.clone()),
        ("kube-services", frontend.clone()),
        ("ingresses", ingresses["items"][0].clone()),
        ("kube-namespaces", labelled_namespace(&app).await),
    ] {
        let uri = format!("/v1/{collection}/{}", item["id"].as_str().unwrap());
        let (code, headers, body) = get(&app, &uri).await;
        assert_eq!(code, StatusCode::OK, "{uri}: {body}");
        assert!(headers.contains_key(header::ETAG), "{uri}");
        assert_eq!(body, item, "{uri}");
        let unknown = format!("/v1/{collection}/{}", Uuid::now_v7());
        assert_eq!(status(&app, &unknown).await, StatusCode::NOT_FOUND);
    }

    // Absent from its cluster: found by `absent_since:exists`.
    env.applied(
        env.connector,
        vec![op(
            "sync",
            synced(
                "mark_absent",
                "workload",
                env.prod,
                "web",
                json!({"namespace": "payments", "kind": "deployment"}),
            ),
        )],
    )
    .await;
    let absent = page(&app, "/v1/workloads?absent_since:exists=true").await;
    assert_eq!(names(&absent), ["web"]);
    assert_eq!(absent["items"][0]["id"], json!(env.web));
    assert!(absent["items"][0]["absent_since"].is_string(), "{absent}");
    let present = page(&app, "/v1/workloads?absent_since:exists=false").await;
    assert_eq!(names(&present), ["db", "web"]);
    assert_eq!(present["items"][1]["cluster"], cluster(stage, "stage"));

    // Refused: a filter a collection does not serve, a value that does not read, a filter
    // given twice.
    for uri in [
        "/v1/kube-services?kind=deployment".to_owned(),
        "/v1/kube-namespaces?machine=x".to_owned(),
        "/v1/kube-nodes?namespace=x".to_owned(),
        "/v1/workloads?kind=nope".to_owned(),
        "/v1/workloads?label=app".to_owned(),
        "/v1/workloads?label==db".to_owned(),
        "/v1/workloads?cluster=x".to_owned(),
        "/v1/workloads?absent_since:exists=maybe".to_owned(),
        "/v1/workloads?name=a&name=b".to_owned(),
        format!("/v1/kube-nodes?machine={}&machine={}", env.node, env.node),
    ] {
        assert_eq!(status(&app, &uri).await, StatusCode::BAD_REQUEST, "{uri}");
    }
}

/// The namespace `shop`, as its list shows it.
async fn labelled_namespace(app: &Router) -> Value {
    page(app, "/v1/kube-namespaces?label=app=shop").await["items"][0].clone()
}

/// REQ-019-38 (lectures datées), REQ-012-18: a list at a `valid_at` before an object and
/// as asserted before it misses it; each read answered is journaled with what it returned
/// and its question.
#[kartenn_testing::pg_test]
async fn mirror_lists_dated_and_journaled(pool: PgPool) {
    let env = Env::new(&pool).await;
    let admin = person(&pool, &["*"]).await;
    let app = app(&pool, principal(admin, IdentityKind::Person), None);
    let before = OffsetDateTime::now_utc();
    env.furnished().await;

    let now = page(&app, "/v1/workloads").await;
    assert_eq!(names(&now), ["db", "web"]);
    let at = rfc3339(before).replace('+', "%2B");
    let earlier = page(&app, &format!("/v1/workloads?valid_at={at}")).await;
    assert_eq!(names(&earlier), ["web"]);
    let asserted = page(&app, &format!("/v1/workloads?as_of={at}")).await;
    assert_eq!(names(&asserted), ["web"]);
    let services = page(&app, &format!("/v1/kube-services?as_of={at}")).await;
    assert!(names(&services).is_empty());
    assert_eq!(
        status(&app, &format!("/v1/workloads/{}?as_of={at}", env.web)).await,
        StatusCode::OK
    );

    // The reads of the workloads journaled: the list with its ids and its question, the
    // object with its identifier.
    let reads: Vec<_> = audited(&pool, "data.objects.read")
        .await
        .into_iter()
        .filter(|(details, ..)| {
            details["criteria"]["collection"] == "workloads"
                || details["criteria"]["id"] == json!(env.web)
        })
        .collect();
    assert_eq!(reads.len(), 4, "{reads:?}");
    let (details, actor, targets, outcome) = &reads[0];
    assert_eq!(details["criteria"]["filters"], json!([]));
    assert_eq!((*actor, outcome.as_str()), (Some(admin), "success"));
    assert_eq!(targets.len(), 2);
    assert!(targets.contains(&env.web), "{targets:?}");
    // Each list journals the dates it was read at, to the microsecond.
    let stored = kartenn_kube::observations::rfc3339(before);
    let (details, _, targets, _) = &reads[1];
    assert_eq!(targets, &[env.web]);
    assert_eq!(details["criteria"]["valid_at"], stored);
    let (details, _, targets, _) = &reads[2];
    assert_eq!(targets, &[env.web]);
    assert_eq!(details["criteria"]["as_of"], stored);
    let (details, _, targets, _) = &reads[3];
    assert_eq!(targets, &[env.web]);
    assert_eq!(details["criteria"], json!({"id": env.web}));
    let kinds: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT target_kind FROM audit.event
          WHERE name = 'data.objects.read' AND target_ids @> ARRAY[$1::uuid]",
    )
    .bind(env.web)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(kinds, ["kube.workload"]);
}

/// REQ-019-39, ADR 0021: an object is read under `kube.<type>.read` on the domain it is
/// anchored in (a workload in the pod domain, a service in the service domain); one the
/// caller may not read is out of the lists and answers 404, as an unknown one.
#[kartenn_testing::pg_test]
async fn mirror_unreadable_is_absent(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.furnished().await;
    let frontend: Uuid = sqlx::query_scalar("SELECT id FROM kube.service WHERE name = 'frontend'")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Reading the clusters only: no workload.
    let clusters = person(&pool, &["kube.cluster.read"]).await;
    let theirs = app(&pool, principal(clusters, IdentityKind::Person), None);
    assert!(names(&page(&theirs, "/v1/workloads").await).is_empty());
    assert_eq!(
        status(&theirs, &format!("/v1/workloads/{}", env.web)).await,
        StatusCode::NOT_FOUND
    );
    // Reading the workloads and services on the pod domain: its workloads, not the
    // services of the service domain.
    let pods = person_on(
        &pool,
        &["kube.workload.read", "kube.service.read"],
        ("domain", Some(env.pods)),
    )
    .await;
    let theirs = app(&pool, principal(pods, IdentityKind::Person), None);
    assert_eq!(names(&page(&theirs, "/v1/workloads").await), ["db", "web"]);
    assert_eq!(
        status(&theirs, &format!("/v1/workloads/{}", env.web)).await,
        StatusCode::OK
    );
    assert!(names(&page(&theirs, "/v1/kube-services").await).is_empty());
    assert_eq!(
        status(&theirs, &format!("/v1/kube-services/{frontend}")).await,
        StatusCode::NOT_FOUND
    );
    // On the service domain, the service.
    let services = person_on(
        &pool,
        &["kube.service.read"],
        ("domain", Some(env.services)),
    )
    .await;
    let theirs = app(&pool, principal(services, IdentityKind::Person), None);
    assert_eq!(
        names(&page(&theirs, "/v1/kube-services").await),
        ["frontend"]
    );
}

/// REQ-019-39 (acceptation): the delivered role `reader` reads the workloads of its
/// scope, not the journal of observation (`kube.observation.read`, in no delivered role).
#[kartenn_testing::pg_test]
async fn reader_reads_workloads_not_observations(pool: PgPool) {
    let env = Env::new(&pool).await;
    sync_roles(&env.store, MAX_OPERATIONS).await.unwrap();
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'r') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let role: Uuid =
        sqlx::query_scalar("SELECT id FROM role WHERE key = 'reader' AND valid_period @> now()")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(kartenn_testing::BIND_IDENTITY)
        .bind(identity)
        .bind(role)
        .bind("global")
        .bind(None::<Uuid>)
        .execute(&pool)
        .await
        .unwrap();
    let theirs = app(&pool, principal(identity, IdentityKind::Person), None);
    assert_eq!(names(&page(&theirs, "/v1/workloads").await), ["web"]);
    assert_eq!(
        status(&theirs, &format!("/v1/workloads/{}", env.web)).await,
        StatusCode::OK
    );
    let (code, _, problem) = get(&theirs, &format!("/v1/workloads/{}/observations", env.web)).await;
    assert_eq!(code, StatusCode::FORBIDDEN, "{problem}");
}

/// The status, the headers but the identifier of the request, and the bytes of the body
/// of `GET uri`.
async fn answer(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let request = Request::builder()
        .uri(uri)
        .header(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test"),
        )
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let mut headers = response.headers().clone();
    assert!(headers.remove("request-id").is_some());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

/// `QUERY /v1/{collection}` filtered by the identifiers `ids`.
async fn query_ids(app: &Router, collection: &str, ids: &[Uuid]) -> (StatusCode, Value) {
    let ids: Vec<String> = ids.iter().map(Uuid::to_string).collect();
    let body = json!({"filters": [{"field": "id", "value": ids}]});
    let request = Request::builder()
        .method("QUERY")
        .uri(format!("/v1/{collection}"))
        .header(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test"),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The identifiers of a page, in its order.
fn ids_of(page: &Value) -> Vec<Uuid> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().parse().unwrap())
        .collect()
}

/// The filter `id` of the clusters and of the mirrored objects (REQ-006-41, task
/// 006-32c): the objects among the identifiers the caller reads, in conjunction with the
/// other filters, in one page without a next one; an unreadable one is absent as an unknown
/// one, headers and bytes included; refused beside `page_size` or a cursor.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // six collections, their conjunctions, page and refusals
async fn clusters_and_mirrors_filtered_by_identifier(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.furnished().await;
    // A cluster `stage` of the entity `beta`, populated as `prod` is.
    env.applied(
        env.admin,
        vec![kartenn_service::Operation::CreateEntity {
            local: Some(1),
            slug: "beta".into(),
            parent: None,
        }],
    )
    .await;
    let beta = env.id("entity", "slug", "beta").await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    let prefix = |domain: Uuid| {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()",
        )
        .bind(domain)
        .fetch_one(&pool)
    };
    let (container, network) = (
        prefix(env.pods).await.unwrap(),
        prefix(env.services).await.unwrap(),
    );
    env.applied(
        env.admin,
        vec![op(
            "create_cluster",
            json!({
                "slug": "stage",
                "owner": {"id": beta},
                "pod_domain": {"id": env.pods},
                "service_domain": {"id": env.services},
                "pod_containers": [{"id": container}],
                "service_networks": [{"id": network}],
                "connector_group": {"id": group},
            }),
        )],
    )
    .await;
    let stage = env.id("kube.cluster", "slug", "stage").await;
    let (stage_node, stage_web) = env.populated(stage, other, "198.51.100.64/26").await;
    let one = |sql: &'static str| sqlx::query_scalar::<_, Uuid>(sql).fetch_one(&pool);
    let db = one("SELECT id FROM kube.workload WHERE name = 'db'")
        .await
        .unwrap();
    let frontend = one("SELECT id FROM kube.service WHERE name = 'frontend'")
        .await
        .unwrap();
    let shop = one("SELECT id FROM kube.ingress WHERE name = 'shop'")
        .await
        .unwrap();
    let payments = namespace(&pool, env.prod, "payments").await;
    let stage_payments = namespace(&pool, stage, "payments").await;
    let everything = [
        "kube.cluster.read",
        "kube.node.read",
        "kube.namespace.read",
        "kube.workload.read",
        "kube.service.read",
        "kube.ingress.read",
    ];
    // What `acme` owns: `prod` and its objects, not `stage`.
    let reader = person_on(&pool, &everything, ("entity", Some(env.acme))).await;
    let reader = app(&pool, principal(reader, IdentityKind::Person), None);
    let admin = person(&pool, &["*"]).await;
    let admin = app(&pool, principal(admin, IdentityKind::Person), None);
    let unknown = Uuid::now_v7();
    let shop_namespace = namespace(&pool, env.prod, "shop").await;
    for (app, uri, expected) in [
        (
            &reader,
            format!("/v1/kube-clusters?id={stage}&id={}", env.prod),
            vec![env.prod],
        ),
        (
            &admin,
            format!("/v1/kube-clusters?id={stage}&id={}&id={stage}", env.prod),
            vec![env.prod, stage],
        ),
        (
            &admin,
            format!("/v1/kube-clusters?id={stage}&id={}&slug=stage", env.prod),
            vec![stage],
        ),
        (
            &admin,
            format!("/v1/kube-nodes?id={stage_node}&id={}", env.node),
            vec![env.node, stage_node],
        ),
        (
            &admin,
            format!(
                "/v1/kube-nodes?id={stage_node}&id={}&cluster={stage}",
                env.node
            ),
            vec![stage_node],
        ),
        (
            &reader,
            format!("/v1/kube-nodes?id={stage_node}&id={}", env.node),
            vec![env.node],
        ),
        (
            &reader,
            format!("/v1/workloads?id={stage_web}&id={}&id={db}", env.web),
            vec![env.web, db],
        ),
        (
            &admin,
            format!(
                "/v1/workloads?id={stage_web}&id={}&id={db}&kind=deployment",
                env.web
            ),
            vec![env.web, stage_web],
        ),
        (
            &admin,
            format!(
                "/v1/kube-namespaces?id={stage_payments}&id={payments}&id={shop_namespace}\
                 &label=app=shop"
            ),
            vec![shop_namespace],
        ),
        (
            &admin,
            format!("/v1/kube-services?id={frontend}&type=node_port"),
            vec![frontend],
        ),
        (
            &admin,
            format!("/v1/ingresses?id={shop}&id={db}"),
            vec![shop],
        ),
    ] {
        let page = page(app, &uri).await;
        let (mut got, mut expected) = (ids_of(&page), expected);
        got.sort();
        expected.sort();
        assert_eq!(got, expected, "{uri}: {page}");
        assert!(page["links"]["next"].is_null(), "{uri}");
    }
    // The unreadable object leaves the answer of an unknown identifier, in one page.
    for (collection, unreadable, known) in [
        ("kube-clusters", stage, env.prod),
        ("kube-nodes", stage_node, env.node),
        ("workloads", stage_web, env.web),
    ] {
        let mut answers = Vec::new();
        for absent in [unknown, unreadable] {
            let uri = format!("/v1/{collection}?id={absent}&id={known}");
            answers.push(answer(&reader, &uri).await);
        }
        assert_eq!(
            answers[0], answers[1],
            "{collection}: unknown and unreadable"
        );
        assert_eq!(answers[0].0, StatusCode::OK, "{collection}");
        assert!(answers[0].1.get("link").is_none(), "{collection}");
    }
    // Past the default size of a page, one page without a next one.
    let many: Vec<_> = (0..101)
        .map(|n| sync("namespace", env.prod, &format!("ns{n:03}"), json!({})))
        .collect();
    env.applied(env.connector, many).await;
    let wanted: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM kube.namespace
          WHERE name LIKE 'ns%' AND valid_period @> now() ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(wanted.len(), 101);
    let (status, body) = query_ids(&reader, "kube-namespaces", &wanted).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids_of(&body), wanted);
    assert!(body["links"]["next"].is_null(), "{}", body["links"]);
    // `page_size` or a cursor beside `id`, or a value that is not an identifier → 400
    // naming it.
    let first = page(&admin, "/v1/kube-namespaces?page_size=1").await;
    let next = first["links"]["next"].as_str().unwrap().to_owned();
    let clusters = page(&admin, "/v1/kube-clusters?page_size=1").await;
    let clusters_next = clusters["links"]["next"].as_str().unwrap().to_owned();
    for (uri, named) in [
        (
            format!("/v1/kube-clusters?id={stage}&page_size=5"),
            "page_size",
        ),
        (
            format!("/v1/kube-namespaces?id={payments}&page_size=5"),
            "page_size",
        ),
        (format!("{next}&id={payments}"), "cursor"),
        (format!("{clusters_next}&id={stage}"), "cursor"),
        ("/v1/kube-clusters?id=prod".to_owned(), "id"),
        ("/v1/workloads?id=web".to_owned(), "id"),
    ] {
        let (status, _, problem) = get(&admin, &uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {problem}");
        assert!(
            problem["detail"].to_string().contains(named),
            "{uri}: {problem}"
        );
    }
}
