//! The chain of execution of a workload (task 019-7b; REQ-019-11; Q-019-31): its nodes by
//! the journal, their machines and what hosts them at the date, framed as `who_had` and
//! journaled; and `kube.machine.looks_like_pod` (REQ-019-3; Q-019-32), evaluated by the
//! module on the machines a changeset writes, the pod never named (Q-019-32, amended).

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use kartenn_kube::observations::{Batch, Opening};
use kartenn_kube::read::{Chain, Cut, Link, Reading, execution_chain};
use kartenn_service::audit::{Instance, RequestContext};
use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::rbac::PgAuthorizer;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use tower::ServiceExt;

use super::super::{Env, closing, hours, sync};
use super::audited;
use super::read::{Nothing, as_person, clock, get, reader};

/// `machines.<name>` from its JSON fields.
fn machines(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// The node `name` of `prod` on the machine `machine`, with the pod range `range` and the
/// address `address`.
fn node_on(env: &Env, (name, address): (&str, &str), machine: Uuid, range: &str) -> Operation {
    sync(
        "node",
        env.prod,
        name,
        json!({"machine": {"id": machine}, "pod_cidrs": [range],
               "addresses": [{"type": "InternalIP", "value": address},
                             {"type": "Hostname", "value": name}]}),
    )
}

/// `operations` submitted by `author`, applied.
async fn submitted(env: &Env, author: Uuid, operations: Vec<Operation>) -> Outcome {
    let request = ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        approval_self: true,
    };
    let outcome = submit(&env.store, &Unrestricted, &request, MAX_OPERATIONS)
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    outcome
}

/// The messages of the warnings `kube.machine.looks_like_pod` of `outcome`, with their
/// subjects.
fn lookalikes(outcome: &Outcome) -> Vec<(String, Vec<Uuid>)> {
    match outcome {
        Outcome::Applied { warnings, .. } => warnings
            .iter()
            .filter(|v| v.rule.id == "kube.machine.looks_like_pod")
            .map(|v| (v.message.clone(), v.subjects.clone()))
            .collect(),
        other => panic!("not applied: {other:?}"),
    }
}

/// The living machine named `name`.
async fn machine(env: &Env, name: &str) -> Uuid {
    env.id("machines.machine", "name", name).await
}

/// The living node named `name` of `prod`.
async fn node(env: &Env, name: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT id FROM kube.node WHERE cluster_id = $1 AND name = $2 AND valid_period @> now()",
    )
    .bind(env.prod)
    .bind(name)
    .fetch_one(&env.pool)
    .await
    .unwrap()
}

/// Whether `object` has an open anomaly `kube.machine.looks_like_pod`.
async fn anomaly(pool: &PgPool, object: Uuid) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM anomaly
                         WHERE object_id = $1 AND rule_id = 'kube.machine.looks_like_pod'
                           AND closed_at IS NULL)",
    )
    .bind(object)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The message of the open anomaly `kube.machine.looks_like_pod` of `object`.
