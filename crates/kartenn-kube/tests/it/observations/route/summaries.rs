//! The references of the bodies of the module rendered as summaries (spec 006,
//! REQ-006-42, task 33e): a cluster, a mirrored object, an observation of the journal and
//! a link of a chain of execution summarise each object they name as their caller may
//! read it now, its key read at the dates of the read (the module's for its own objects);
//! an object the caller may not read and one absent at those dates leave the same `{id}`,
//! byte for byte, status and headers but the identifier of the request alike.
//!
//! A mirrored object names its cluster, its namespace, its pod networks and its addresses,
//! which the module closes with it or refuses to close before it: none of them can be
//! absent while it is present. The absent case of their summaries is the one of the
//! cluster, of the journal and of the chain, by the same path.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_core::objects::Nature;
use kartenn_kube::observations::Batch;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::PeriodSpec;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Outcome, submit};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use tower::ServiceExt;

use super::super::{Env, hours, op};
use super::read::{as_person, clock, get, reader};
use super::{rfc3339, slugged, summary};

/// The status, the headers but the identifier of the request, and the bytes of the body
/// of `GET uri` by `app`.
async fn answer(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let request = Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, "Bearer test")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let mut headers = response.headers().clone();
    assert!(headers.remove("request-id").is_some());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

/// `permissions` granted to `identity` on the domain `domain`, by a role of its own.
async fn grant_on(pool: &PgPool, identity: Uuid, permissions: &[&str], domain: Uuid) {
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, 'r_' || replace($1::text, '-', ''), jsonb_build_object('fr', 'r'),
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    for permission in permissions {
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(permission)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(kartenn_testing::BIND_IDENTITY)
        .bind(identity)
        .bind(role)
        .bind("domain")
        .bind(Some(domain))
        .execute(pool)
        .await
        .unwrap();
}

/// The living object of `table` named `name`.
async fn named(pool: &PgPool, table: &str, name: &str) -> Uuid {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT id FROM {table} WHERE name = $1 AND valid_period @> now()"
    )))
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The living prefix of `domain`.
async fn prefix_of(pool: &PgPool, domain: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()")
        .bind(domain)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// REQ-006-42: a workload and a node name their cluster, their namespace and their pod
/// networks by their summaries, the key of an object of the module given by the module (a
/// name, without its cluster nor its namespace); `{id}` alone for one the caller may not
/// read, in a list as in a read.
#[kartenn_testing::pg_test]
async fn a_mirror_summarises_what_its_caller_may_read(pool: PgPool) {
    let env = Env::new(&pool).await;
    let payments = named(&pool, "kube.namespace", "payments").await;
    let network: Uuid = sqlx::query_scalar(
        "SELECT unnest(range_ids) FROM kube.node WHERE id = $1 AND valid_period @> now()",
    )
    .bind(env.node)
    .fetch_one(&pool)
    .await
    .unwrap();
    let all = as_person(
        &pool,
        reader(
            &pool,
            &[
                "kube.workload.read",
                "kube.node.read",
                "kube.namespace.read",
                "kube.cluster.read",
                "core.prefix.read",
            ],
            None,
        )
        .await,
    );
    let mirrors = as_person(
        &pool,
        reader(&pool, &["kube.workload.read", "kube.node.read"], None).await,
    );
    let workload = format!("/v1/workloads/{}", env.web);
    let node = format!("/v1/kube-nodes/{}", env.node);
    let list = format!("/v1/workloads?id={}", env.web);

    let (status, body) = get(&all, &workload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["cluster"], summary("kube-clusters", env.prod, "prod"));
    assert_eq!(
        body["namespace"],
        summary("kube-namespaces", payments, "payments")
    );
    let (_, page) = get(&all, &list).await;
    assert_eq!(page["items"][0]["namespace"], body["namespace"]);
    let (status, body) = get(&all, &node).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["cluster"], summary("kube-clusters", env.prod, "prod"));
    assert_eq!(
        body["ranges"],
        json!([summary("prefixes", network, "198.51.100.0/26")])
    );

    let (_, body) = get(&mirrors, &workload).await;
    assert_eq!(body["cluster"], json!({"id": env.prod}));
    assert_eq!(body["namespace"], json!({"id": payments}));
    let (_, page) = get(&mirrors, &list).await;
    assert_eq!(page["items"][0]["namespace"], json!({"id": payments}));
    let (_, body) = get(&mirrors, &node).await;
    assert_eq!(body["ranges"], json!([{"id": network}]));

    // A service names its namespace and its cluster; a namespace its cluster.
    env.applied(
        env.connector,
        vec![super::super::sync(
            "service",
            env.prod,
            "frontend",
            json!({"namespace": "payments", "service_type": "node_port",
                   "ports": [{"port": 80, "target_port": "http", "node_port": 30080}]}),
        )],
    )
    .await;
    let frontend = named(&pool, "kube.service", "frontend").await;
    let service = format!("/v1/kube-services/{frontend}");
    let namespace = format!("/v1/kube-namespaces/{payments}");
    let services = as_person(
        &pool,
        reader(
            &pool,
            &[
                "kube.service.read",
                "kube.namespace.read",
                "kube.cluster.read",
            ],
            None,
        )
        .await,
    );
    let (status, body) = get(&services, &service).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["namespace"],
        summary("kube-namespaces", payments, "payments")
    );
    assert_eq!(body["cluster"], summary("kube-clusters", env.prod, "prod"));
    let (_, body) = get(&services, &namespace).await;
    assert_eq!(body["cluster"], summary("kube-clusters", env.prod, "prod"));
    let alone = as_person(
        &pool,
        reader(&pool, &["kube.service.read", "kube.namespace.read"], None).await,
    );
    let (_, body) = get(&alone, &service).await;
    assert_eq!(
        body["namespace"],
        summary("kube-namespaces", payments, "payments")
    );
    assert_eq!(body["cluster"], json!({"id": env.prod}));
    let (_, body) = get(&alone, &namespace).await;
    assert_eq!(body["cluster"], json!({"id": env.prod}));
    let (_, body) = get(&mirrors, &service).await;
    assert!(body.get("namespace").is_none(), "{body}");
}

