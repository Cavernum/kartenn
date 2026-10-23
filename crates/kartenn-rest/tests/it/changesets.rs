//! Changesets through the router (spec 006, REQ-006-11, 21, 22; task 006-14): applied →
//! 201 with its link, simulated → 200 without writing, refused → one 422 with every
//! violation and a pointer to its operation, and the order of the problems.
//! `Idempotency-Key` (REQ-006-15; task 006-15): replay, key in progress, other identity.

use crate::common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::changesets::REL_CHANGESET;
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

/// A person who reads domains and prefixes everywhere and may `verb` them: `write` or
/// only `propose`.
async fn writer(pool: &PgPool, verb: &str) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, 'writer', jsonb_build_object('fr', 'writer'), tstzrange(now() - interval '1 day', NULL),
                 gen_random_uuid())",
    )
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    for permission in [
        "core.domain.read".to_owned(),
        format!("core.domain.{verb}"),
        "core.prefix.read".to_owned(),
        format!("core.prefix.{verb}"),
    ] {
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

async fn app(pool: &PgPool) -> Router {
    app_of(pool, "write").await
}

async fn app_of(pool: &PgPool, verb: &str) -> Router {
    app_as(pool, writer(pool, verb).await)
}

/// Another person with the roles of `of`.
async fn colleague(pool: &PgPool, of: Uuid) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'q') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                              valid_period, changeset_id)
         SELECT gen_random_uuid(), 'identity', $1, role_id, scope_kind, scope_id,
                valid_period, changeset_id
         FROM binding WHERE subject_id = $2",
    )
    .bind(identity)
    .bind(of)
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
        // Without issuer: no confinement (005).
        Arc::new(common::As(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    )
}

fn submission(uri: &str, content_type: &str, body: impl Into<Body>) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, content_type)
        .body(body.into())
        .unwrap()
}

async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

