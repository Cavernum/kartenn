//! `?simulate=true` on every write (spec 024, REQ-024-17, M6; task 024-8): a write that
//! submits a changeset answers 200 with the simulation of `POST /v1/changesets`
//! (`valid`, `violations`, `diff`, `sample`, `cascaded`) and writes nothing, no event
//! either; any other write answers 400 `simulate-unsupported` before it acts.
//!
//! The generic test walks every write of the contract of the build (`x-kartenn.simulate`
//! says which simulate), on objects that exist where the path names one.

use crate::common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::openapi::CLIENT_EXTENSION;
use kartenn_rest::quota::{Quotas, Rate, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// The members of a simulation (`ChangesetSimulated`).
const SIMULATION: [&str; 5] = ["valid", "violations", "diff", "sample", "cascaded"];

/// A person granted everything everywhere.
async fn admin(pool: &PgPool) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, $2, '{\"fr\": \"r\"}', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .bind(format!("r_{}", role.simple()))
    .execute(pool)
    .await
    .unwrap();
    for statement in [
        "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
         VALUES ($1, '*', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        "WITH g AS (
            INSERT INTO rbac_group (id, key, labels, valid_period, changeset_id)
            VALUES (gen_random_uuid(), 't-' || replace(gen_random_uuid()::text, '-', ''),
                    '{\"fr\": \"t\"}', tstzrange(now() - interval '1 day', NULL),
                    gen_random_uuid())
            RETURNING id),
         m AS (
            INSERT INTO group_member (id, group_id, member_kind, identity_id, valid_period,
                                      changeset_id)
            SELECT gen_random_uuid(), g.id, 'identity', $2,
                   tstzrange(now() - interval '1 day', NULL), gen_random_uuid() FROM g)
         INSERT INTO binding (id, group_id, role_id, scope_kind, valid_period, changeset_id)
         SELECT gen_random_uuid(), g.id, $1, 'global',
                tstzrange(now() - interval '1 day', NULL), gen_random_uuid() FROM g",
    ] {
        sqlx::query(statement)
            .bind(role)
            .bind(identity)
            .execute(pool)
            .await
            .unwrap();
    }
    identity
}

/// The router of the build, every module included.
async fn app(pool: &PgPool) -> Router {
    let registry = kartenn_server::modules::registry().unwrap();
    kartenn_store_pg::modules::migrate(pool, &registry)
        .await
        .unwrap();
    let identity = admin(pool).await;
    let rate = Rate::new(1_000_000, 1_000_000).unwrap();
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: registry,
    };
    kartenn_rest::router(
        kartenn_rest::v1_with(api, kartenn_server::modules::routers()),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates {
            read: rate,
            write: rate,
            anonymous: rate,
        })),
        Arc::new(common::As(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    )
}