/// REQ-006-42 by a route of the core: the object of an anomaly, a setting of the module,
/// carries the key the module gives it (the label key of a tracked label), `{id}` alone
/// for who may not read it.
#[kartenn_testing::pg_test]
async fn an_anomaly_of_a_setting_carries_the_key_of_the_module(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.applied(
        env.admin,
        vec![op(
            "set_tracked_labels",
            json!({"keys": ["app.example.org/tier"]}),
        )],
    )
    .await;
    let tracked: Uuid = sqlx::query_scalar(
        "SELECT id FROM kube.tracked_label
          WHERE label_key = 'app.example.org/tier' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // An anomaly opened on it, as a rule of the module would.
    sqlx::query(
        "INSERT INTO anomaly (rule_id, object_id, message, opened_by)
         SELECT 'kube.operation.invalid', $1, 'test', id FROM changeset LIMIT 1",
    )
    .bind(tracked)
    .execute(&pool)
    .await
    .unwrap();
    let admin = as_person(&pool, super::person(&pool, &["*"]).await);
    let (status, page) = get(&admin, "/v1/anomalies").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let item = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["object"]["id"] == json!(tracked))
        .unwrap_or_else(|| panic!("{page}"));
    assert_eq!(
        item["object"],
        json!({"kind": "kube.tracked_label", "id": tracked, "label": "app.example.org/tier",
               "href": format!("/v1/kube-tracked-labels/{tracked}")})
    );
}

