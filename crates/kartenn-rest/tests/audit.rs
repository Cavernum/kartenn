//! Audit and operations through the router (spec 006, REQ-006-4; task 006-19; design
//! « Audit et exploitation »): the journal read by pages, by `QUERY` and exported, under
//! `core.audit.read`, a consultation refused and audited without it; a person
//! pseudonymised at once; the filter of an output changed, then back to the configuration.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::observability::Overrides;
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use kartenn_telemetry::capture::{FilterOverrides, Output};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// A person granted `entries` everywhere.
async fn person(pool: &PgPool, entries: &[&str]) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name, email) VALUES (gen_random_uuid(), 'p', 'p@example.org')
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, label, valid_period, changeset_id)
         VALUES ($1, $2, 'r', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
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
        outputs: Arc::from(["stdout".to_owned()]),
        modules: Arc::default(),
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

async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(common::bearer(request)).await.unwrap()
}

async fn json_of(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn with_body(method: &str, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

/// The audit events named `name`, oldest first: actor and details.
async fn recorded(pool: &PgPool, name: &str) -> Vec<(Option<Uuid>, Value)> {
    sqlx::query_as("SELECT actor_id, details FROM audit.event WHERE name = $1 ORDER BY seq")
        .bind(name)
        .fetch_all(pool)
        .await
        .unwrap()
}

/// Three refusals of `outsider`, audited `authz.access.denied`.
async fn refusals(app: &Router) {
    for _ in 0..3 {
        assert_eq!(
            send(app, get("/v1/audit-events")).await.status(),
            StatusCode::FORBIDDEN
        );
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_consultation_without_permission_is_refused_and_audited(pool: PgPool) {
    let outsider = person(&pool, &["core.rbac.read"]).await;
    let app = app_as(&pool, outsider);
    let refused = send(&app, get("/v1/audit-events")).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    let query = send(
        &app,
        with_body("QUERY", "/v1/audit-events", &json!({ "filters": [] })),
    )
    .await;
    assert_eq!(query.status(), StatusCode::FORBIDDEN);
    let export = send(
        &app,
        Request::get("/v1/audit-events")
            .header(header::ACCEPT, "application/x-ndjson")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(export.status(), StatusCode::FORBIDDEN);
    let denied = recorded(&pool, "authz.access.denied").await;
    assert_eq!(denied.len(), 3);
    assert!(
        denied
            .iter()
            .all(|(actor, details)| *actor == Some(outsider)
                && details["permission"] == "core.audit.read")
    );
    assert!(recorded(&pool, "audit.events.read").await.is_empty());
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_journal_reads_by_pages_and_by_query(pool: PgPool) {
    let outsider = person(&pool, &[]).await;
    refusals(&app_as(&pool, outsider)).await;
    let auditor = app_as(&pool, person(&pool, &["core.audit.read"]).await);

    let uri = format!("/v1/audit-events?name=authz.access.denied&actor={outsider}&page_size=2");
    let first = send(&auditor, get(&uri)).await;
    assert_eq!(first.status(), StatusCode::OK);
    let first = json_of(first).await;
    let items = first["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(
        items[0]["seq"].as_i64() > items[1]["seq"].as_i64(),
        "newest first"
    );
    assert!(
        items
            .iter()
            .all(|e| e["name"] == "authz.access.denied" && e["actor_id"] == json!(outsider))
    );
    let next = first["links"]["next"].as_str().unwrap();
    let second = json_of(send(&auditor, get(next)).await).await;
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert!(second["links"]["next"].is_null());

    // The same first page by `QUERY`.
    let by_query = send(
        &auditor,
        with_body(
            "QUERY",
            "/v1/audit-events",
            &json!({
                "filters": [
                    { "field": "name", "value": "authz.access.denied" },
                    { "field": "actor", "value": outsider }
                ],
                "page_size": 2
            }),
        ),
    )
    .await;
    assert_eq!(by_query.status(), StatusCode::OK);
    assert_eq!(json_of(by_query).await["items"], first["items"]);

    // Each page is audited, with its criteria and its count.
    let reads = recorded(&pool, "audit.events.read").await;
    let counts: Vec<i64> = reads
        .iter()
        .map(|(_, d)| d["count"].as_i64().unwrap())
        .collect();
    assert_eq!(counts, [2, 1, 2]);
    assert_eq!(reads[0].1["criteria"]["name"], "authz.access.denied");

    for bad in [
        "/v1/audit-events?valid_at=2026-01-01T00:00:00Z",
        "/v1/audit-events?from=2026-01-01T00:00:00",
        "/v1/audit-events?actor=me",
        "/v1/audit-events?name=a&name=b",
        "/v1/audit-events?other=1",
    ] {
        assert_eq!(
            send(&auditor, get(bad)).await.status(),
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_export_is_whole_and_audited_once(pool: PgPool) {
    let outsider = person(&pool, &[]).await;
    refusals(&app_as(&pool, outsider)).await;
    let auditor = app_as(&pool, person(&pool, &["core.audit.read"]).await);
    let ndjson = |uri: &str| {
        Request::get(uri)
            .header(header::ACCEPT, "application/x-ndjson")
            .body(Body::empty())
            .unwrap()
    };

    let export = send(
        &auditor,
        ndjson(&format!("/v1/audit-events?actor={outsider}")),
    )
    .await;
    assert_eq!(export.status(), StatusCode::OK);
    assert_eq!(
        export.headers()[header::CONTENT_TYPE],
        "application/x-ndjson"
    );
    let bytes = export.into_body().collect().await.unwrap().to_bytes();
    let lines: Vec<Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|e| e["actor_id"] == json!(outsider)));

    // The whole selection, its own audit left out.
    let all = send(&auditor, ndjson("/v1/audit-events")).await;
    let all = all.into_body().collect().await.unwrap().to_bytes();
    let names: Vec<Value> = std::str::from_utf8(&all)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap()["name"].clone())
        .collect();
    assert_eq!(
        names,
        [
            json!("audit.events.read"),
            json!("authz.access.denied"),
            json!("authz.access.denied"),
            json!("authz.access.denied"),
        ]
    );

    let reads = recorded(&pool, "audit.events.read").await;
    assert_eq!(reads.len(), 2, "one event per export");
    assert_eq!(reads[0].1["count"], 3);
    assert_eq!(reads[0].1["criteria"]["export"], true);
    assert_eq!(reads[1].1["count"], 4);

    for bad in [
        "/v1/audit-events?page_size=10",
        "/v1/audit-events?as_of=2026-01-01T00:00:00Z",
    ] {
        assert_eq!(
            send(&auditor, ndjson(bad)).await.status(),
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_person_is_pseudonymised_at_once(pool: PgPool) {
    let target = person(&pool, &[]).await;
    let path = format!("/v1/identities/{target}/pseudonymisation");
    let motive = json!({ "motive": "right to erasure" });

    let outsider = app_as(&pool, person(&pool, &["core.rbac.read"]).await);
    assert_eq!(
        send(&outsider, with_body("POST", &path, &motive))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let denied = recorded(&pool, "authz.access.denied").await;
    assert_eq!(denied[0].1["permission"], "core.identity.pseudonymise");

    let officer = person(&pool, &["core.identity.pseudonymise"]).await;
    let app = app_as(&pool, officer);
    assert_eq!(
        send(&app, with_body("POST", &path, &json!({})))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let unknown = format!("/v1/identities/{}/pseudonymisation", Uuid::now_v7());
    assert_eq!(
        send(&app, with_body("POST", &unknown, &motive))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let account: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name, kind)
         VALUES (gen_random_uuid(), 'robot', 'service_account') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let robot = format!("/v1/identities/{account}/pseudonymisation");
    assert_eq!(
        send(&app, with_body("POST", &robot, &motive))
            .await
            .status(),
        StatusCode::CONFLICT
    );

    let done = send(&app, with_body("POST", &path, &motive)).await;
    assert_eq!(done.status(), StatusCode::OK);
    let done = json_of(done).await;
    assert_eq!(done["id"], json!(target));
    assert_eq!(
        done["name"],
        format!("personne #{}", &target.to_string()[..8])
    );
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM person WHERE id = $1")
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(email, None);

    // Again: nothing more done.
    assert_eq!(
        send(&app, with_body("POST", &path, &motive)).await.status(),
        StatusCode::OK
    );
    let events = recorded(&pool, "person.identity.pseudonymised").await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, Some(officer));
    assert_eq!(events[0].1["motive"], "right to erasure");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_log_filter_returns_to_the_configuration(pool: PgPool) {
    let put =
        |output: &str, body: Value| with_body("PUT", &format!("/v1/log-filters/{output}"), &body);
    let outsider = app_as(&pool, person(&pool, &[]).await);
    assert_eq!(
        send(&outsider, put("stdout", json!({ "filter": "debug" })))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let denied = recorded(&pool, "authz.access.denied").await;
    assert_eq!(denied[0].1["permission"], "core.observability.admin");

    let admin = app_as(&pool, person(&pool, &["core.observability.admin"]).await);
    assert_eq!(
        send(&admin, put("syslog", json!({ "filter": "debug" })))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    for invalid in [
        json!({ "filter": "debug[" }),
        json!({ "filter": "debug", "ttl": 0 }),
        json!({ "filter": "debug", "ttl": 86_401 }),
        json!({ "filter": "debug", "ttl": -1 }),
    ] {
        let response = send(&admin, put("stdout", invalid.clone())).await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{invalid}"
        );
        assert!(
            json_of(response).await["type"]
                .as_str()
                .unwrap()
                .ends_with("invalid-override")
        );
    }

    // An instance, configured at `warn`, applies the change at its next read…
    let (output, _receiver) = Output::new("stdout", "warn", 8).unwrap();
    let mut filters = FilterOverrides::new([("stdout".to_owned(), "warn".to_owned())]);
    let store = PgStore::new(pool.clone());
    let refresh = async |filters: &mut FilterOverrides| {
        let pairs: Vec<(String, String)> = store
            .overrides()
            .await
            .unwrap()
            .into_iter()
            .map(|o| (o.output, o.filter))
            .collect();
        filters.apply(std::slice::from_ref(&output), &pairs)
    };
    let set = send(
        &admin,
        put("stdout", json!({ "filter": "debug", "ttl": 600 })),
    )
    .await;
    assert_eq!(set.status(), StatusCode::OK);
    let set = json_of(set).await;
    assert_eq!(set["output"], "stdout");
    assert_eq!(set["filter"], "debug");
    assert!(set["expires_at"].is_string());
    assert_eq!(refresh(&mut filters).await, ["stdout"]);
    assert!(refresh(&mut filters).await.is_empty(), "applied once");
    let changed = recorded(&pool, "sys.log_filter.changed").await;
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].1["filter"], "debug");

    // …and returns to its configuration at expiry.
    sqlx::query("UPDATE observability_override SET expires_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(refresh(&mut filters).await, ["stdout"]);
    assert!(refresh(&mut filters).await.is_empty());
}
