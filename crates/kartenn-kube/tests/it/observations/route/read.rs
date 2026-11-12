//! The reads of the journal (task 019-6d; REQ-019-16, 17; ADR 0043, decisions 6, 8;
//! Q-019-23, 25): `who_had` of the read port, `GET /v1/kube-clusters/{id}/observations?ip=&at=`
//! and `GET /v1/workloads/{id}/observations` under `kube.observation.read` on the pod
//! domain of the cluster, paged, journaled, empty past the retention with the reason
//! `retention`.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::objects::Nature;
use kartenn_kube::observations::{Batch, Opening};
use kartenn_kube::read::{Cut, who_had};
use kartenn_service::authn::IdentityKind;
use kartenn_service::authz::{Authorizer, Subject, Unrestricted};
use kartenn_service::{ObjectKind, Operation, Ref, StoreError};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use tower::ServiceExt;

use super::super::{Env, closing, hours, op, sync};
use super::{app, audited, principal};

/// An authorizer that grants nothing.
struct Nothing;

impl Authorizer for Nothing {
    fn authorize_many(
        &self,
        _: &Subject,
        _: &str,
        objects: &[Uuid],
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send {
        std::future::ready(Ok(objects.to_vec()))
    }
}

/// A person granted `permissions` on the domain `domain`, everywhere without one.
async fn reader(pool: &PgPool, permissions: &[&str], domain: Option<Uuid>) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'r') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
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
        .bind(if domain.is_some() { "domain" } else { "global" })
        .bind(domain)
        .execute(pool)
        .await
        .unwrap();
    identity
}

/// The router as the person `who`.
fn as_person(pool: &PgPool, who: Uuid) -> Router {
    app(pool, principal(who, IdentityKind::Person), None)
}

/// `GET uri` by `app`: its status and body.
async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, "Bearer test")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The question « who had `ip` at `at` » on the journal of `cluster`.
fn held(cluster: Uuid, ip: &str, at: OffsetDateTime) -> String {
    let at = super::rfc3339(at);
    format!("/v1/kube-clusters/{cluster}/observations?ip={ip}&at={at}")
}

/// The pod UIDs of the items of a page, in order.
fn pods(page: &Value) -> Vec<Uuid> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["pod_uid"].as_str().unwrap().parse().unwrap())
        .collect()
}

fn prefix(text: &str) -> Prefix {
    if text.contains('/') {
        Prefix::parse_any(&ALL, text).unwrap()
    } else {
        Prefix::host(Addr::parse_any(&ALL, text).unwrap())
    }
}

/// The pod `pod` of `web` in `hostNetwork` on `worker-1`, seen an hour ago.
fn host_pod(env: &Env, pod: u128) -> Opening {
    Opening {
        ip: None,
        host_network: true,
        ..env.pod(pod, "198.51.100.1")
    }
}

