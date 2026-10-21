//! Imports deposited by chunks (spec 006, REQ-006-17 to 19; task 006-21): a million
//! elements in a hundred chunks, one resent; a different chunk under a number already
//! received, a chunk too large; the import is its author's; start, resume and cancel
//! guarded by the state.

use crate::common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::objects::AddressUsage;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rate, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::port::ImportItem;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{Operation, Ref};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn person(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'connector') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

fn app_as(pool: &PgPool, identity: Uuid, rates: Rates) -> Router {
    app_with(pool, identity, rates, Settings::default())
}

fn app_with(pool: &PgPool, identity: Uuid, rates: Rates, settings: Settings) -> Router {
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
        settings,
        Arc::new(Quotas::new(rates)),
        Arc::new(common::As(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    )
}

/// Rates that never refuse the deposit of a test.
fn generous() -> Rates {
    let rate = Rate::new(1_000_000, 1_000_000).unwrap();
    Rates {
        read: rate,
        write: rate,
        anonymous: rate,
    }
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

fn post(uri: &str) -> Request<Body> {
    Request::post(uri).body(Body::empty()).unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

/// Element `i` of an import: the domain first, then addresses of it.
fn element(i: u32) -> ImportItem {
    let operation = if i == 0 {
        Operation::CreateDomain {
            local: Some(0),
            slug: "lab".into(),
        }
    } else {
        let [_, a, b, c] = i.to_be_bytes();
        Operation::CreateAddress {
            local: Some(i),
            domain: Ref::Local(0),
            address: format!("10.{a}.{b}.{c}"),
            usage: AddressUsage::Host,
        }
    };
    ImportItem {
        external_id: Some(format!("e-{i}")),
        operation,
    }
}

fn chunk(elements: std::ops::Range<u32>) -> Value {
    json!({ "elements": elements.map(element).collect::<Vec<_>>() })
}

/// Opens an import of `app`; its path.
async fn open(app: &Router) -> String {
    let response = send(
        app,
        with_body("POST", "/v1/imports", &json!({"source": "legacy"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let body = json_of(response).await;
    assert_eq!(body["state"], "uploading");
    assert_eq!(body["links"]["self"], location.as_str());
    location
}

#[kartenn_testing::pg_test]
async fn a_million_elements_in_a_hundred_chunks(pool: PgPool) {
    // `start` sorts the million elements into batches inside the request (design 006,
    // « Imports et tâches de fond ») : about 11 s on a quiet machine, past the 30 s delay
    // of the transport layer once the whole suite loads the machine, which then answers
    // 503. The delay is not what this test checks (see the test below), hence a long one.
    let settings = Settings {
        request_timeout: Duration::from_secs(300),
        ..Settings::default()
    };
    let app = app_with(&pool, person(&pool).await, generous(), settings);
    let import = open(&app).await;
    for n in 0..100_u32 {
        let body = chunk(n * 10_000..(n + 1) * 10_000);
        let uri = format!("{import}/chunks/{n}");
        let response = send(&app, with_body("PUT", &uri, &body)).await;
        assert_eq!(response.status(), StatusCode::CREATED, "{n}");
        if n == 42 {
            // Resent identical: ignored (REQ-006-17).
            let again = send(&app, with_body("PUT", &uri, &body)).await;
            assert_eq!(again.status(), StatusCode::OK);
        }
    }
    // Different under a number already received → 409; too large → 413.
    let other = send(
        &app,
        with_body("PUT", &format!("{import}/chunks/7"), &chunk(0..3)),
    )
    .await;
    assert_eq!(other.status(), StatusCode::CONFLICT);
    assert_eq!(json_of(other).await["type"], "urn:kartenn:problem:conflict");
    let large = send(
        &app,
        with_body("PUT", &format!("{import}/chunks/100"), &chunk(0..10_001)),
    )
    .await;
    assert_eq!(large.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let started = send(&app, post(&format!("{import}/start"))).await;
    assert_eq!(started.status(), StatusCode::ACCEPTED);
    assert_eq!(started.headers()[header::LOCATION], import.as_str());
    let shown = send(&app, get(&import)).await;
    assert_eq!(shown.status(), StatusCode::OK);
    assert_eq!(shown.headers()[header::RETRY_AFTER], "5");
    let body = json_of(shown).await;
    assert_eq!(body["state"], "ready");
    assert_eq!(body["chunks"], 100);
    assert_eq!(body["elements"], 1_000_000);
    // A creation counts two operations: 5 000 elements by batch of 10 000 operations.
    assert_eq!(body["batches"], json!({"applied": 0, "remaining": 200}));
    let elements: i64 =
        sqlx::query_scalar("SELECT sum(jsonb_array_length(operations))::bigint FROM import_batch")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(elements, 1_000_000);
    let due: bool = sqlx::query_scalar("SELECT due_at <= now() FROM job WHERE name = $1")
        .bind(format!(
            "import:{}",
            import.trim_start_matches("/v1/imports/")
        ))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(due);
}

#[kartenn_testing::pg_test]
async fn an_import_is_its_authors(pool: PgPool) {
    let app = app_as(&pool, person(&pool).await, generous());
    let import = open(&app).await;
    let outsider = app_as(&pool, person(&pool).await, generous());
    for request in [
        get(&import),
        with_body("PUT", &format!("{import}/chunks/0"), &chunk(0..2)),
        post(&format!("{import}/start")),
        post(&format!("{import}/resume")),
        post(&format!("{import}/cancel")),
    ] {
        let response = send(&outsider, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let unknown = send(&app, get(&format!("/v1/imports/{}", Uuid::now_v7()))).await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    let shown = json_of(send(&app, get(&import)).await).await;
    assert_eq!(shown["chunks"], 0);
}

#[kartenn_testing::pg_test]
async fn the_imports_of_the_caller_list_the_most_recent_first(pool: PgPool) {
    let app = app_as(&pool, person(&pool).await, generous());
    let outsider = app_as(&pool, person(&pool).await, generous());
    let theirs = open(&outsider).await;
    let mut mine = Vec::new();
    for _ in 0..3 {
        mine.push(open(&app).await);
    }
    let selves = |body: &Value| -> Vec<String> {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["links"]["self"].as_str().unwrap().to_owned())
            .collect()
    };

    // The most recent first, by pages; the import of another is absent.
    let body = json_of(send(&app, get("/v1/imports?page_size=2")).await).await;
    assert_eq!(selves(&body), [mine[2].clone(), mine[1].clone()]);
    assert_eq!(body["items"][0]["state"], "uploading");
    let next = body["links"]["next"].as_str().unwrap().to_owned();
    assert!(next.starts_with("/v1/imports?cursor="));
    let body = json_of(send(&app, get(&next)).await).await;
    assert_eq!(selves(&body), [mine[0].clone()]);
    assert_eq!(body["links"]["next"], Value::Null);
    let body = json_of(send(&outsider, get("/v1/imports")).await).await;
    assert_eq!(selves(&body), [theirs]);

    // Neither a filter nor a date.
    for uri in [
        "/v1/imports?state=done",
        "/v1/imports?valid_at=2020-01-01T00:00:00Z",
    ] {
        assert_eq!(
            send(&app, get(uri)).await.status(),
            StatusCode::BAD_REQUEST,
            "{uri}"
        );
    }
}

#[kartenn_testing::pg_test]
async fn the_state_guards_the_actions(pool: PgPool) {
    let author = person(&pool).await;
    let app = app_as(&pool, author, generous());
    let import = open(&app).await;
    // An address whose domain no element defines: the import does not sort, 422, and
    // stays open.
    let stray = json!({"elements": [element(1)]});
    let response = send(
        &app,
        with_body("PUT", &format!("{import}/chunks/3"), &stray),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let refused = send(&app, post(&format!("{import}/start"))).await;
    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        json_of(refused).await["type"],
        "urn:kartenn:problem:invalid-import"
    );
    assert_eq!(
        json_of(send(&app, get(&import)).await).await["state"],
        "uploading"
    );
    // Chunks read by increasing number, gaps allowed: the domain comes in chunk 0.
    let domain = json!({"elements": [element(0)]});
    let response = send(
        &app,
        with_body("PUT", &format!("{import}/chunks/0"), &domain),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    // Only a paused import resumes.
    let early = send(&app, post(&format!("{import}/resume"))).await;
    assert_eq!(early.status(), StatusCode::CONFLICT);
    assert_eq!(
        send(&app, post(&format!("{import}/start"))).await.status(),
        StatusCode::ACCEPTED
    );
    // Started: no more chunks, no second start.
    let late = send(
        &app,
        with_body("PUT", &format!("{import}/chunks/4"), &domain),
    )
    .await;
    assert_eq!(late.status(), StatusCode::CONFLICT);
    let twice = send(&app, post(&format!("{import}/start"))).await;
    assert_eq!(twice.status(), StatusCode::CONFLICT);

    // Paused by its run (here by hand), it resumes and its task is due again.
    let id: Uuid = import.trim_start_matches("/v1/imports/").parse().unwrap();
    sqlx::query("UPDATE import SET state = 'paused' WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM job WHERE name = $1")
        .bind(format!("import:{id}"))
        .execute(&pool)
        .await
        .unwrap();
    let shown = send(&app, get(&import)).await;
    assert!(shown.headers().get(header::RETRY_AFTER).is_none());
    let resumed = send(&app, post(&format!("{import}/resume"))).await;
    assert_eq!(resumed.status(), StatusCode::ACCEPTED);
    assert_eq!(json_of(resumed).await["state"], "ready");
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM job WHERE name = $1")
        .bind(format!("import:{id}"))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1);

    // Cancelled once; then over.
    let cancelled = send(&app, post(&format!("{import}/cancel"))).await;
    assert_eq!(cancelled.status(), StatusCode::OK);
    assert_eq!(json_of(cancelled).await["state"], "cancelled");
    let again = send(&app, post(&format!("{import}/cancel"))).await;
    assert_eq!(again.status(), StatusCode::CONFLICT);
}

#[kartenn_testing::pg_test]
async fn a_chunk_weighs_its_thousands_on_the_quota(pool: PgPool) {
    // 20 writes at once, as by default; a chunk of 10 000 elements weighs 10. The bucket
    // refills at 1/s rather than 10/s, so that a chunk slowed by a loaded machine does not
    // earn back the weight of the next one.
    let rates = Rates {
        write: kartenn_rest::quota::Rate::new(1, 20).unwrap(),
        ..Rates::default()
    };
    let app = app_as(&pool, person(&pool).await, rates);
    let import = open(&app).await;
    let mut statuses = Vec::new();
    for n in 0..3 {
        let uri = format!("{import}/chunks/{n}");
        let body = chunk(n * 10_000..(n + 1) * 10_000);
        statuses.push(send(&app, with_body("PUT", &uri, &body)).await.status());
    }
    assert_eq!(statuses[0], StatusCode::CREATED);
    assert_eq!(statuses[2], StatusCode::TOO_MANY_REQUESTS, "{statuses:?}");
}

/// A `start` cut by the delay of the transport layer answers 503 and changes nothing:
/// its transaction is dropped, the import is still `uploading`, and a new `start`
/// succeeds.
#[kartenn_testing::pg_test]
async fn a_start_cut_by_the_delay_leaves_the_import_uploading(pool: PgPool) {
    let author = person(&pool).await;
    let app = app_as(&pool, author, generous());
    let hasty = app_with(
        &pool,
        author,
        generous(),
        Settings {
            request_timeout: Duration::from_millis(1),
            ..Settings::default()
        },
    );
    let import = open(&app).await;
    for n in 0..3_u32 {
        let body = chunk(n * 10_000..(n + 1) * 10_000);
        let response = send(
            &app,
            with_body("PUT", &format!("{import}/chunks/{n}"), &body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "{n}");
    }
    let cut = send(&hasty, post(&format!("{import}/start"))).await;
    assert_eq!(cut.status(), StatusCode::SERVICE_UNAVAILABLE);
    let shown = json_of(send(&app, get(&import)).await).await;
    assert_eq!(shown["state"], "uploading");
    let started = send(&app, post(&format!("{import}/start"))).await;
    assert_eq!(started.status(), StatusCode::ACCEPTED);
}
