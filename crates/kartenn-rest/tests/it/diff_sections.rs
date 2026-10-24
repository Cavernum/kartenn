//! Sections of the differential through the router (spec 008, REQ-008-26; task 008-19):
//! in the answer of a simulation, then by pages under
//! `GET /v1/changesets/{id}/diff?section=`.

use crate::common;

use crate::support::section_modules as sections;

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

/// A person granted `entries` everywhere.
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
    sqlx::query(
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'identity', $1, $2, 'global', NULL,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(identity)
    .bind(role)
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
        outputs: Arc::from([]),
        modules: sections::registry(),
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

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(common::bearer(common::applied(request)))
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn post(uri: &str, body: &Value) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

#[kartenn_testing::pg_test]
async fn a_section_is_served_by_pages(pool: PgPool) {
    let identity = person(
        &pool,
        &[
            "core.domain.edit",
            "core.domain.apply_own",
            "core.changeset.read",
        ],
    )
    .await;
    let app = app_as(&pool, identity);
    let operations: Vec<Value> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|slug| json!({"op": "create_domain", "slug": slug}))
        .collect();

    // The simulation answers the sections, in the order of the dependencies.
    let (status, simulated) = send(
        &app,
        post(
            "/v1/changesets?simulate=true",
            &json!({"motive": "m", "operations": operations}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{simulated}");
    assert_eq!(simulated["sections"][0]["name"], "alpha.objects");
    assert_eq!(simulated["sections"][0]["count"], 5);
    assert_eq!(simulated["sections"][1]["name"], "beta.echo");
    assert_eq!(
        simulated["sections"][1]["rows"][0]["after"],
        json!({"alpha_rows": 5})
    );

    let (status, applied) = send(
        &app,
        post(
            "/v1/changesets",
            &json!({"motive": "m", "operations": operations}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{applied}");
    let changeset = applied["changeset"]["id"].as_str().unwrap().to_owned();

    // The differential counts the sections.
    let (status, diff) = send(&app, get(&format!("/v1/changesets/{changeset}/diff"))).await;
    assert_eq!(status, StatusCode::OK, "{diff}");
    assert_eq!(
        diff["sections"],
        json!([{"name": "alpha.objects", "count": 5}, {"name": "beta.echo", "count": 1}])
    );

    // A section by pages of two, following the links.
    let mut uri = format!("/v1/changesets/{changeset}/diff?section=alpha.objects&page_size=2");
    let mut seen = Vec::new();
    loop {
        let (status, page) = send(&app, get(&uri)).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(page["section"], "alpha.objects");
        let rows = page["rows"].as_array().unwrap();
        assert!(rows.len() <= 2);
        seen.extend(rows.iter().map(|r| r["object"].clone()));
        assert!(rows.iter().all(|r| r["kind"] == "domain"));
        match page["links"]["next"].as_str() {
            Some(next) => uri = next.to_owned(),
            None => break,
        }
    }
    assert_eq!(seen.len(), 5);

    // Property and section exclude each other.
    let (status, _) = send(
        &app,
        get(&format!(
            "/v1/changesets/{changeset}/diff?section=alpha.objects&property=core.owner"
        )),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