async fn anomaly_message(pool: &PgPool, object: Uuid) -> String {
    sqlx::query_scalar(
        "SELECT message FROM anomaly
          WHERE object_id = $1 AND rule_id = 'kube.machine.looks_like_pod'
            AND closed_at IS NULL",
    )
    .bind(object)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The pod `pod` of `web` on `node` with the IP `ip`, seen from `from`.
fn pod_on(env: &Env, pod: u128, node: Uuid, ip: &str, from: OffsetDateTime) -> Opening {
    Opening {
        node: Some(node),
        from,
        ..env.pod(pod, ip)
    }
}

/// REQ-019-11 (chaîne d'exécution à la date; Q-019-31): the nodes the journal saw the
/// workload on at the date, the machine of each node and the hypervisor or the cluster of
/// virtualisation that hosts it, as they were then; a date before its pods, no link and no
/// reason; past the retention, `retention`; a workload never observed, `unobserved`; a
/// closed one, `closed`, unjournaled; nothing for an unknown workload or a reader without
/// `kube.observation.read` on the pod domain. Each read answered is journaled with the
/// pods it relied on.
#[kartenn_testing::pg_test]
async fn execution_chain_at_date(pool: PgPool) {
    let env = Env::new(&pool).await;
    let owner = json!({"id": env.acme});
    env.applied(
        env.admin,
        vec![
            machines(
                "create_machine",
                json!({"name": "hv-1", "kind": "physical", "owner": owner}),
            ),
            machines(
                "create_machine",
                json!({"name": "hv-2", "kind": "physical", "owner": owner}),
            ),
            machines("create_cluster", json!({"slug": "vc", "owner": owner})),
        ],
    )
    .await;
    let (hv1, hv2) = (machine(&env, "hv-1").await, machine(&env, "hv-2").await);
    let vc = env.id("machines.cluster", "slug", "vc").await;
    env.applied(
        env.admin,
        vec![
            machines(
                "create_machine",
                json!({"name": "vm-1", "kind": "virtual", "owner": owner,
                       "host": {"machine": {"id": hv1}}}),
            ),
            machines(
                "create_machine",
                json!({"name": "vm-2", "kind": "virtual", "owner": owner,
                       "host": {"cluster": {"id": vc}}}),
            ),
        ],
    )
    .await;
    let (vm1, vm2) = (machine(&env, "vm-1").await, machine(&env, "vm-2").await);
    env.applied(
        env.connector,
        vec![
            node_on(&env, ("worker-1", "192.0.2.21"), vm1, "198.51.100.0/26"),
            node_on(&env, ("worker-2", "192.0.2.22"), vm2, "198.51.100.64/26"),
            sync(
                "workload",
                env.prod,
                "api",
                json!({"namespace": "payments", "kind": "deployment"}),
            ),
        ],
    )
    .await;
    let (worker1, worker2) = (node(&env, "worker-1").await, node(&env, "worker-2").await);
    let api: Uuid = sqlx::query_scalar(
        "SELECT id FROM kube.workload WHERE name = 'api' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // Two pods of `web`, one on each node; the second one gone, then `vm-1` moved.
    let seen = clock(&pool).await;
    env.recorded(&Batch {
        opened: vec![
            pod_on(&env, 1, worker1, "198.51.100.17", seen),
            pod_on(&env, 2, worker2, "198.51.100.70", seen),
        ],
        closed: Vec::new(),
    })
    .await;
    let gone = clock(&pool).await;
    env.recorded(&Batch {
        opened: Vec::new(),
        closed: vec![closing(2, "198.51.100.70", gone)],
    })
    .await;
    env.applied(
        env.admin,
        vec![machines(
            "update_machine",
            json!({"id": {"id": vm1}, "host": {"machine": {"id": hv2}}}),
        )],
    )
    .await;
    let now = clock(&pool).await;

    let who = reader(&pool, &["kube.observation.read"], Some(env.pods)).await;
    let context = RequestContext::person(who);
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("test"));
    let reading = |context| Reading {
        context,
        journal: &journal,
        audit: ReadAudit::default(),
    };
    let mut conn = pool.acquire().await.unwrap();
    let mut chain = async |authorizer: &PgAuthorizer, workload: Uuid, at: OffsetDateTime| {
        execution_chain(&mut conn, authorizer, reading(&context), workload, at)
            .await
            .unwrap()
    };
    let rbac = PgAuthorizer::new(pool.clone());
    let link = |node, machine, hypervisor, cluster| Link {
        node,
        machine: Some(machine),
        hypervisor,
        cluster,
    };
    let answered = |links: Vec<Link>, reason| Some(Chain { links, reason });

    // While both pods ran: each node, its machine and what hosted it then.
    let mut both = vec![
        link(worker1, vm1, Some(hv1), None),
        link(worker2, vm2, None, Some(vc)),
    ];
    both.sort_by_key(|l| l.node);
    assert_eq!(chain(&rbac, env.web, seen).await, answered(both, None));
    // Now: the pod left on `worker-1`, its machine on the hypervisor it moved to.
    assert_eq!(
        chain(&rbac, env.web, now).await,
        answered(vec![link(worker1, vm1, Some(hv2), None)], None)
    );
    // Before its pods: no link, no reason.
    let before = seen - std::time::Duration::from_secs(1);
    assert_eq!(
        chain(&rbac, env.web, before).await,
        answered(Vec::new(), None)
    );
    // Past the retention of the cluster (30 days by default).
    let old = now - hours(31 * 24);
    assert_eq!(
        chain(&rbac, env.web, old).await,
        answered(Vec::new(), Some(Cut::Retention))
    );
    // A workload never observed.
    assert_eq!(
        chain(&rbac, api, now).await,
        answered(Vec::new(), Some(Cut::Unobserved))
    );
    // An unknown workload.
    assert_eq!(chain(&rbac, Uuid::now_v7(), now).await, None);

    // Without the right on the pod domain, or kartenn itself: nothing.
    let nobody = RequestContext::person(reader(&pool, &["kube.workload.read"], None).await);
    let answer = execution_chain(&mut conn, &rbac, reading(&nobody), env.web, now).await;
    assert_eq!(answer.unwrap(), None);
    let answer = execution_chain(&mut conn, &Nothing, reading(&context), env.web, now).await;
    assert_eq!(answer.unwrap(), None);
    let system = RequestContext::system();
    let answer = execution_chain(&mut conn, &Unrestricted, reading(&system), env.web, now).await;
    assert_eq!(answer.unwrap(), None);

    // Each read answered is journaled, with the pods it relied on.
    let reads = audited(&pool, "data.objects.read").await;
    assert_eq!(reads.len(), 5, "{reads:?}");
    let (details, actor, targets, outcome) = &reads[0];
    assert_eq!((*actor, outcome.as_str()), (Some(who), "success"));
    assert_eq!(targets, &[Uuid::from_u128(1), Uuid::from_u128(2)]);
    assert_eq!(details["criteria"]["read"], "kube.execution_chain");
    assert_eq!(details["criteria"]["workload"], json!(env.web));
    assert!(details["criteria"]["valid_at"].is_string(), "{details}");
    assert_eq!(reads[1].2, [Uuid::from_u128(1)]);
    assert!(reads[2..].iter().all(|r| r.2.is_empty()), "{reads:?}");

    // The workload closed by the connector: `closed`, unjournaled.
    env.applied(
        env.connector,
        vec![super::super::op(
            "sync",
            json!({"action": "close", "type": "kube.workload", "cluster": {"id": env.prod},
                   "name": "web", "namespace": "payments", "kind": "deployment"}),
        )],
    )
    .await;
    let answer = execution_chain(&mut conn, &rbac, reading(&context), env.web, seen).await;
    assert_eq!(answer.unwrap(), answered(Vec::new(), Some(Cut::Closed)));
    assert_eq!(audited(&pool, "data.objects.read").await.len(), 5);
}

/// REQ-019-3 (Q-019-32 and its amendments): a machine `container` hosted by the machine of
/// a node and named as a pod observed there warns `kube.machine.looks_like_pod`, the
/// changeset applied, to an author who holds `kube.observation.read` on the pod domain
/// only; the warning and its anomaly say « looks like a pod of a cluster », never the
/// pod; an author without the right gets no warning and leaves the anomaly as it is, but
/// closes it by moving the machine; a pod gone, or another name, does not warn.
#[kartenn_testing::pg_test]
async fn looks_like_pod_warns(pool: PgPool) {
    let env = Env::new(&pool).await;
    let owner = json!({"id": env.acme});
    env.applied(
        env.admin,
        vec![machines(
            "create_machine",
            json!({"name": "hv-1", "kind": "physical", "owner": owner}),
        )],
    )
    .await;
    let hv1 = machine(&env, "hv-1").await;
    env.applied(
        env.admin,
        vec![machines(
            "create_machine",
            json!({"name": "vm-1", "kind": "virtual", "owner": owner,
                   "host": {"machine": {"id": hv1}}}),
        )],
    )
    .await;
    let vm1 = machine(&env, "vm-1").await;
    env.applied(
        env.connector,
        vec![node_on(
            &env,
            ("worker-1", "192.0.2.21"),
            vm1,
            "198.51.100.0/26",
        )],
    )
    .await;
    // `web-1` runs on `worker-1`; `web-2` ran there, gone since.
    env.recorded(&Batch {
        opened: vec![env.pod(1, "198.51.100.17"), env.pod(2, "198.51.100.18")],
        closed: Vec::new(),
    })
    .await;
    env.recorded(&Batch {
        opened: Vec::new(),
        closed: vec![closing(
            2,
            "198.51.100.18",
            OffsetDateTime::now_utc() - hours(1) / 2,
        )],
    })
    .await;
    let container = |name: &str, host: Uuid| {
        machines(
            "create_machine",
            json!({"name": name, "kind": "container", "owner": owner,
                   "host": {"machine": {"id": host}}}),
        )
    };

    // An author without the right on the journal: no warning, which would answer whether
    // such a pod runs there.
    let generic = "the container machine web-1 looks like a pod of a cluster";
    let outcome = submitted(&env, env.admin, vec![container("web-1", vm1)]).await;
    let ct = machine(&env, "web-1").await;
    assert!(lookalikes(&outcome).is_empty(), "{outcome:?}");
    assert!(!anomaly(&pool, ct).await);
    // An author who holds it, the same machine: the warning, generic, and its anomaly.
    let who = reader(&pool, &["kube.observation.read"], Some(env.pods)).await;
    let rehost = |host: Uuid| {
        machines(
            "update_machine",
            json!({"id": {"id": ct}, "host": {"machine": {"id": host}}}),
        )
    };
    let outcome = submitted(&env, who, vec![rehost(vm1)]).await;
    let found = lookalikes(&outcome);
    assert_eq!(found.len(), 1, "{outcome:?}");
    assert_eq!(found[0].1, [ct]);
    assert_eq!(found[0].0, generic);
    assert_eq!(anomaly_message(&pool, ct).await, generic);
    // Written again by the author without the right, still alike: no warning, the
    // anomaly kept.
    let outcome = submitted(&env, env.admin, vec![rehost(vm1)]).await;
    assert!(lookalikes(&outcome).is_empty(), "{outcome:?}");
    assert!(anomaly(&pool, ct).await);
    // Moved off the machine of the node by that author: the anomaly closed, which
    // reveals nothing.
    let outcome = submitted(&env, env.admin, vec![rehost(hv1)]).await;
    assert!(lookalikes(&outcome).is_empty(), "{outcome:?}");
    assert!(!anomaly(&pool, ct).await);
    // A pod gone, a name of no pod: no warning.
    let outcome = submitted(
        &env,
        who,
        vec![container("web-2", vm1), container("api-1", vm1)],
    )
    .await;
    assert!(lookalikes(&outcome).is_empty(), "{outcome:?}");
    // Back on the machine of the node, then closed by a changeset of its own, by an
    // author without the right: the anomaly closed with the machine.
    submitted(&env, who, vec![rehost(vm1)]).await;
    assert!(anomaly(&pool, ct).await);
    let close = Operation::Close {
        kind: ObjectKind::from_name("machines.machine").unwrap(),
        id: Ref::Id(ct),
        expected: None,
        cascade: false,
    };
    let outcome = submitted(&env, env.admin, vec![close]).await;
    assert!(lookalikes(&outcome).is_empty(), "{outcome:?}");
    assert!(!anomaly(&pool, ct).await);
}

/// `GET uri` by `app` with `If-None-Match: etag` when given: its status and `ETag`.
async fn revalidated(app: &Router, uri: &str, etag: Option<&str>) -> (StatusCode, Option<String>) {
    let mut request = Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, "Bearer test");
    if let Some(etag) = etag {
        request = request.header(header::IF_NONE_MATCH, etag);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let etag = response
        .headers()
        .get(header::ETAG)
        .map(|v| v.to_str().unwrap().to_owned());
    (response.status(), etag)
}

/// Task 019-8c (REQ-019-11; Q-019-28, 31): `GET /v1/workloads/{id}/chain` answers the
/// links of the port at `valid_at` (now by default), with the reasons `retention` and
/// `unobserved`, a weak `ETag` and a 304 on revalidation; no `as_of`; a workload unknown,
/// or invisible to the caller, 404 unjournaled; visible without `kube.observation.read`
/// on the pod domain, 403 journaled; closed, 410 to whoever may read it, unjournaled.
/// Each chain answered is journaled once, by the port.
#[kartenn_testing::pg_test]
async fn chain_route_framed(pool: PgPool) {
    let env = Env::new(&pool).await;
    let owner = json!({"id": env.acme});
    env.applied(
        env.admin,
        vec![machines(
            "create_machine",
            json!({"name": "hv-1", "kind": "physical", "owner": owner}),
        )],
    )
    .await;
    let hv1 = machine(&env, "hv-1").await;
    env.applied(
        env.admin,
        vec![machines(
            "create_machine",
            json!({"name": "vm-1", "kind": "virtual", "owner": owner,
                   "host": {"machine": {"id": hv1}}}),
        )],
    )
    .await;
    let vm1 = machine(&env, "vm-1").await;
    env.applied(
        env.connector,
        vec![
            node_on(&env, ("worker-1", "192.0.2.21"), vm1, "198.51.100.0/26"),
            sync(
                "workload",
                env.prod,
                "api",
                json!({"namespace": "payments", "kind": "deployment"}),
            ),
        ],
    )
    .await;
    let worker1 = node(&env, "worker-1").await;
    let api: Uuid = sqlx::query_scalar(
        "SELECT id FROM kube.workload WHERE name = 'api' AND valid_period @> now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // A pod of `web` on `worker-1`, seen an hour ago.
    env.recorded(&Batch {
        opened: vec![env.pod(1, "198.51.100.17")],
        closed: Vec::new(),
    })
    .await;
    let now = clock(&pool).await;
    let chain = |workload: Uuid| format!("/v1/workloads/{workload}/chain");
    let at = |workload: Uuid, instant: OffsetDateTime| {
        format!("{}?valid_at={}", chain(workload), super::rfc3339(instant))
    };

    let who = reader(&pool, &["kube.observation.read"], Some(env.pods)).await;
    let app = as_person(&pool, who);
    // Now: the node, its machine and the hypervisor of that machine.
    let (status, body) = get(&app, &chain(env.web)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"links": [{"node": {"id": worker1}, "machine": {"id": vm1},
                          "hypervisor": {"id": hv1}, "virtualization_cluster": null}],
               "reason": null})
    );
    // Before its pod: no link, no reason.
    let (status, body) = get(&app, &at(env.web, now - hours(2))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({"links": [], "reason": null}));
    // Past the retention of the cluster (30 days by default).
    let (status, body) = get(&app, &at(env.web, now - hours(31 * 24))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({"links": [], "reason": "retention"}));
    // A workload never observed.
    let (status, body) = get(&app, &chain(api)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({"links": [], "reason": "unobserved"}));
    // A weak tag of the content, revalidated.
    let uri = at(env.web, now);
    let (status, etag) = revalidated(&app, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    let etag = etag.expect("an ETag");
    assert!(etag.starts_with("W/"), "{etag}");
    let (status, _) = revalidated(&app, &uri, Some(&etag)).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    // No `as_of`, no date without offset: 400.
    for uri in [
        format!("{}?as_of={}", chain(env.web), super::rfc3339(now)),
        format!("{}?valid_at=2026-01-01T00:00:00", chain(env.web)),
    ] {
        let (status, problem) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {problem}");
    }
    // Each chain answered journaled once, by the port.
    let reads = audited(&pool, "data.objects.read").await;
    assert_eq!(reads.len(), 6, "{reads:?}");
    assert!(
        reads
            .iter()
            .all(|r| r.0["criteria"]["read"] == "kube.execution_chain"),
        "{reads:?}"
    );
    assert_eq!(reads[0].2, [Uuid::from_u128(1)]);
    assert_eq!(reads[0].1, Some(who));

    // Unknown, or invisible to the caller: 404, unjournaled.
    let nobody = as_person(&pool, reader(&pool, &[], None).await);
    for (app, workload) in [(&app, Uuid::now_v7()), (&nobody, env.web)] {
        let (status, problem) = get(app, &chain(workload)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{problem}");
    }
    assert!(audited(&pool, "authz.access.denied").await.is_empty());
    // Visible without the right on the pod domain: 403, journaled.
    let workloads = as_person(&pool, reader(&pool, &["kube.workload.read"], None).await);
    let (status, problem) = get(&workloads, &chain(env.web)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    let denials: Vec<(Value, Vec<Uuid>)> = audited(&pool, "authz.access.denied")
        .await
        .into_iter()
        .map(|(details, _, targets, _)| (details, targets))
        .collect();
    assert_eq!(
        denials,
        [(
            json!({"permission": "kube.observation.read"}),
            vec![env.web]
        )]
    );

    // Closed by the connector: 410 to the reader, 404 to anyone else, neither journaled.
    env.applied(
        env.connector,
        vec![super::super::op(
            "sync",
            json!({"action": "close", "type": "kube.workload", "cluster": {"id": env.prod},
                   "name": "web", "namespace": "payments", "kind": "deployment"}),
        )],
    )
    .await;
    let (status, problem) = get(&app, &chain(env.web)).await;
    assert_eq!(status, StatusCode::GONE, "{problem}");
    assert_eq!(problem["type"], "urn:kartenn:problem:gone");
    let (status, problem) = get(&nobody, &chain(env.web)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{problem}");
    assert_eq!(audited(&pool, "data.objects.read").await.len(), 6);
    assert_eq!(audited(&pool, "authz.access.denied").await.len(), 1);
}
