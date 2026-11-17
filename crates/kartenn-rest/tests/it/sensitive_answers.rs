//! The answers of a write mask the sensitive values as the differential read does
//! (REQ-005-21; task 006-43): an author who may edit a sensitive property without reading
//! it reads neither the value it replaces nor the one it writes, in a simulation (`diff`
//! and `sample`), an applied changeset, a pending one or the replay of either by its
//! idempotency key; with `prop.<key>.read`, the value is served and audited
//! (`data.sensitive.read`).

use crate::common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// The value the administrator assigns, which the writes replace.
const SECRET: &str = "s3cret";
/// The value the writes assign.
const WRITTEN: &str = "n3w";

/// A person granted `entries` everywhere, by a role of their own.
async fn person(pool: &PgPool, entries: &[&str]) -> Uuid {
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

fn app_as(pool: &PgPool, identity: Uuid) -> Router {
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: std::sync::Arc::from([]),
        modules: std::sync::Arc::default(),
    };
    kartenn_rest::router(
        kartenn_rest::v1(api),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates::default())),
        Arc::new(common::As(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    )
}

/// `POST uri` with `operations`, under the idempotency key `key` when given; the status and
/// body.
async fn post(
    app: &Router,
    uri: &str,
    operations: &Value,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"motive": "m", "operations": operations}).to_string(),
        ))
        .unwrap();
    if let Some(key) = key {
        request
            .headers_mut()
            .insert("idempotency-key", key.parse().unwrap());
    }
    let response = app.clone().oneshot(common::bearer(request)).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// The events `data.sensitive.read` journaled.
