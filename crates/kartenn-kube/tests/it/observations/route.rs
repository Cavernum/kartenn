//! The batch of observations through its route (task 019-6b; REQ-019-16; ADR 0043,
//! decisions 2, 3, 5; Q-019-21 to 23): `POST /v1/kube-clusters/{id}/observations` under
//! `kube.observation.record` on the pod domain of the cluster, by its connector alone,
//! refused whole by its lines (422 `invalid-batch`), written and audited once in its
//! transaction, and seen by no subscriber of the event stream.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_core::audit::seal::hex;
use kartenn_kube::Kube;
use kartenn_kube::observations::{Closing, Opening};
use kartenn_machines::Machines;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::events::hub::{Hub, Settings as EventSettings};
use kartenn_rest::events::reader::Reader;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::Operation;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::events::{Parsed, Parser};
use kartenn_service::module::{Module, ModuleRegistry};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::watch;
use tower::ServiceExt;

use super::{Env, closing, op};

/// The chain of execution of a workload and the machines that look like its pods
/// (task 019-7b).
mod chain;
/// The collection of the clusters (task 019-8a).
mod clusters;
/// The reads of the same journal (task 019-6d).
mod read;

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

/// The identity `identity` of `kind`, by a token.
fn principal(identity: Uuid, kind: IdentityKind) -> Principal {
    Principal {
        identity,
        kind,
        issuer: None,
        groups: Vec::new(),
        method: Method::Bearer,
    }
}

/// The router of an instance with both modules and their routes, as `who`, its event
/// stream on `hub` when given.
fn app(pool: &PgPool, who: Principal, hub: Option<&Arc<Hub>>) -> Router {
    let modules: Vec<Arc<dyn Module>> = vec![Arc::new(Machines), Arc::new(Kube)];
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: Arc::new(ModuleRegistry::new(modules).unwrap()),
    };
    let mut v1 = kartenn_rest::v1_with(api, vec![kartenn_kube::rest::routes()]);
    if let Some(hub) = hub {
        v1 = v1.layer(axum::Extension(hub.clone()));
    }
    kartenn_rest::router(
        v1,
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates::default())),
        Arc::new(As(who)),
    )
}

/// A role of `permissions` bound to `group` on the domain `domain`, from a day ago.
async fn bind(pool: &PgPool, group: Uuid, domain: Uuid, permissions: &[&str]) {
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
    sqlx::query(
        "INSERT INTO binding (id, group_id, role_id, scope_kind, scope_id, valid_period,
                              changeset_id)
         VALUES (gen_random_uuid(), $1, $2, 'domain', $3,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(group)
    .bind(role)
    .bind(domain)
    .execute(pool)
    .await
    .unwrap();
}

/// A person granted `permissions` everywhere, by a role of their own.
async fn person(pool: &PgPool, permissions: &[&str]) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
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
        .bind("global")
        .bind(None::<Uuid>)
        .execute(pool)
        .await
        .unwrap();
    identity
}

/// The connector group `cluster` designates.
async fn group_of(pool: &PgPool, cluster: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT connector_group_id FROM kube.cluster WHERE id = $1")
        .bind(cluster)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap()
}

/// An opening as the body writes it.
fn opened(o: &Opening) -> Value {
    json!({
        "namespace": o.namespace,
        "pod_name": o.pod_name,
        "pod_uid": o.pod_uid,
        "workload": o.workload.map(|id| json!({"id": id})),
        "node": o.node.map(|id| json!({"id": id})),
        "ip": o.ip.map(|ip| ip.to_string()),
        "host_network": o.host_network,
        "from": rfc3339(o.from),
    })
}

/// A closing as the body writes it.
fn closed(c: &Closing) -> Value {
    json!({
        "pod_uid": c.pod_uid,
        "ip": c.ip.map(|ip| ip.to_string()),
        "at": rfc3339(c.at),
    })
}

