//! `GET /v1/stats` through the router (spec 024, task 7; REQ-024-14 to 16): each counter
//! under the read rights of the caller, a kind it reads nowhere absent, the changesets
//! only to a reader of the changesets, its own pending changesets and those it may apply;
//! the `ETag` and the cache kept by the revision of the outbox.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::objects::Nature;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rate, Rates};
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
    sqlx::query(
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'identity', $1, $2, $3, $4,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
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

fn app(pool: &PgPool, who: Principal) -> Router {
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: Arc::default(),
    };
    kartenn_rest::router(
        kartenn_rest::v1(api),
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
        apply_own: true,
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

/// REQ-024-16: a strong `ETag` answers 304; a changeset moves the revision of the outbox,
/// which the cache keys on: the next read counts it at once.
#[kartenn_testing::pg_test]
async fn the_etag_and_the_cache_follow_the_outbox(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let reader = person(&pool, &READ_ALL, ("global", None)).await;
    let app = app(&pool, reader);
    let (status, tag, first) = stats(&app, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(tag.starts_with('"'), "strong: {tag}");
    let (status, again, body) = stats(&app, Some(&tag)).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert_eq!(again, tag);
    assert_eq!(body, Value::Null);
    assert_eq!(first["objects"]["core.prefix"], 0);
    apply(&pool, vec![prefix_in(lab, "10.0.0.0/8")]).await;
    let (status, moved, after) = stats(&app, Some(&tag)).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(moved, tag);
    assert_eq!(after["objects"]["core.prefix"], 1, "{after}");
}