/// REQ-006-42: a cluster names its owner, its domains and its pod containers by their
/// summaries, `{id}` alone for a domain the caller may not read; a cluster of machines
/// the caller may not read and one absent at the date of the read leave the same bytes,
/// and the same answer. (The module keeps every domain of a cluster living over its
/// period, not the cluster of machines it shows.)
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // a cluster of machines for an hour, three callers, two dates
async fn a_cluster_leaves_alike_an_unreadable_reference_and_an_absent_one(pool: PgPool) {
    let env = Env::new(&pool).await;
    let edge = env.domain("edge", "192.0.2.0/24", Nature::Network).await;
    // The cluster of machines `vc`, for an hour from now.
    let start: i64 =
        sqlx::query_scalar("SELECT (extract(epoch FROM clock_timestamp()) * 1000000)::bigint")
            .fetch_one(&pool)
            .await
            .unwrap();
    let request = ChangesetRequest {
        author: env.admin,
        motive: "test".into(),
        period: Some(PeriodSpec {
            start,
            end: Some(start + 3_600_000_000),
        }),
        operations: vec![
            serde_json::from_value(json!({"op": "machines.create_cluster", "slug": "vc",
                                          "owner": {"id": env.acme}}))
            .unwrap(),
        ],
        simulate: false,
        idempotency_key: None,
        approval_self: true,
    };
    let outcome = submit(&env.store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    let vc = env.id("machines.cluster", "slug", "vc").await;
    let (container, network) = (
        prefix_of(&pool, env.pods).await,
        prefix_of(&pool, env.services).await,
    );
    let (_, group) = crate::support::connector(&pool, "edge").await;
    env.applied(
        env.admin,
        vec![op(
            "create_cluster",
            json!({
                "slug": "edge-1",
                "owner": {"id": env.acme},
                "pod_domain": {"id": env.pods},
                "service_domain": {"id": env.services},
                "external_domain": {"id": edge},
                "pod_containers": [{"id": container}],
                "service_networks": [{"id": network}],
                "machines_cluster": {"id": vc},
                "connector_group": {"id": group},
            }),
        )],
    )
    .await;
    let cluster = env.id("kube.cluster", "slug", "edge-1").await;
    let reads = [
        "kube.cluster.read",
        "core.entity.read",
        "core.prefix.read",
        "core.domain.read",
    ];
    // Who reads the clusters of machines, who does not, and who reads the domains of the
    // pods and of the services only.
    let everywhere = reader(
        &pool,
        &[reads.as_slice(), &["machines.cluster.read"]].concat(),
        None,
    )
    .await;
    let everywhere = as_person(&pool, everywhere);
    let kube = as_person(&pool, reader(&pool, &reads, None).await);
    let two = reader(&pool, &reads[..3], None).await;
    for domain in [env.pods, env.services] {
        grant_on(&pool, two, &["core.domain.read"], domain).await;
    }
    let two = as_person(&pool, two);
    let uri = format!("/v1/kube-clusters/{cluster}");

    // Now: `edge` is summarised for who reads it, `{id}` for the other one.
    let (_, body) = get(&two, &uri).await;
    assert_eq!(body["pod_domain"], slugged("domains", env.pods, "pods"));
    assert_eq!(body["external_domain"], json!({"id": edge}));
    let (status, body) = get(&kube, &uri).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["owner"], slugged("entities", env.acme, "acme"));
    assert_eq!(body["pod_domain"], slugged("domains", env.pods, "pods"));
    assert_eq!(body["external_domain"], slugged("domains", edge, "edge"));
    assert_eq!(
        body["pod_containers"],
        json!([summary("prefixes", container, "198.51.100.0/24")])
    );
    // A group of the rights is no object of the referential: its identifier alone.
    assert_eq!(body["connector_group"], json!({"id": group}));
    // The cluster of machines, which this caller may not read.
    assert_eq!(body["machines_cluster"], json!({"id": vc}));
    let unreadable = answer(&kube, &uri).await;

    // Past its hour, `vc` is absent: the same `{id}` for both, the same answer, and the
    // same bytes as the unreadable one.
    let later = clock(&pool).await + hours(2);
    let uri = format!("{uri}?valid_at={}", rfc3339(later));
    let absent = answer(&everywhere, &uri).await;
    assert_eq!(absent.0, StatusCode::OK);
    let body: Value = serde_json::from_slice(&absent.2).unwrap();
    assert_eq!(body["machines_cluster"], json!({"id": vc}));
    assert_eq!(absent, answer(&kube, &uri).await);
    assert_eq!(absent.2, unreadable.2);
}