async fn json_of(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn domains(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM addressing_domain")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn lab(prefixes: &[&str]) -> Value {
    let mut operations = vec![json!({"op": "create_domain", "local": 1, "slug": "lab"})];
    operations.extend(prefixes.iter().map(|p| {
        json!({"op": "create_prefix", "local": null, "domain": {"local": 1},
               "prefix": p, "nature": "network"})
    }));
    json!({"motive": "a lab", "operations": operations})
}

#[kartenn_testing::pg_test]
async fn an_applied_changeset_answers_201_with_its_link(pool: PgPool) {
    let app = app(&pool).await;
    let body = lab(&["10.0.0.0/24"]).to_string();

    // REQ-006-11: a simulation writes nothing and reports the validation.
    let simulated = send(
        &app,
        common::bearer(submission(
            "/v1/changesets?simulate=true",
            "application/json",
            body.clone(),
        )),
    )
    .await;
    assert_eq!(simulated.status(), StatusCode::OK);
    assert!(simulated.headers().get(header::LINK).is_none());
    let report = json_of(simulated).await;
    assert_eq!(report["valid"], true, "{report}");
    assert_eq!(report["violations"], json!([]));
    assert_eq!(domains(&pool).await, 0);

    let applied = send(
        &app,
        common::bearer(submission("/v1/changesets", "application/json", body)),
    )
    .await;
    assert_eq!(applied.status(), StatusCode::CREATED);
    let location = applied.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let link = applied.headers()[header::LINK].to_str().unwrap().to_owned();
    let created = json_of(applied).await;
    let id = created["changeset"]["id"].as_str().unwrap();
    assert_eq!(location, format!("/v1/changesets/{id}"));
    assert_eq!(link, format!("<{location}>; rel=\"{REL_CHANGESET}\""));
    assert_eq!(created["changeset"]["href"], location);
    assert_eq!(created["state"], "applied");
    assert_eq!(domains(&pool).await, 1);
}

#[kartenn_testing::pg_test]
async fn a_proposal_answers_202_with_its_link(pool: PgPool) {
    // REQ-004-41: an author who may only propose makes a pending changeset.
    let app = app_of(&pool, "propose").await;
    let pending = send(
        &app,
        common::bearer(submission(
            "/v1/changesets",
            "application/json",
            lab(&["10.0.0.0/24"]).to_string(),
        )),
    )
    .await;
    assert_eq!(pending.status(), StatusCode::ACCEPTED);
    assert!(pending.headers().get(header::LOCATION).is_none());
    let link = pending.headers()[header::LINK].to_str().unwrap().to_owned();
    let body = json_of(pending).await;
    assert_eq!(body["state"], "pending");
    assert_eq!(body["kind"], "proposal");
    assert_eq!(
        link,
        format!(
            "<{}>; rel=\"{REL_CHANGESET}\"",
            body["changeset"]["href"].as_str().unwrap()
        )
    );
    assert_eq!(domains(&pool).await, 0);
}

#[kartenn_testing::pg_test]
async fn valid_to_alone_starts_at_the_time_of_the_database(pool: PgPool) {
    let app = app(&pool).await;
    let mut body = lab(&[]);
    body["valid_to"] = json!("2999-01-01T00:00:00Z");
    let applied = send(
        &app,
        common::bearer(submission(
            "/v1/changesets",
            "application/json",
            body.to_string(),
        )),
    )
    .await;
    assert_eq!(applied.status(), StatusCode::CREATED);
    let (current, upper): (bool, String) = sqlx::query_as(
        "SELECT valid_period @> now(), to_char(upper(valid_period) AT TIME ZONE 'UTC', 'YYYY')
         FROM addressing_domain",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(current);
    assert_eq!(upper, "2999");

    // A date without offset is refused, before the rules.
    body["valid_to"] = json!("2999-01-01T00:00:00");
    let refused = send(
        &app,
        common::bearer(submission(
            "/v1/changesets",
            "application/json",
            body.to_string(),
        )),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
}

#[kartenn_testing::pg_test]
async fn a_refused_changeset_lists_every_violation_in_one_422(pool: PgPool) {
    let app = app(&pool).await;
    // REQ-006-21: two violations of different rules, on operations 2 and 3.
    let body = lab(&["10.0.0.0/24", "10.1.2.5/24", "fe80::%2/64"]).to_string();
    let refused = send(
        &app,
        common::bearer(submission("/v1/changesets", "application/json", body)),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let link = refused.headers()[header::LINK].to_str().unwrap().to_owned();
    let problem = json_of(refused).await;
    assert_eq!(problem["type"], "urn:kartenn:problem:changeset-refused");
    let found: Vec<(&str, &str, &Value)> = problem["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["rule"].as_str().unwrap(),
                v["severity"].as_str().unwrap(),
                &v["pointer"],
            )
        })
        .collect();
    assert_eq!(
        found,
        vec![
            ("core.prefix.unaligned", "error", &json!("/operations/2")),
            ("core.addr.zone", "error", &json!("/operations/3")),
        ]
    );
    assert_eq!(problem["violations"][0]["operation"], 2);
    // The refused changeset is journaled and linked; nothing else is written.
    let id: Uuid = problem["changeset"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        link,
        format!("</v1/changesets/{id}>; rel=\"{REL_CHANGESET}\"")
    );
    let state: String = sqlx::query_scalar("SELECT state FROM changeset WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "refused");
    assert_eq!(domains(&pool).await, 0);
}

#[kartenn_testing::pg_test]
async fn the_problems_come_in_the_order_of_http(pool: PgPool) {
    let app = app(&pool).await;
    // REQ-006-22: unauthenticated and malformed → 401.
    let anonymous = send(&app, submission("/v1/changesets", "application/json", "{")).await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    // Malformed before the rules: 400, then the media type 415, then the motive 422.
    let malformed = send(
        &app,
        common::bearer(submission("/v1/changesets", "application/json", "{")),
    )
    .await;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    let patch = send(
        &app,
        common::bearer(submission(
            "/v1/changesets",
            "application/merge-patch+json",
            lab(&[]).to_string(),
        )),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let mut unmotivated = lab(&["10.1.2.5/24"]);
    unmotivated["motive"] = json!(" ");
    let unmotivated = send(
        &app,
        common::bearer(submission(
            "/v1/changesets",
            "application/json",
            unmotivated.to_string(),
        )),
    )
    .await;
    assert_eq!(unmotivated.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        json_of(unmotivated).await["type"],
        "urn:kartenn:problem:motive-required"
    );
}

fn keyed(key: &str, body: &Value) -> Request<Body> {
    let mut request = common::bearer(submission(
        "/v1/changesets",
        "application/json",
        body.to_string(),
    ));
    request
        .headers_mut()
        .insert("idempotency-key", key.parse().unwrap());
    request
}

/// Status, `Location`, `Link` and body bytes.
async fn parts(response: Response) -> (StatusCode, Option<String>, Option<String>, Vec<u8>) {
    let header = |name| {
        response
            .headers()
            .get(name)
            .map(|v: &header::HeaderValue| v.to_str().unwrap().to_owned())
    };
    let (location, link) = (header(header::LOCATION), header(header::LINK));
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, location, link, bytes.to_vec())
}

#[kartenn_testing::pg_test]
async fn a_replay_answers_the_first_response_byte_for_byte(pool: PgPool) {
    let author = writer(&pool, "write").await;
    let app = app_as(&pool, author);
    let body = lab(&["10.0.0.0/24"]);
    let first = parts(send(&app, keyed("k-1", &body)).await).await;
    assert_eq!(first.0, StatusCode::CREATED);
    // REQ-006-15: same key, same content → the first response, from the journal.
    let replay = parts(send(&app, keyed("k-1", &body)).await).await;
    assert_eq!(replay, first);
    assert_eq!(domains(&pool).await, 1);
    // Same key, another content → 422.
    let other = send(&app, keyed("k-1", &lab(&["10.0.1.0/24"]))).await;
    assert_eq!(other.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        json_of(other).await["type"],
        "urn:kartenn:problem:idempotency-key-reused"
    );
    // The same key by another identity is its own: a second object. Its body differs,
    // the same one would break the uniqueness of the slug (D34).
    let mut second = json!({"motive": "a lab", "operations": [
        {"op": "create_domain", "local": 1, "slug": "lab-2"}]});
    let app_2 = app_as(&pool, colleague(&pool, author).await);
    let created = send(&app_2, keyed("k-1", &second)).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(domains(&pool).await, 2);
    // A key of 256 characters, or given twice → 400.
    second["operations"][0]["slug"] = json!("lab-3");
    let long = send(&app, keyed(&"k".repeat(256), &second)).await;
    assert_eq!(long.status(), StatusCode::BAD_REQUEST);
    let mut twice = keyed("k-2", &second);
    twice
        .headers_mut()
        .append("idempotency-key", "k-3".parse().unwrap());
    assert_eq!(send(&app, twice).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(domains(&pool).await, 2);
    // Past its 24 h, a key is purged (task of REQ-006-38).
    sqlx::query("UPDATE idempotency_key SET expires_at = now() WHERE identity_id = $1")
        .bind(author)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        kartenn_store_pg::idempotency::purge_expired(&pool)
            .await
            .unwrap(),
        1
    );
}

#[kartenn_testing::pg_test]
async fn a_replay_of_a_refusal_answers_the_same_problem(pool: PgPool) {
    let app = app(&pool).await;
    // The 422 serves errors, then warnings, as the journal keeps them.
    let body = lab(&["10.0.0.0/24", "10.1.2.5/24", "fe80::%2/64"]);
    let first = send(&app, keyed("r", &body)).await;
    assert_eq!(first.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let link = first.headers()[header::LINK].clone();
    let mut first = json_of(first).await;
    let replay = send(&app, keyed("r", &body)).await;
    assert_eq!(replay.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(replay.headers()[header::LINK], link);
    let mut replay = json_of(replay).await;
    // Only the `instance` of a problem, the identifier of each request, differs.
    assert_ne!(first["instance"], replay["instance"]);
    first["instance"] = Value::Null;
    replay["instance"] = Value::Null;
    assert_eq!(replay, first);
}

#[kartenn_testing::pg_test]
async fn a_replay_of_a_proposal_answers_202(pool: PgPool) {
    let proposer = app_of(&pool, "propose").await;
    let first = parts(send(&proposer, keyed("p", &lab(&["10.0.0.0/24"]))).await).await;
    assert_eq!(first.0, StatusCode::ACCEPTED);
    let replay = parts(send(&proposer, keyed("p", &lab(&["10.0.0.0/24"]))).await).await;
    assert_eq!(replay, first);
}

#[kartenn_testing::pg_test]
async fn a_key_in_progress_answers_409(pool: PgPool) {
    let app = app(&pool).await;
    let body = lab(&["10.0.0.0/24"]);
    // The journal held by another transaction: the first request waits there, its key
    // taken.
    let mut hold = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE changeset IN EXCLUSIVE MODE")
        .execute(&mut *hold)
        .await
        .unwrap();
    let first = tokio::spawn({
        let (app, body) = (app.clone(), body.clone());
        async move { send(&app, keyed("same", &body)).await.status() }
    });
    loop {
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT FROM pg_locks
                            WHERE relation = 'changeset'::regclass AND NOT granted)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // REQ-006-15: the same key meanwhile → 409, not a wait.
    let second = send(&app, keyed("same", &body)).await;
    assert_eq!(second.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_of(second).await["type"],
        "urn:kartenn:problem:idempotency-in-progress"
    );
    hold.commit().await.unwrap();
    assert_eq!(first.await.unwrap(), StatusCode::CREATED);
    assert_eq!(domains(&pool).await, 1);
}