/// The body of a batch.
fn body(opened_lines: &[Opening], closed_lines: &[Closing]) -> Value {
    json!({
        "opened": opened_lines.iter().map(opened).collect::<Vec<_>>(),
        "closed": closed_lines.iter().map(closed).collect::<Vec<_>>(),
    })
}

/// `POST /v1/kube-clusters/{cluster}/observations` of `body` by `app`: its status and body.
async fn post(app: &Router, cluster: Uuid, body: &Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/v1/kube-clusters/{cluster}/observations"))
        .header(header::AUTHORIZATION, "Bearer test")
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

/// The audit events `name`, in order: their details, actor, targets and outcome.
async fn audited(pool: &PgPool, name: &str) -> Vec<(Value, Option<Uuid>, Vec<Uuid>, String)> {
    sqlx::query_as(
        "SELECT details, actor_id, coalesce(target_ids, '{}'), outcome FROM audit.event
          WHERE name = $1 ORDER BY occurred_at, id",
    )
    .bind(name)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// REQ-019-16 (audit par lot; ADR 0043, decision 5): each batch accepted by the route
/// answers 201 with its identifier, counts and fingerprint in hexadecimal, and inscribes
/// one `observation.batch.recorded` with the same fingerprint as `observation_batch`; an
/// empty batch is audited as the others (Q-019-21).
#[kartenn_testing::pg_test]
async fn batch_is_audited_once_with_hash(pool: PgPool) {
    let env = Env::new(&pool).await;
    bind(
        &pool,
        group_of(&pool, env.prod).await,
        env.pods,
        &["kube.observation.record"],
    )
    .await;
    let app = app(
        &pool,
        principal(env.connector, IdentityKind::ServiceAccount),
        None,
    );
    let (status, answer) = post(
        &app,
        env.prod,
        &body(
            &[env.pod(1, "198.51.100.17"), env.pod(2, "198.51.100.18")],
            &[],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    let id: Uuid = answer["id"].as_str().unwrap().parse().unwrap();
    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT content_hash FROM kube.observation_batch WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let hash = hex(&stored);
    let earliest: OffsetDateTime =
        sqlx::query_scalar("SELECT earliest FROM kube.observation_batch WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(answer["hash"], hash.as_str());
    assert_eq!(
        (answer["opened"].as_u64(), answer["closed"].as_u64()),
        (Some(2), Some(0))
    );
    OffsetDateTime::parse(answer["received_at"].as_str().unwrap(), &Rfc3339).unwrap();
    let events = audited(&pool, "observation.batch.recorded").await;
    assert_eq!(events.len(), 1, "one event for the batch: {events:?}");
    let (details, actor, targets, outcome) = &events[0];
    assert_eq!(
        details,
        &json!({
            "journal": "kube.pod_observation",
            "cluster": env.prod,
            "batch": id,
            "opened": 2,
            "closed": 0,
            "hash": hash,
            "clamped": 0,
            "skew_max": 0,
            "earliest": kartenn_kube::observations::rfc3339(earliest),
        })
    );
    assert_eq!(*actor, Some(env.connector));
    assert_eq!(targets, &[env.prod]);
    assert_eq!(outcome, "success");
    // A closing, then an empty batch: one event each, the empty one with its own row.
    let (status, second) = post(
        &app,
        env.prod,
        &body(
            &[],
            &[closing(1, "198.51.100.17", OffsetDateTime::now_utc())],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{second}");
    assert_eq!(
        (second["opened"].as_u64(), second["closed"].as_u64()),
        (Some(0), Some(1))
    );
    let (status, empty) = post(&app, env.prod, &body(&[], &[])).await;
    assert_eq!(status, StatusCode::CREATED, "{empty}");
    let events = audited(&pool, "observation.batch.recorded").await;
    let batches: Vec<&Value> = events.iter().map(|(d, ..)| &d["batch"]).collect();
    assert_eq!(batches, [&answer["id"], &second["id"], &empty["id"]]);
    assert_eq!(events[2].0["hash"], empty["hash"]);
    assert_eq!(env.counts().await, (2, 3));
}

/// REQ-019-16 (droit propre; Q-019-23): the right is evaluated on the pod domain of the
/// cluster, and held there by the connector of another cluster only refuses at the
/// membership: 403 either way, journaled as `authz.access.denied`, nothing written; a
/// cluster that does not live is 404.
#[kartenn_testing::pg_test]
async fn batch_from_other_cluster_connector_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    env.cluster("stage", group).await;
    let batch = body(&[env.pod(1, "198.51.100.17")], &[]);
    // Without any right, the connector of `prod` itself: the cluster answers as an absent
    // one, as an unknown identifier does, and nothing is journaled (ADR 0021).
    let theirs = app(
        &pool,
        principal(env.connector, IdentityKind::ServiceAccount),
        None,
    );
    for cluster in [env.prod, Uuid::now_v7()] {
        let (status, problem) = post(&theirs, cluster, &batch).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{problem}");
    }
    assert!(audited(&pool, "authz.access.denied").await.is_empty());
    // Who reads the cluster without the right on its pod domain: 403.
    let reader = person(&pool, &["kube.cluster.read"]).await;
    let theirs = app(&pool, principal(reader, IdentityKind::Person), None);
    let (status, problem) = post(&theirs, env.prod, &batch).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["type"], "urn:kartenn:problem:forbidden");
    // With the right on the domain, the connector of `stage` (the same pod domain).
    bind(&pool, group, env.pods, &["kube.observation.record"]).await;
    let stranger = app(&pool, principal(other, IdentityKind::ServiceAccount), None);
    let (status, problem) = post(&stranger, env.prod, &batch).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(env.counts().await, (0, 0), "nothing written");
    let denials = audited(&pool, "authz.access.denied").await;
    assert_eq!(denials.len(), 2, "{denials:?}");
    for ((details, actor, targets, outcome), who) in denials.iter().zip([reader, other]) {
        assert_eq!(details, &json!({"permission": "kube.observation.record"}));
        assert_eq!(
            (*actor, targets.as_slice()),
            (Some(who), [env.prod].as_slice())
        );
        assert_eq!(outcome, "denied");
    }
    assert!(
        audited(&pool, "observation.batch.recorded")
            .await
            .is_empty()
    );
}

/// REQ-019-16 (validation; Q-019-21): a batch with an IP outside the cluster, a node of
/// another cluster or a closing of nothing open is refused whole by the route: 422
/// `invalid-batch`, `code` `kube.observation.invalid`, each line by its JSON pointer,
/// nothing written, journaled as `abuse.input.rejected`; over `batch_max_operations`
/// lines, the same class for the whole document; a malformed line is 400.
#[kartenn_testing::pg_test]
async fn batch_outside_pod_ranges_refused_whole(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    let stage = env.cluster("stage", group).await;
    let (stage_node, _) = env.populated(stage, other, "198.51.100.64/26").await;
    bind(
        &pool,
        group_of(&pool, env.prod).await,
        env.pods,
        &["kube.observation.record"],
    )
    .await;
    let app = app(
        &pool,
        principal(env.connector, IdentityKind::ServiceAccount),
        None,
    );
    let mut foreign_node = env.pod(3, "198.51.100.19");
    foreign_node.node = Some(stage_node);
    let now = OffsetDateTime::now_utc();
    let (status, problem) = post(
        &app,
        env.prod,
        &body(
            &[
                env.pod(1, "198.51.100.17"),
                env.pod(2, "192.0.2.10"),
                foreign_node,
            ],
            &[closing(9, "198.51.100.40", now)],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(problem["type"], "urn:kartenn:problem:invalid-batch");
    assert_eq!(problem["code"], "kube.observation.invalid");
    let pointers: Vec<&str> = problem["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            assert!(!v["detail"].as_str().unwrap().is_empty(), "{v}");
            v["pointer"].as_str().unwrap()
        })
        .collect();
    assert_eq!(pointers, ["/opened/1", "/opened/2", "/closed/0"]);
    assert_eq!(
        env.counts().await,
        (0, 0),
        "nothing of the batch is written"
    );
    let rejected = audited(&pool, "abuse.input.rejected").await;
    assert_eq!(rejected.len(), 1, "{rejected:?}");
    assert_eq!(rejected[0].0, json!({"reason": "kube.observation.invalid"}));
    assert_eq!(rejected[0].2, [env.prod]);
    // Over the ceiling of the cluster: refused before anything is read.
    env.applied(
        env.admin,
        vec![op(
            "update_cluster",
            json!({"id": {"id": env.prod}, "batch_max_operations": 2}),
        )],
    )
    .await;
    let lines = [
        env.pod(1, "198.51.100.17"),
        env.pod(2, "198.51.100.18"),
        env.pod(4, "198.51.100.20"),
    ];
    let (status, problem) = post(&app, env.prod, &body(&lines, &[])).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(problem["type"], "urn:kartenn:problem:invalid-batch");
    assert_eq!(problem["violations"][0]["pointer"], "");
    let (status, answer) = post(&app, env.prod, &body(&lines[..2], &[])).await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    // A malformed line: 400, as any body that does not parse.
    let mut wrong = body(&lines[..1], &[]);
    wrong["opened"][0]["ip"] = json!("198.51.100.300");
    let (status, problem) = post(&app, env.prod, &wrong).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{problem}");
    wrong["opened"][0]["ip"] = json!("198.51.100.21");
    wrong["opened"][0]["from"] = json!("2026-11-02T10:00:00");
    let (status, problem) = post(&app, env.prod, &wrong).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{problem}");
    wrong["opened"][0]["from"] = json!(rfc3339(now));
    wrong["opened"][0]["pod_ip"] = json!("198.51.100.21");
    let (status, problem) = post(&app, env.prod, &wrong).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an unknown member: {problem}"
    );
    assert_eq!(audited(&pool, "observation.batch.recorded").await.len(), 1);
    // Every refusal at the entry is journaled with its reason: the lines, the ceiling,
    // then the three bodies that do not read.
    let reasons: Vec<Value> = audited(&pool, "abuse.input.rejected")
        .await
        .into_iter()
        .map(|(details, ..)| details["reason"].clone())
        .collect();
    assert_eq!(
        reasons,
        [
            "kube.observation.invalid",
            "kube.observation.invalid",
            "bad-request",
            "bad-request",
            "bad-request",
        ]
    );
}

/// Q-019-26: an instant to come is brought back to the instant of the batch, which is
/// accepted; its event counts the lines brought back (`clamped`) and keeps their longest
/// advance (`skew_max`, in milliseconds).
#[kartenn_testing::pg_test]
async fn future_instant_clamped_and_audited(pool: PgPool) {
    let env = Env::new(&pool).await;
    bind(
        &pool,
        group_of(&pool, env.prod).await,
        env.pods,
        &["kube.observation.record"],
    )
    .await;
    let app = app(
        &pool,
        principal(env.connector, IdentityKind::ServiceAccount),
        None,
    );
    let mut ahead = env.pod(1, "198.51.100.17");
    ahead.from = OffsetDateTime::now_utc() + Duration::from_secs(3_600);
    let (status, answer) = post(
        &app,
        env.prod,
        &body(&[ahead, env.pod(2, "198.51.100.18")], &[]),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    let events = audited(&pool, "observation.batch.recorded").await;
    let details = &events[0].0;
    assert_eq!(details["clamped"], 1, "{details}");
    let skew = details["skew_max"].as_u64().unwrap();
    assert!(
        (3_590_000..=3_600_000).contains(&skew),
        "the advance on the instant of the batch: {skew}"
    );
}

/// A stream of events being read.
struct Stream {
    body: Body,
    parser: Parser,
    events: std::collections::VecDeque<Parsed>,
}

impl Stream {
    /// The next event of `kind` within `wait`, the others skipped.
    async fn next_of(&mut self, kind: &str, wait: Duration) -> Option<Parsed> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            while let Some(e) = self.events.pop_front() {
                if e.event == kind {
                    return Some(e);
                }
            }
            let frame = tokio::time::timeout_at(deadline, self.body.frame())
                .await
                .ok()??;
            if let Ok(data) = frame.unwrap().into_data() {
                let text = String::from_utf8(data.to_vec()).unwrap();
                self.events.extend(self.parser.feed(&text));
            }
        }
    }
}

/// REQ-019-16 (séparation; ADR 0043, decision 7): a subscriber of the event stream that
/// sees the changesets before and after a batch accepted by the route receives no
/// `changes` for the batch.
#[kartenn_testing::pg_test]
async fn batch_emits_no_changes_event(pool: PgPool) {
    const WAIT: Duration = Duration::from_secs(5);
    let env = Env::new(&pool).await;
    bind(
        &pool,
        group_of(&pool, env.prod).await,
        env.pods,
        &["kube.observation.record"],
    )
    .await;
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let settings = EventSettings {
        heartbeat: Duration::from_secs(5),
        checkpoint: Duration::from_millis(300),
        poll: Duration::from_millis(200),
        recheck: Duration::from_millis(300),
        ..EventSettings::default()
    };
    let hub = Hub::new(settings, pool.clone(), ring, Arc::default());
    let reader = Reader::start(hub.clone(), pool.clone()).await.unwrap();
    let (_stop, stopped) = watch::channel(false);
    tokio::spawn(reader.run(stopped.clone()));
    tokio::spawn(hub.clone().watch(stopped));
    let reader_identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'r') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, 'r_' || replace($1::text, '-', ''), jsonb_build_object('fr', 'r'),
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
         VALUES ($1, '*', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(kartenn_testing::BIND_IDENTITY)
        .bind(reader_identity)
        .bind(role)
        .bind("global")
        .bind(None::<Uuid>)
        .execute(&pool)
        .await
        .unwrap();
    let watcher = app(
        &pool,
        principal(reader_identity, IdentityKind::Person),
        Some(&hub),
    );
    let request = Request::get("/v1/events")
        .header(header::ACCEPT, "text/event-stream")
        .header(header::AUTHORIZATION, "Bearer t")
        .body(Body::empty())
        .unwrap();
    let response = watcher.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut stream = Stream {
        body: response.into_body(),
        parser: Parser::default(),
        events: std::collections::VecDeque::new(),
    };
    stream.next_of("ready", WAIT).await.expect("ready");
    let entity = |slug: &str| Operation::CreateEntity {
        local: None,
        slug: slug.into(),
        parent: None,
    };
    let changed = |e: &Parsed| -> Value {
        let data: Value = serde_json::from_str(&e.data).unwrap();
        data["objects"][0].clone()
    };
    env.applied(env.admin, vec![entity("before")]).await;
    let before = env.id("entity", "slug", "before").await;
    let first = stream
        .next_of("changes", WAIT)
        .await
        .expect("the first changeset");
    assert_eq!(
        changed(&first),
        json!({"type": "core.entity", "id": before})
    );
    let connector = app(
        &pool,
        principal(env.connector, IdentityKind::ServiceAccount),
        None,
    );
    let (status, answer) = post(
        &connector,
        env.prod,
        &body(
            &[env.pod(1, "198.51.100.17")],
            &[closing(1, "198.51.100.17", OffsetDateTime::now_utc())],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    env.applied(env.admin, vec![entity("after")]).await;
    let after = env.id("entity", "slug", "after").await;
    let next = stream
        .next_of("changes", WAIT)
        .await
        .expect("the next changeset");
    assert_eq!(
        changed(&next),
        json!({"type": "core.entity", "id": after}),
        "nothing between the two changesets"
    );
}