async fn disclosures(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'data.sensitive.read'")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A prefix carrying the sensitive property `sec.code` at [`SECRET`], written by an
/// administrator; the assignment of [`WRITTEN`] to it, and the routers of an author who
/// may edit `sec.code` without reading it (`blind`) and of one who may read it too
/// (`sighted`).
struct Scene {
    pool: PgPool,
    assign: Value,
    blind: Router,
    sighted: Router,
}

async fn scene(pool: PgPool) -> Scene {
    let admin = app_as(&pool, person(&pool, &["*"]).await);
    let property: Uuid = sqlx::query_scalar(
        "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers,
                                           type_params, sensitive, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'sec.code', 'text', 'single', ARRAY['prefix'],
                 '{\"min_len\": 1, \"max_len\": 64}', true,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, body) = post(
        &admin,
        "/v1/changesets?approval=self",
        &json!([{"op": "create_domain", "slug": "lab"}]),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let domain: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = 'lab'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, body) = post(
        &admin,
        "/v1/changesets?approval=self",
        &json!([{"op": "create_prefix", "local": 1, "domain": {"id": domain},
                 "prefix": "192.0.2.0/24", "nature": "network"}]),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let prefix: Uuid = sqlx::query_scalar("SELECT id FROM prefix")
        .fetch_one(&pool)
        .await
        .unwrap();
    let assign = |value: &str| {
        json!([{"op": "assign_property", "object": {"id": prefix}, "kind": "prefix",
                "property": {"id": property}, "values": [value]}])
    };
    let (status, body) = post(
        &admin,
        "/v1/changesets?approval=self",
        &assign(SECRET),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let author = [
        "prop.sec.code.edit",
        "prop.sec.code.approval.self",
        "core.prefix.read",
    ];
    let blind = app_as(&pool, person(&pool, &author).await);
    let sighted = app_as(
        &pool,
        person(&pool, &[&author[..], &["prop.sec.code.read"]].concat()).await,
    );
    Scene {
        pool,
        assign: assign(WRITTEN),
        blind,
        sighted,
    }
}

/// The JSON text of `value`, as the counted differential and the sample carry it.
fn text(value: &str) -> Value {
    Value::String(Value::String(value.to_owned()).to_string())
}

/// `entry` masked: `before` and `after` `null`, `masked: true`.
fn assert_masked(entry: &Value) {
    assert_eq!(entry["property"], "sec.code", "{entry}");
    assert!(entry["before"].is_null(), "{entry}");
    assert!(entry["after"].is_null(), "{entry}");
    assert_eq!(entry["masked"], true, "{entry}");
}

/// `entry` shown: the value replaced and the one written.
fn assert_shown(entry: &Value) {
    assert_eq!(entry["property"], "sec.code", "{entry}");
    assert_eq!(entry["before"], text(SECRET), "{entry}");
    assert_eq!(entry["after"], text(WRITTEN), "{entry}");
    assert_ne!(entry["masked"], true, "{entry}");
}

/// The answer of a blind author names neither value and journals no disclosure.
async fn assert_blind(scene: &Scene, body: &Value, before: i64) {
    let served = body.to_string();
    assert!(!served.contains(SECRET), "{body}");
    assert!(!served.contains(WRITTEN), "{body}");
    assert_eq!(disclosures(&scene.pool).await, before);
}

#[kartenn_testing::pg_test]
async fn a_simulation_masks_a_sensitive_value_without_its_right(pool: PgPool) {
    let scene = scene(pool).await;
    let uri = "/v1/changesets?simulate=true";
    let before = disclosures(&scene.pool).await;
    let (status, body) = post(&scene.blind, uri, &scene.assign, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_masked(&body["diff"][0]);
    assert_masked(&body["sample"][0]);
    assert_blind(&scene, &body, before).await;

    let (status, body) = post(&scene.sighted, uri, &scene.assign, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_shown(&body["diff"][0]);
    assert_shown(&body["sample"][0]);
    assert!(disclosures(&scene.pool).await > before);
}

#[kartenn_testing::pg_test]
async fn an_applied_answer_and_its_replay_mask_a_sensitive_value_without_its_right(pool: PgPool) {
    let scene = scene(pool).await;
    let uri = "/v1/changesets?approval=self";
    let before = disclosures(&scene.pool).await;
    let (status, body) = post(&scene.blind, uri, &scene.assign, Some("blind-1")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_masked(&body["diff"][0]);
    assert_blind(&scene, &body, before).await;
    let (status, body) = post(&scene.blind, uri, &scene.assign, Some("blind-1")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_masked(&body["diff"][0]);
    assert_blind(&scene, &body, before).await;

    // The sighted author puts the secret back, then writes it over again.
    let restore = |assign: &Value| {
        let mut assign = assign.clone();
        assign[0]["values"] = json!([SECRET]);
        assign
    };
    let (status, body) = post(&scene.sighted, uri, &restore(&scene.assign), None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let before = disclosures(&scene.pool).await;
    let (status, body) = post(&scene.sighted, uri, &scene.assign, Some("sighted-1")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_shown(&body["diff"][0]);
    let first = disclosures(&scene.pool).await;
    assert!(first > before);
    let (status, body) = post(&scene.sighted, uri, &scene.assign, Some("sighted-1")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_shown(&body["diff"][0]);
    assert!(disclosures(&scene.pool).await > first);
}

#[kartenn_testing::pg_test]
async fn a_pending_answer_and_its_replay_mask_a_sensitive_value(pool: PgPool) {
    let scene = scene(pool).await;
    let uri = "/v1/changesets";
    let before = disclosures(&scene.pool).await;
    // A pending changeset journals no detail of its differential: an entry of a sensitive
    // property counts objects it cannot name, masked even with the right, as its read
    // (`GET /v1/changesets/{id}`) masks it.
    for (app, key) in [(&scene.blind, "pending-1"), (&scene.sighted, "pending-2")] {
        let (status, body) = post(app, uri, &scene.assign, Some(key)).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{body}");
        assert_masked(&body["diff"][0]);
        assert_blind(&scene, &body, before).await;
        let (status, body) = post(app, uri, &scene.assign, Some(key)).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{body}");
        assert_masked(&body["diff"][0]);
        assert_blind(&scene, &body, before).await;
    }
}
