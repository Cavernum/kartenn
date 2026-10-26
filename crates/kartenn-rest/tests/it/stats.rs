//! `GET /v1/stats` through the router (spec 024, task 7; REQ-024-14 to 16): each counter
//! under the read rights of the caller, a kind it reads nowhere absent, the changesets
//! only to a reader of the changesets, its own pending changesets and those it may apply;
//! the `ETag`, `computed_at` and the snapshots recomputed once the outbox moved (M18).

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::{Extension, Router};
use futures_util::future::join_all;
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::objects::Nature;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rate, Rates};
use kartenn_rest::stats::{Freshness, Snapshots};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common::As;

const READ_ALL: [&str; 6] = [
    "core.entity.read",
    "core.site.read",
    "core.domain.read",
    "core.prefix.read",
    "core.range.read",
    "core.address.read",
];

/// A person holding `permissions` on `scope` (`global`, or a domain).
async fn person(pool: &PgPool, permissions: &[&str], scope: (&str, Option<Uuid>)) -> Principal {
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
        .bind(scope.0)
        .bind(scope.1)
        .execute(pool)
        .await
        .unwrap();
    Principal {
        identity,
        kind: IdentityKind::Person,
        issuer: None,
        groups: Vec::new(),
        method: Method::Bearer,
    }
}

fn api(pool: &PgPool) -> Api {
    Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: Arc::default(),
    }
}

/// The router as `who`, the snapshots those of the process.
fn app(pool: &PgPool, who: Principal) -> Router {
    router(kartenn_rest::v1(api(pool)), pool, who)
}

/// The router as `who`, with `snapshots`.
fn app_with(pool: &PgPool, who: Principal, snapshots: &Arc<Snapshots>) -> Router {
    router(
        kartenn_rest::v1(api(pool)).layer(Extension(snapshots.clone())),
        pool,
        who,
    )
}

fn router(v1: Router, pool: &PgPool, who: Principal) -> Router {
    kartenn_rest::router(
        v1,
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates {
            read: Rate::new(1000, 1000).unwrap(),
            write: Rate::new(1000, 1000).unwrap(),
            ..Rates::default()
        })),
        Arc::new(As(who)),
    )
}

/// `GET /v1/stats`, with `If-None-Match` when given: the status, the `ETag` and the body.
async fn stats(app: &Router, tag: Option<&str>) -> (StatusCode, String, Value) {
    let mut request = Request::get("/v1/stats").header(header::AUTHORIZATION, "Bearer t");
    if let Some(tag) = tag {
        request = request.header(header::IF_NONE_MATCH, tag);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let tag = response.headers()[header::ETAG]
        .to_str()
        .unwrap()
        .to_owned();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, tag, body)
}

/// Applies `operations` as kartenn, its changeset.
async fn apply(pool: &PgPool, operations: Vec<Operation>) -> Uuid {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'w') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let request = ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        approval_self: true,
    };
    match submit(
        &PgStore::new(pool.clone()),
        &Unrestricted,
        &request,
        MAX_OPERATIONS,
    )
    .await
    .unwrap()
    {
        Outcome::Applied { changeset, .. } => changeset,
        other => panic!("not applied: {other:?}"),
    }
}