/// REQ-006-42, REQ-019-16: an observation names its workload and its node as they were at
/// the instant asked, as its caller may read them now: a workload closed since, read then,
/// keeps its summary for who reads it and leaves `{id}` to who does not; one absent at the
/// instant asked leaves the same `{id}`, and the same answer, `ETag` included, to both.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // two pods, two callers, two instants
async fn a_journal_names_its_objects_as_they_were_at_the_instant_asked(pool: PgPool) {
    let env = Env::new(&pool).await;
    // `web-1` seen an hour ago, before `web` and `worker-1` were declared; `web-2` seen
    // once they were.
    let seen = clock(&pool).await;
    let mut second = env.pod(2, "198.51.100.18");
    second.from = seen;
    env.recorded(&Batch {
        opened: vec![env.pod(1, "198.51.100.17"), second],
        closed: Vec::new(),
    })
    .await;
    // `web` closed by the connector since: the journal still names it.
    env.applied(
        env.connector,
        vec![op(
            "sync",
            json!({"action": "close", "type": "kube.workload", "cluster": {"id": env.prod},
                   "name": "web", "namespace": "payments", "kind": "deployment"}),
        )],
    )
    .await;
    let journal = ["kube.observation.read", "kube.node.read"];
    let workloads = reader(
        &pool,
        &[journal.as_slice(), &["kube.workload.read"]].concat(),
        None,
    )
    .await;
    let (workloads, nodes) = (
        as_person(&pool, workloads),
        as_person(&pool, reader(&pool, &journal, None).await),
    );
    let held = |ip: &str, at: String| {
        format!(
            "/v1/kube-clusters/{}/observations?ip={ip}&at={at}",
            env.prod
        )
    };

    // At the instant `web-2` was seen, `web` lived: its summary for who reads it, though
    // closed now; `{id}` for who does not.
    let then = held("198.51.100.18", rfc3339(seen));
    let (status, page) = get(&workloads, &then).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let item = &page["items"][0];
    assert_eq!(item["workload"], summary("workloads", env.web, "web"));
    assert_eq!(item["node"], summary("kube-nodes", env.node, "worker-1"));
    let (_, page) = get(&nodes, &then).await;
    let unreadable = page["items"][0]["workload"].clone();
    assert_eq!(unreadable, json!({"id": env.web}));
    assert_eq!(
        page["items"][0]["node"],
        summary("kube-nodes", env.node, "worker-1")
    );

    // An hour ago, neither `web` nor `worker-1` was declared: `{id}` for both callers, the
    // same bytes as the unreadable one, the same answer.
    let before = held(
        "198.51.100.17",
        rfc3339(seen - hours(1) + std::time::Duration::from_secs(60)),
    );
    let absent = answer(&workloads, &before).await;
    assert_eq!(absent.0, StatusCode::OK);
    let page: Value = serde_json::from_slice(&absent.2).unwrap();
    assert_eq!(
        serde_json::to_vec(&page["items"][0]["workload"]).unwrap(),
        serde_json::to_vec(&unreadable).unwrap()
    );
    assert_eq!(page["items"][0]["node"], json!({"id": env.node}));
    assert_eq!(absent, answer(&nodes, &before).await);
}

/// REQ-006-42: a link of a chain of execution names its node by its summary; a node the
/// caller may not read and one closed since leave the same bytes, and the same answer.
#[kartenn_testing::pg_test]
async fn a_chain_leaves_alike_an_unreadable_node_and_a_gone_one(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.recorded(&Batch {
        opened: vec![env.pod(1, "198.51.100.17")],
        closed: Vec::new(),
    })
    .await;
    let journal = "kube.observation.read";
    let nodes = as_person(
        &pool,
        reader(&pool, &[journal, "kube.node.read"], None).await,
    );
    let pods = as_person(&pool, reader(&pool, &[journal], None).await);
    let uri = format!("/v1/workloads/{}/chain", env.web);

    let (status, body) = get(&nodes, &uri).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["links"][0]["node"],
        summary("kube-nodes", env.node, "worker-1")
    );
    let (_, body) = get(&pods, &uri).await;
    assert_eq!(body["links"][0]["node"], json!({"id": env.node}));
    let unreadable = answer(&pods, &uri).await;

    // The node closed by the connector: the journal still names it, as absent.
    env.applied(
        env.connector,
        vec![op(
            "sync",
            json!({"action": "close", "type": "kube.node", "cluster": {"id": env.prod},
                   "name": "worker-1"}),
        )],
    )
    .await;
    let gone = answer(&nodes, &uri).await;
    assert_eq!(gone.0, StatusCode::OK);
    assert_eq!(gone, unreadable);
    assert_eq!(gone, answer(&pods, &uri).await);
}