/// REQ-019-16, 17 (« Qui avait `10.64.3.17` le 2026-11-02 à 10 h ? »): the pod, its
/// workload and its node, by an address or a network, a pod in `hostNetwork` by an address
/// of its node; paged by start, pod and IP; every read journaled with the pods it
/// returned; the workload answers its pods, nodes and addresses; the port answers the same.
#[kartenn_testing::pg_test]
async fn who_had_ip_at_time(pool: PgPool) {
    let env = Env::new(&pool).await;
    let now = OffsetDateTime::now_utc();
    let mut first = env.pod(1, "198.51.100.17");
    first.from = now - hours(3);
    let mut second = env.pod(2, "198.51.100.18");
    second.from = now - hours(2);
    env.recorded(&Batch {
        opened: vec![first, second, host_pod(&env, 3)],
        closed: Vec::new(),
    })
    .await;
    env.recorded(&Batch {
        opened: Vec::new(),
        closed: vec![closing(1, "198.51.100.17", now - hours(1))],
    })
    .await;
    let who = reader(&pool, &["kube.observation.read"], Some(env.pods)).await;
    let app = as_person(&pool, who);

    // An address, at an instant it was held: the pod, its workload, its node.
    let (status, page) = get(&app, &held(env.prod, "198.51.100.17", now - hours(2))).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["reason"], Value::Null);
    assert_eq!(page["links"]["next"], Value::Null);
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{page}");
    let item = &items[0];
    assert_eq!(item["pod_uid"], json!(Uuid::from_u128(1)));
    assert_eq!(item["pod_name"], "web-1");
    assert_eq!(item["namespace"], "payments");
    assert_eq!(item["workload"], json!({"id": env.web}));
    assert_eq!(item["node"], json!({"id": env.node}));
    assert_eq!(item["ip"], "198.51.100.17");
    assert_eq!(item["host_network"], false);
    assert!(item["until"].is_string(), "{item}");
    assert!(item["closed_batch"].is_string(), "{item}");
    // Once it was gone, nobody; a network, both pods it held then, by start.
    let (_, page) = get(&app, &held(env.prod, "198.51.100.17", now)).await;
    assert_eq!(pods(&page), Vec::<Uuid>::new());
    let network = held(env.prod, "198.51.100.0/26", now - hours(1) - hours(1) / 2);
    let (_, page) = get(&app, &network).await;
    assert_eq!(pods(&page), [Uuid::from_u128(1), Uuid::from_u128(2)]);
    // A page of one, then the next one from its link (the cursor keeps the question).
    let (status, one) = get(&app, &format!("{network}&page_size=1")).await;
    assert_eq!(status, StatusCode::OK, "{one}");
    assert_eq!(pods(&one), [Uuid::from_u128(1)]);
    let next = one["links"]["next"].as_str().unwrap().to_owned();
    let (status, two) = get(&app, &next).await;
    assert_eq!(status, StatusCode::OK, "{two}");
    assert_eq!(pods(&two), [Uuid::from_u128(2)]);
    assert_eq!(two["links"]["next"], Value::Null);
    // The address of the node, now: the pod in `hostNetwork` on it.
    let (_, page) = get(&app, &held(env.prod, "192.0.2.21", now)).await;
    assert_eq!(pods(&page), [Uuid::from_u128(3)]);
    assert_eq!(page["items"][0]["ip"], Value::Null);
    assert_eq!(page["items"][0]["host_network"], true);

    // Malformed questions: 400.
    for uri in [
        held(env.prod, "198.51.100.1/24", now),
        held(env.prod, "web-1", now),
        format!(
            "/v1/kube-clusters/{}/observations?ip=198.51.100.17",
            env.prod
        ),
        format!(
            "/v1/kube-clusters/{}/observations?at=2026-11-02T10:00:00Z",
            env.prod
        ),
        format!(
            "{}&valid_at=2026-11-02T10:00:00Z",
            held(env.prod, "198.51.100.17", now)
        ),
        format!("{}&pod=web-1", held(env.prod, "198.51.100.17", now)),
        format!(
            "/v1/workloads/{}/observations?as_of=2026-11-02T10:00:00Z",
            env.web
        ),
    ] {
        let (status, problem) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {problem}");
    }

    // The workload: its three pods, their nodes and addresses, by start.
    let (status, page) = get(&app, &format!("/v1/workloads/{}/observations", env.web)).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(
        pods(&page),
        [Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)]
    );
    assert_eq!(page["reason"], Value::Null);

    // Each read answered is journaled, with the pods it returned and the question.
    let reads = audited(&pool, "data.objects.read").await;
    assert_eq!(reads.len(), 7, "{reads:?}");
    let (details, actor, targets, outcome) = &reads[0];
    assert_eq!((*actor, outcome.as_str()), (Some(who), "success"));
    assert_eq!(targets, &[Uuid::from_u128(1)]);
    assert_eq!(
        details["criteria"]["collection"],
        format!("kube-clusters/{}/observations", env.prod)
    );
    assert_eq!(
        details["criteria"]["filters"][1],
        json!(["ip", "198.51.100.17"])
    );
    let kinds: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT target_kind FROM audit.event WHERE name = 'data.objects.read'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(kinds, ["kube.pod_observation"]);

    // The port: the same pods, for a reader who holds the right on the pod domain.
    let mut conn = pool.acquire().await.unwrap();
    let subject = principal(who, IdentityKind::Person).subject();
    let answer = who_had(
        &mut conn,
        &Unrestricted,
        &subject,
        prefix("198.51.100.0/26"),
        env.pods,
        now - hours(1) - hours(1) / 2,
    )
    .await
    .unwrap();
    let found: Vec<(Uuid, Uuid, Option<Uuid>, Option<Uuid>)> = answer
        .observations
        .iter()
        .map(|o| (o.pod_uid, o.cluster, o.workload, o.node))
        .collect();
    assert_eq!(
        found,
        [
            (Uuid::from_u128(1), env.prod, Some(env.web), Some(env.node)),
            (Uuid::from_u128(2), env.prod, Some(env.web), Some(env.node)),
        ]
    );
    assert_eq!(answer.reason, None);
}