/// The status, `ETag` and body of `request`.
async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Option<String>, Value) {
    let response = app.clone().oneshot(common::bearer(request)).await.unwrap();
    let status = response.status();
    let etag = response
        .headers()
        .get(header::ETAG)
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        etag,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

/// A small world, applied: a domain, a prefix, an entity, a site.
async fn world(app: &Router) {
    let body = json!({"motive": "a lab", "operations": [
        {"op": "create_domain", "local": 1, "slug": "lab"},
        {"op": "create_prefix", "local": 2, "domain": {"local": 1},
         "prefix": "10.0.0.0/24", "nature": "network"},
        {"op": "create_entity", "local": 3, "slug": "team"},
    ]});
    let (status, _, answer) = send(
        app,
        Request::post("/v1/changesets?approval=self")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
}

/// What a simulation must leave as it was: the changesets, the events to deliver, the
/// objects as a read shows them.
async fn state(pool: &PgPool, app: &Router) -> (i64, i64, Value, Value) {
    let (changesets, events): (i64, i64) =
        sqlx::query_as("SELECT (SELECT count(*) FROM changeset), (SELECT count(*) FROM outbox)")
            .fetch_one(pool)
            .await
            .unwrap();
    let (_, _, prefixes) = send(app, get("/v1/prefixes")).await;
    let (_, _, entities) = send(app, get("/v1/entities")).await;
    (
        changesets,
        events,
        prefixes["items"].clone(),
        entities["items"].clone(),
    )
}

/// An object of the collection of `path` (`/v1/<collection>/{id}…`), and its `ETag`.
async fn existing(app: &Router, path: &str) -> Option<(String, String)> {
    let collection = path.strip_prefix("/v1/")?.split('/').next()?;
    let (status, _, listed) = send(app, get(&format!("/v1/{collection}"))).await;
    if status != StatusCode::OK {
        return None;
    }
    let id = listed["items"].get(0)?["id"].as_str()?.to_owned();
    let (_, etag, _) = send(app, get(&format!("/v1/{collection}/{id}"))).await;
    Some((id, etag?))
}

/// The request of a write of the contract with `?simulate=true`: path parameters filled
/// with an existing object when there is one, a body with a motive in the media type it
/// takes, the `ETag` of the object.
async fn simulated(app: &Router, method: &str, path: &str, operation: &Value) -> Request<Body> {
    let found = existing(app, path).await;
    let mut uri = path.to_owned();
    while let Some(start) = uri.find('{') {
        let end = uri[start..].find('}').unwrap() + start;
        let value = match (&uri[start + 1..end], &found) {
            ("id", Some((id, _))) => id.clone(),
            ("id" | "address" | "secret", _) => Uuid::now_v7().to_string(),
            _ => "x".to_owned(),
        };
        uri.replace_range(start..=end, &value);
    }
    let media = operation["requestBody"]["content"]
        .as_object()
        .and_then(|content| content.keys().next().cloned())
        .unwrap_or_else(|| "application/json".to_owned());
    let mut request = Request::builder()
        .method(method.to_ascii_uppercase().as_str())
        .uri(format!("{uri}?simulate=true"))
        .header(header::CONTENT_TYPE, media)
        .header(
            header::IF_MATCH,
            found.map_or("\"x\"".to_owned(), |(_, e)| e),
        );
    if method == "post" || method == "put" {
        request = request.header("idempotency-key", Uuid::now_v7().to_string());
    }
    request
        .body(Body::from(json!({"motive": "simulated"}).to_string()))
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn every_write_simulates_or_says_it_cannot(pool: PgPool) {
    let app = app(&pool).await;
    world(&app).await;
    let registry = kartenn_server::modules::registry().unwrap();
    let document = kartenn_rest::openapi::json_with(&registry, kartenn_server::modules::routers());
    let before = state(&pool, &app).await;
    let mut wrong = Vec::new();
    let mut answered: BTreeMap<String, u16> = BTreeMap::new();
    for (path, item) in document["paths"].as_object().unwrap() {
        for method in ["post", "put", "patch", "delete"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let simulates = &operation[CLIENT_EXTENSION]["simulate"];
            let name = format!("{} {path}", method.to_ascii_uppercase());
            let (status, _, body) =
                send(&app, simulated(&app, method, path, operation).await).await;
            answered.insert(name.clone(), status.as_u16());
            let unsupported = body["type"] == "urn:kartenn:problem:simulate-unsupported";
            match simulates {
                Value::Bool(false) => {
                    if !(status == StatusCode::BAD_REQUEST && unsupported) {
                        wrong.push(format!(
                            "{name}: {status} {body} (not simulate-unsupported)"
                        ));
                    }
                }
                Value::Bool(true) => {
                    let shaped = SIMULATION.iter().all(|member| body.get(member).is_some());
                    let refused = status.is_client_error() && !unsupported;
                    if !((status == StatusCode::OK && shaped) || refused) {
                        wrong.push(format!("{name}: {status} {body}"));
                    }
                }
                other => wrong.push(format!("{name}: x-kartenn.simulate is {other}")),
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "writes off the contract:\n{}",
        wrong.join("\n")
    );
    assert_eq!(state(&pool, &app).await, before, "a simulation wrote");
    // The objects of the world reach the pipeline: their simulation is answered.
    for name in [
        "POST /v1/vlan-groups",
        "POST /v1/changesets/{id}/undo",
        "PATCH /v1/domains/{id}",
        "DELETE /v1/domains/{id}",
        "PATCH /v1/prefixes/{id}",
        "DELETE /v1/prefixes/{id}",
        "DELETE /v1/entities/{id}",
    ] {
        assert_eq!(answered.get(name), Some(&200), "{name}: {answered:?}");
    }
}

#[kartenn_testing::pg_test]
async fn a_write_simulated_answers_what_applying_would_do(pool: PgPool) {
    let app = app(&pool).await;
    world(&app).await;
    let before = state(&pool, &app).await;
    let last: i64 = sqlx::query_scalar("SELECT coalesce(max(seq), 0) FROM audit.event")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, _, report) = send(
        &app,
        Request::post("/v1/domains?simulate=true&approval=self")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"motive": "a second lab", "slug": "lab2"}).to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["valid"], true, "{report}");
    assert_eq!(report["violations"], json!([]));
    // Spec 024, M6: one audit line, a read, never that of a write.
    let lines: Vec<(String, Option<String>, Option<Uuid>)> = sqlx::query_as(
        "SELECT name, target_kind, changeset_id FROM audit.event WHERE seq > $1 ORDER BY seq",
    )
    .bind(last)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        lines,
        [(
            "data.objects.read".to_owned(),
            Some("changeset".to_owned()),
            None
        )]
    );
    // The answer of the same changeset simulated.
    let (_, _, same) = send(
        &app,
        Request::post("/v1/changesets?simulate=true")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"motive": "a second lab", "operations": [
                    {"op": "create_domain", "local": null, "slug": "lab2"}]})
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(report, same);
    // A refusal is reported, not answered 422.
    let (status, _, refused) = send(
        &app,
        Request::post("/v1/domains?simulate=true")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"motive": "again", "slug": "lab"}).to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{refused}");
    assert_eq!(refused["valid"], false, "{refused}");
    assert_eq!(state(&pool, &app).await, before);

    // A decision on a changeset submits none: refused before it acts.
    let (status, _, problem) = send(
        &app,
        Request::post(format!(
            "/v1/changesets/{}/withdrawal?simulate=true",
            Uuid::now_v7()
        ))
        .body(Body::empty())
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem["type"], "urn:kartenn:problem:simulate-unsupported");
    // `simulate` is a flag given once.
    let (status, _, _) = send(
        &app,
        Request::post("/v1/domains?simulate=yes")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"motive": "m", "slug": "x"}).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