async fn domain(pool: &PgPool, slug: &str) -> Uuid {
    apply(
        pool,
        vec![Operation::CreateDomain {
            local: None,
            slug: slug.into(),
        }],
    )
    .await;
    sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = $1")
        .bind(slug)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn prefix_in(domain: Uuid, prefix: &str) -> Operation {
    Operation::CreatePrefix {
        local: None,
        domain: Ref::Id(domain),
        prefix: prefix.into(),
        nature: Nature::Container,
        point_to_point: false,
        anycast: false,
    }
}

/// REQ-024-14, 15: each caller counts what it reads, by the predicate of the lists; a
/// kind read nowhere and the changesets without `core.changeset.read` are absent; the
/// anomalies follow the read of their object; the pending changesets are the caller's.
#[kartenn_testing::pg_test]
async fn each_counter_follows_the_rights_of_the_caller(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let prod = domain(&pool, "prod").await;
    apply(&pool, vec![prefix_in(lab, "10.0.0.0/8")]).await;
    let opened_by = apply(&pool, vec![prefix_in(prod, "192.168.0.0/16")]).await;
    let hidden: Uuid = sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '192.168.0.0/16'")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO anomaly (rule_id, object_id, message, opened_by)
         VALUES ('core.test', $1, 'a test', $2)",
    )
    .bind(hidden)
    .bind(opened_by)
    .execute(&pool)
    .await
    .unwrap();

    let mut everything: Vec<&str> = READ_ALL.to_vec();
    everything.extend(["core.changeset.read", "core.prefix.apply"]);
    let admin = person(&pool, &everything, ("global", None)).await;
    let mut editing: Vec<&str> = READ_ALL.to_vec();
    editing.push("core.prefix.edit");
    let scoped = person(&pool, &editing, ("domain", Some(lab))).await;
    let nobody = person(&pool, &[], ("global", None)).await;

    // The scoped person submits a prefix of its domain without applying it: pending.
    let submitted = app(&pool, scoped.clone())
        .oneshot(
            Request::post("/v1/prefixes")
                .header(header::AUTHORIZATION, "Bearer t")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"motive": "a net", "domain": {"id": lab},
                           "value": "10.1.0.0/16", "nature": "network"})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(submitted.status(), StatusCode::ACCEPTED);

    let (status, _, all) = stats(&app(&pool, admin), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(all["objects"]["core.domain"], 2, "{all}");
    assert_eq!(all["objects"]["core.prefix"], 2);
    assert_eq!(all["objects"]["core.address"], 0);
    assert_eq!(all["anomalies"]["open"], 1);
    assert_eq!(all["pending"], json!({"to_apply": 1, "mine": 0}));
    // Every changeset of a new base is applied today, the migrations' included (Q-024-5).
    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM changeset WHERE state = 'applied'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(applied >= 4);
    let by_day = all["changesets"]["by_day"].as_array().unwrap();
    assert_eq!(by_day.len(), 30);
    assert_eq!(by_day[29], applied, "today, last: {all}");
    assert_eq!(all["changesets"]["applied_7d"], applied);
    assert_eq!(all["changesets"]["applied_30d"], applied);
    assert_eq!(all["modules"], json!({}));

    let (_, _, mine) = stats(&app(&pool, scoped), None).await;
    assert_eq!(mine["objects"]["core.domain"], 1, "{mine}");
    assert_eq!(mine["objects"]["core.prefix"], 1);
    assert_eq!(mine["anomalies"]["open"], 0);
    assert!(mine.get("changesets").is_none(), "{mine}");
    assert_eq!(mine["pending"], json!({"to_apply": 0, "mine": 1}));

    let (_, _, none) = stats(&app(&pool, nobody), None).await;
    assert_eq!(none["objects"], json!({}), "{none}");
    assert_eq!(none["anomalies"]["open"], 0);
    assert!(none.get("changesets").is_none());
    assert_eq!(none["pending"], json!({"to_apply": 0, "mine": 0}));
}

/// Snapshots due after `refresh`, the rest as by default.
fn snapshots(refresh: Duration) -> Arc<Snapshots> {
    Snapshots::new(Freshness {
        refresh,
        ..Freshness::default()
    })
}