/// REQ-019-16 (rétention; ADR 0043, decision 8; Q-019-25): an instant older than the
/// retention of the cluster answers 200, no item and the reason `retention`, though its
/// rows are still stored (not purged yet), and is journaled as any read; a pod alive for
/// longer than the retention is still answered within it; the workload answers only the
/// observations within the retention; the port says `retention` too.
#[kartenn_testing::pg_test]
async fn retention_answers_empty_with_reason(pool: PgPool) {
    let env = Env::new(&pool).await;
    let now = OffsetDateTime::now_utc();
    let days = |n: u64| hours(24 * n);
    let mut old = env.pod(1, "198.51.100.17");
    old.from = now - days(40);
    let mut long = env.pod(2, "198.51.100.18");
    long.from = now - days(45);
    env.recorded(&Batch {
        opened: vec![old, long],
        closed: Vec::new(),
    })
    .await;
    env.recorded(&Batch {
        opened: Vec::new(),
        closed: vec![closing(1, "198.51.100.17", now - days(35))],
    })
    .await;
    let who = reader(&pool, &["kube.observation.read"], Some(env.pods)).await;
    let app = as_person(&pool, who);
    // Stored, and held at the instant asked, but past the 30 days of the cluster.
    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kube.pod_observation WHERE pod_uid = $1 AND observed @> $2",
    )
    .bind(Uuid::from_u128(1))
    .bind(now - days(36))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, 1);
    let (status, page) = get(&app, &held(env.prod, "198.51.100.17", now - days(36))).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["items"], json!([]));
    assert_eq!(page["reason"], "retention");
    assert_eq!(page["links"]["next"], Value::Null);
    // The pod alive for 45 days, asked about 10 days ago: within the retention.
    let (_, page) = get(&app, &held(env.prod, "198.51.100.18", now - days(10))).await;
    assert_eq!(pods(&page), [Uuid::from_u128(2)]);
    assert_eq!(page["reason"], Value::Null);
    // The workload: the open one only, the other closed 35 days ago.
    let (_, page) = get(&app, &format!("/v1/workloads/{}/observations", env.web)).await;
    assert_eq!(pods(&page), [Uuid::from_u128(2)]);
    // Every read journaled, the one past the retention without a target.
    let reads = audited(&pool, "data.objects.read").await;
    assert_eq!(reads.len(), 3, "{reads:?}");
    assert!(reads[0].2.is_empty(), "{reads:?}");
    // The port: nothing, and the reason.
    let mut conn = pool.acquire().await.unwrap();
    let subject = principal(who, IdentityKind::Person).subject();
    let answer = who_had(
        &mut conn,
        &Unrestricted,
        &subject,
        prefix("198.51.100.17"),
        env.pods,
        now - days(36),
    )
    .await
    .unwrap();
    assert!(answer.observations.is_empty());
    assert_eq!(answer.reason, Some(Cut::Retention));
}