/// Waits until `snapshots` counts `n` parts computed, and not one more.
async fn computed(snapshots: &Snapshots, n: u64) {
    for _ in 0..400 {
        if snapshots.computations() >= n {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(snapshots.computations(), n);
}

fn computed_at(body: &Value) -> OffsetDateTime {
    OffsetDateTime::parse(body["computed_at"].as_str().unwrap(), &Rfc3339).unwrap()
}

/// REQ-024-16, M18: a strong `ETag` answers 304; the first read computes, the next ones
/// are served the snapshot without computing, even past a changeset while it is recent;
/// once old enough, a read is served it once more and recomputes it behind, and the next
/// one counts the change, its `computed_at` later.
#[kartenn_testing::pg_test]
async fn the_etag_and_the_snapshot_follow_the_outbox(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let reader = person(&pool, &READ_ALL, ("global", None)).await;
    let refresh = Duration::from_millis(400);
    let snapshots = snapshots(refresh);
    let app = app_with(&pool, reader, &snapshots);
    let before = OffsetDateTime::now_utc() - Duration::from_secs(5);
    let (status, tag, first) = stats(&app, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(tag.starts_with('"'), "strong: {tag}");
    assert_eq!(snapshots.computations(), 2, "its rights' part and its own");
    assert!(computed_at(&first) >= before, "{first}");
    assert_eq!(first["objects"]["core.prefix"], 0);
    let (status, again, body) = stats(&app, Some(&tag)).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert_eq!(again, tag);
    assert_eq!(body, Value::Null);

    apply(&pool, vec![prefix_in(lab, "10.0.0.0/8")]).await;
    let (status, same, _) = stats(&app, Some(&tag)).await;
    assert_eq!(
        status,
        StatusCode::NOT_MODIFIED,
        "too recent to be recomputed"
    );
    assert_eq!(same, tag);
    assert_eq!(snapshots.computations(), 2);

    tokio::time::sleep(refresh).await;
    let (status, _, _) = stats(&app, Some(&tag)).await;
    assert_eq!(
        status,
        StatusCode::NOT_MODIFIED,
        "served, recomputed behind"
    );
    computed(&snapshots, 4).await;
    let (status, moved, after) = stats(&app, Some(&tag)).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(moved, tag);
    assert_eq!(after["objects"]["core.prefix"], 1, "{after}");
    assert!(computed_at(&after) > computed_at(&first), "{after}");
    assert_eq!(snapshots.computations(), 4);
}

/// M18: an old snapshot is not recomputed while the outbox has not moved, by a read or by
/// a sweep.
#[kartenn_testing::pg_test]
async fn nothing_changed_nothing_is_recomputed(pool: PgPool) {
    let reader = person(&pool, &READ_ALL, ("global", None)).await;
    let refresh = Duration::from_millis(200);
    let snapshots = snapshots(refresh);
    let app = app_with(&pool, reader, &snapshots);
    let (_, tag, _) = stats(&app, None).await;
    tokio::time::sleep(refresh).await;
    let (status, _, _) = stats(&app, Some(&tag)).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    snapshots.sweep(&api(&pool)).await.unwrap();
    computed(&snapshots, 2).await;
}

/// M18: concurrent first reads compute a key once; concurrent reads and a sweep of a due
/// key recompute it once.
#[kartenn_testing::pg_test]
async fn one_computation_per_key_at_a_time(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let reader = person(&pool, &READ_ALL, ("global", None)).await;
    let refresh = Duration::from_millis(300);
    let snapshots = snapshots(refresh);
    let changed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = changed.clone();
    snapshots.on_changed(move || {
        counted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    });
    let app = app_with(&pool, reader, &snapshots);
    let firsts = join_all((0..8).map(|_| stats(&app, None))).await;
    assert!(
        firsts
            .iter()
            .all(|(status, tag, _)| *status == StatusCode::OK && *tag == firsts[0].1)
    );
    assert_eq!(snapshots.computations(), 2);

    apply(&pool, vec![prefix_in(lab, "10.0.0.0/8")]).await;
    tokio::time::sleep(refresh).await;
    let api = api(&pool);
    let (_, swept) = tokio::join!(
        join_all((0..8).map(|_| stats(&app, None))),
        snapshots.sweep(&api)
    );
    swept.unwrap();
    computed(&snapshots, 4).await;
    assert_eq!(
        changed.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "the part of the rights changed, not its own"
    );
    let (_, _, after) = stats(&app, None).await;
    assert_eq!(after["objects"]["core.prefix"], 1, "{after}");
}

/// M18: a sweep recomputes the due keys read lately, not the others; a key unread for
/// long is forgotten and its next read computes at once, as one pushed out by the bound
/// on the keys.
#[kartenn_testing::pg_test]
async fn the_sweep_keeps_the_keys_read_lately(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let reader = person(&pool, &READ_ALL, ("global", None)).await;
    let api = api(&pool);
    let refresh = Duration::from_millis(200);
    let lately = snapshots(refresh);
    let idle = Snapshots::new(Freshness {
        refresh,
        active: Duration::from_millis(100),
        ..Freshness::default()
    });
    stats(&app_with(&pool, reader.clone(), &lately), None).await;
    stats(&app_with(&pool, reader.clone(), &idle), None).await;
    apply(&pool, vec![prefix_in(lab, "10.0.0.0/8")]).await;
    tokio::time::sleep(refresh).await;
    lately.sweep(&api).await.unwrap();
    idle.sweep(&api).await.unwrap();
    assert_eq!(lately.computations(), 4, "recomputed by the sweep");
    assert_eq!(idle.computations(), 2, "not read lately");
    let (_, _, served) = stats(&app_with(&pool, reader.clone(), &lately), None).await;
    assert_eq!(served["objects"]["core.prefix"], 1, "{served}");
    assert_eq!(lately.computations(), 4);

    let forgetting = Snapshots::new(Freshness {
        forget: Duration::from_millis(100),
        ..Freshness::default()
    });
    let app = app_with(&pool, reader.clone(), &forgetting);
    stats(&app, None).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    forgetting.sweep(&api).await.unwrap();
    assert_eq!(forgetting.computations(), 2);
    stats(&app, None).await;
    assert_eq!(forgetting.computations(), 4, "forgotten, computed again");

    let one = Snapshots::new(Freshness {
        keys: 1,
        ..Freshness::default()
    });
    let other = person(&pool, &["core.domain.read"], ("global", None)).await;
    stats(&app_with(&pool, reader.clone(), &one), None).await;
    stats(&app_with(&pool, other, &one), None).await;
    stats(&app_with(&pool, reader, &one), None).await;
    assert_eq!(one.computations(), 6, "the least recently read pushed out");
}