/// REQ-019-16 (lecture bornée; ADR 0043, decision 6; Q-019-23): a reader who holds
/// `kube.observation.read` on the pod domain of the cluster A sees no observation of the
/// cluster B, whose pods live in another domain: B and its workloads answer 404, as
/// absent ones, unjournaled; who reads a cluster or a workload without the right on its
/// pod domain gets 403, journaled; the port answers nothing without the right, and only
/// the clusters of the domain asked.
#[kartenn_testing::pg_test]
async fn observations_read_bounded_to_cluster(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    // The cluster `stage`, its pods in another container.
    let pods_b = env
        .domain("pods-b", "192.0.2.128/25", Nature::Container)
        .await;
    let container: Uuid =
        sqlx::query_scalar("SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()")
            .bind(pods_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    let network: Uuid =
        sqlx::query_scalar("SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()")
            .bind(env.services)
            .fetch_one(&pool)
            .await
            .unwrap();
    env.applied(
        env.admin,
        vec![op(
            "create_cluster",
            json!({
                "slug": "stage",
                "owner": {"id": env.acme},
                "pod_domain": {"id": pods_b},
                "service_domain": {"id": env.services},
                "pod_containers": [{"id": container}],
                "service_networks": [{"id": network}],
                "connector_group": {"id": group},
            }),
        )],
    )
    .await;
    let stage = env.id("kube.cluster", "slug", "stage").await;
    let (stage_node, stage_web) = env.populated(stage, other, "192.0.2.128/26").await;
    let now = OffsetDateTime::now_utc();
    env.recorded(&Batch {
        opened: vec![env.pod(1, "198.51.100.17")],
        closed: Vec::new(),
    })
    .await;
    let theirs = Opening {
        workload: Some(stage_web),
        node: Some(stage_node),
        ..env.pod(2, "192.0.2.130")
    };
    env.record_by(
        stage,
        other,
        &Batch {
            opened: vec![theirs],
            closed: Vec::new(),
        },
    )
    .await
    .unwrap();

    // The reader of the journal of `prod` alone.
    let who = reader(&pool, &["kube.observation.read"], Some(env.pods)).await;
    let app = as_person(&pool, who);
    let (status, page) = get(&app, &held(env.prod, "198.51.100.17", now)).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(pods(&page), [Uuid::from_u128(1)]);
    for uri in [
        held(stage, "192.0.2.130", now),
        format!("/v1/workloads/{stage_web}/observations"),
        held(Uuid::now_v7(), "192.0.2.130", now),
        format!("/v1/workloads/{}/observations", Uuid::now_v7()),
    ] {
        let (status, problem) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {problem}");
    }
    assert!(audited(&pool, "authz.access.denied").await.is_empty());
    // Who reads the cluster or the workload of `stage`, without the right on its pods.
    let clusters = reader(&pool, &["kube.cluster.read", "kube.workload.read"], None).await;
    let theirs = as_person(&pool, clusters);
    let (status, problem) = get(&theirs, &held(stage, "192.0.2.130", now)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    let uri = format!("/v1/workloads/{stage_web}/observations");
    let (status, problem) = get(&theirs, &uri).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    let denials = audited(&pool, "authz.access.denied").await;
    let denied: Vec<(Value, Vec<Uuid>)> = denials
        .into_iter()
        .map(|(details, _, targets, _)| (details, targets))
        .collect();
    assert_eq!(
        denied,
        [
            (json!({"permission": "kube.observation.read"}), vec![stage]),
            (
                json!({"permission": "kube.observation.read"}),
                vec![stage_web]
            ),
        ]
    );
    // The connector of `stage`, which writes its journal, does not read it.
    let connector = app_as_account(&pool, other);
    let (status, _) = get(&connector, &held(stage, "192.0.2.130", now)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The port: only the clusters of the domain asked, and nothing without the right.
    let mut conn = pool.acquire().await.unwrap();
    let subject = principal(who, IdentityKind::Person).subject();
    let everything = prefix("0.0.0.0/0");
    let answer = who_had(
        &mut conn,
        &Unrestricted,
        &subject,
        everything,
        env.pods,
        now,
    )
    .await
    .unwrap();
    let clusters: Vec<Uuid> = answer.observations.iter().map(|o| o.cluster).collect();
    assert_eq!(clusters, [env.prod]);
    let answer = who_had(&mut conn, &Nothing, &subject, everything, env.pods, now)
        .await
        .unwrap();
    assert!(answer.observations.is_empty());
    assert_eq!(answer.reason, None);
}

/// The router as the service account `who`.
fn app_as_account(pool: &PgPool, who: Uuid) -> Router {
    app(pool, principal(who, IdentityKind::ServiceAccount), None)
}

/// The instant of the database, now.
async fn clock(pool: &PgPool) -> OffsetDateTime {
    sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Q-019-28 (objets clos : illisibles, mais dit): a closed workload answers 410 `gone`,
/// a closed cluster too, to whoever may read their journal; to a caller without any right
/// they stay 404, as absent ones; neither answer is journaled as a read; the port answers
/// nothing for the closed cluster with the reason `closed`, and nothing without a reason
/// for an instant after its closing.
#[kartenn_testing::pg_test]
async fn closed_objects_answer_gone(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.recorded(&Batch {
        opened: vec![env.pod(1, "198.51.100.17")],
        closed: Vec::new(),
    })
    .await;
    let lived = clock(&pool).await;
    let who = reader(&pool, &["kube.observation.read"], Some(env.pods)).await;
    let app = as_person(&pool, who);
    let nobody = as_person(&pool, reader(&pool, &[], None).await);
    let gone = |status: StatusCode, problem: &Value| {
        assert_eq!(status, StatusCode::GONE, "{problem}");
        assert_eq!(problem["type"], "urn:kartenn:problem:gone");
    };
    // The workload closed by the connector.
    env.applied(
        env.connector,
        vec![op(
            "sync",
            json!({"action": "close", "type": "kube.workload", "cluster": {"id": env.prod},
                   "name": "web", "namespace": "payments", "kind": "deployment"}),
        )],
    )
    .await;
    let workload = format!("/v1/workloads/{}/observations", env.web);
    let (status, problem) = get(&app, &workload).await;
    gone(status, &problem);
    let (status, problem) = get(&nobody, &workload).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{problem}");
    // The cluster still lives: read.
    let (status, page) = get(&app, &held(env.prod, "198.51.100.17", lived)).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(pods(&page), [Uuid::from_u128(1)]);
    // The cluster closed.
    env.applied(
        env.admin,
        vec![Operation::Close {
            kind: ObjectKind::from_name("kube.cluster").unwrap(),
            id: Ref::Id(env.prod),
            expected: None,
            cascade: true,
        }],
    )
    .await;
    let after = clock(&pool).await;
    let (status, problem) = get(&app, &held(env.prod, "198.51.100.17", lived)).await;
    gone(status, &problem);
    let (status, problem) = get(&nobody, &held(env.prod, "198.51.100.17", lived)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{problem}");
    // One read journaled: the one answered while the cluster lived.
    assert_eq!(audited(&pool, "data.objects.read").await.len(), 1);
    assert!(audited(&pool, "authz.access.denied").await.is_empty());
    // The port: the rows are kept, not served, and it says so.
    let mut conn = pool.acquire().await.unwrap();
    let subject = principal(who, IdentityKind::Person).subject();
    let network = prefix("198.51.100.17");
    let answer = who_had(&mut conn, &Unrestricted, &subject, network, env.pods, lived)
        .await
        .unwrap();
    assert!(answer.observations.is_empty());
    assert_eq!(answer.reason, Some(Cut::Closed));
    let answer = who_had(&mut conn, &Unrestricted, &subject, network, env.pods, after)
        .await
        .unwrap();
    assert!(answer.observations.is_empty());
    assert_eq!(answer.reason, None);
}

/// A pod in `hostNetwork` is answered by the addresses of its node in the version valid at
/// the instant asked: the address of before a change answers before it, not after; the
/// address of after, after it, not before.
#[kartenn_testing::pg_test]
async fn host_network_by_the_node_version_at_the_instant(pool: PgPool) {
    let env = Env::new(&pool).await;
    env.recorded(&Batch {
        opened: vec![host_pod(&env, 3)],
        closed: Vec::new(),
    })
    .await;
    let before = clock(&pool).await;
    // The node `worker-1` now at `192.0.2.22` instead of `192.0.2.21`.
    env.applied(
        env.connector,
        vec![sync(
            "node",
            env.prod,
            "worker-1",
            json!({"pod_cidrs": ["198.51.100.0/26"],
                   "addresses": [{"type": "InternalIP", "value": "192.0.2.22"}]}),
        )],
    )
    .await;
    let versions: i64 = sqlx::query_scalar("SELECT count(*) FROM kube.node WHERE id = $1")
        .bind(env.node)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(versions, 2, "a second version of the node");
    let after = clock(&pool).await;
    let who = reader(&pool, &["kube.observation.read"], Some(env.pods)).await;
    let app = as_person(&pool, who);
    for (ip, at, expected) in [
        ("192.0.2.21", before, vec![Uuid::from_u128(3)]),
        ("192.0.2.22", before, Vec::new()),
        ("192.0.2.22", after, vec![Uuid::from_u128(3)]),
        ("192.0.2.21", after, Vec::new()),
    ] {
        let (status, page) = get(&app, &held(env.prod, ip, at)).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(pods(&page), expected, "{ip} at {at}");
    }
}
