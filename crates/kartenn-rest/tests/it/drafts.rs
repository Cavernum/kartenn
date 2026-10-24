//! Drafts through the router (spec 024, REQ-024-1 to 7a; tasks 024-5, 6): the routes of
//! the author, 404 for someone else, `If-Match`, the limits, the retention announced, the
//! submission (deleted on 201/202, kept on 422, replayed), the audit, and the actions of
//! `core.draft.admin`.

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

use crate::common;

/// A person holding `permissions` everywhere.
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

/// Who edits domains and applies their own changesets.
const EDITOR: &[&str] = &[
    "core.domain.read",
    "core.domain.edit",
    "core.domain.apply_own",
];
/// Who only edits.
const EDIT_ONLY: &[&str] = &[
    "core.domain.read",
    "core.domain.edit",
    "core.changeset.read",
];

fn app(pool: &PgPool, identity: Uuid, kind: IdentityKind) -> Router {
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
        Arc::new(common::As(Principal {
            identity,
            kind,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    )
}

async fn as_person(pool: &PgPool, permissions: &[&str]) -> (Uuid, Router) {
    let identity = person(pool, permissions).await;
    (identity, app(pool, identity, IdentityKind::Person))
}

/// A request with an access token, its body JSON when given.
fn request(method: &str, uri: &str, if_match: Option<i64>, body: Option<Value>) -> Request<Body> {
    let mut r = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, "Bearer test");
    if let Some(version) = if_match {
        r = r.header(header::IF_MATCH, format!("\"{version}\""));
    }
    match body {
        Some(body) => r
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => r.body(Body::empty()).unwrap(),
    }
}

async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

async fn json_of(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    if bytes.is_empty() {
        return Value::Null;
    }
    serde_json::from_slice(&bytes).unwrap()
}

/// The status, and the body as JSON.
async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    if_match: Option<i64>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = send(app, request(method, uri, if_match, body)).await;
    (response.status(), json_of(response).await)
}

fn domain(slug: &str) -> Value {
    json!({"op": "create_domain", "local": null, "slug": slug})
}

/// A new draft of `app`: its identifier.
async fn created(app: &Router, body: Value) -> Uuid {
    let (status, draft) = call(app, "POST", "/v1/drafts", None, Some(body)).await;
    assert_eq!(status, StatusCode::CREATED, "{draft}");
    draft["id"].as_str().unwrap().parse().unwrap()
}

/// The audit events named `name` targeting `id`: their details.
async fn audited(pool: &PgPool, name: &str, id: Uuid) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT coalesce(details, '{}'::jsonb) FROM audit.event
         WHERE name = $1 AND $2 = ANY(target_ids) ORDER BY occurred_at",
    )
    .bind(name)
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[kartenn_testing::pg_test]
async fn an_author_keeps_a_draft_under_its_versions(pool: PgPool) {
    let (author, app) = as_person(&pool, EDITOR).await;
    // REQ-024-1: created with its first operations, its version as ETag, its URL.
    let response = send(
        &app,
        request(
            "POST",
            "/v1/drafts",
            None,
            Some(json!({"title": "Lab", "operations": [domain("lab")]})),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let draft = json_of(response).await;
    let id = draft["id"].as_str().unwrap();
    assert_eq!(location, format!("/v1/drafts/{id}"));
    assert_eq!(draft["author"], author.to_string());
    assert_eq!(draft["operations"], json!([domain("lab")]));
    assert_eq!(draft["operation_count"], 1);
    assert!(draft.get("expires_at").is_none(), "{draft}");

    // The list names it without its operations.
    let (status, list) = call(&app, "GET", "/v1/drafts", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert_eq!(list["items"][0]["operation_count"], 1);
    assert!(list["items"][0].get("reason").is_none());
    assert!(!list["items"][0]["operations"].is_array());

    // REQ-024-3: If-Match on every modification.
    let uri = format!("/v1/drafts/{id}");
    let patch = json!({"title": "Lab 2", "reason": {"summary": "Two labs"}});
    let (status, _) = call(&app, "PATCH", &uri, None, Some(patch.clone())).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    let (status, _) = call(&app, "PATCH", &uri, Some(7), Some(patch.clone())).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    let (status, changed) = call(&app, "PATCH", &uri, Some(1), Some(patch.clone())).await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(changed["version"], 2);
    assert_eq!(changed["title"], "Lab 2");
    assert_eq!(changed["reason"], json!({"summary": "Two labs"}));
    // Another tab on version 1: 412.
    let (status, _) = call(&app, "PATCH", &uri, Some(1), Some(patch)).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);

    // Operations added after its own; one that is not an operation → 400.
    let added = format!("{uri}/operations");
    let (status, _) = call(
        &app,
        "POST",
        &added,
        Some(2),
        Some(json!({"operations": [{"op": "nope"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, changed) = call(
        &app,
        "POST",
        &added,
        Some(2),
        Some(json!({"operations": [domain("lab2")]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        changed["operations"],
        json!([domain("lab"), domain("lab2")])
    );
    assert_eq!(changed["version"], 3);

    // REQ-024-6: the creation is audited, the modifications are not.
    let id: Uuid = id.parse().unwrap();
    assert_eq!(audited(&pool, "draft.state.created", id).await.len(), 1);
    let (status, _) = call(&app, "DELETE", &uri, Some(2), None).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    let (status, _) = call(&app, "DELETE", &uri, Some(3), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, "GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let deleted = audited(&pool, "draft.state.deleted", id).await;
    assert_eq!(
        deleted,
        [json!({"cause": "author", "author": author.to_string()})]
    );
}

#[kartenn_testing::pg_test]
async fn the_draft_of_someone_else_is_absent(pool: PgPool) {
    let (_, author) = as_person(&pool, EDITOR).await;
    let (_, other) = as_person(&pool, EDITOR).await;
    let id = created(&author, json!({"title": "Mine"})).await;
    let uri = format!("/v1/drafts/{id}");
    // REQ-024-2: 404 whatever the route, the existence untold.
    for (method, path, version, body) in [
        ("GET", uri.clone(), None, None),
        (
            "PATCH",
            uri.clone(),
            Some(1),
            Some(json!({"title": "Theirs"})),
        ),
        ("DELETE", uri.clone(), Some(1), None),
        (
            "POST",
            format!("{uri}/operations"),
            Some(1),
            Some(json!({"operations": []})),
        ),
        ("POST", format!("{uri}/simulation"), None, None),
        ("POST", format!("{uri}/submission"), None, None),
        ("POST", format!("{uri}/takeover"), Some(1), None),
    ] {
        let (status, _) = call(&other, method, &path, version, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}");
    }
    let (_, list) = call(&other, "GET", "/v1/drafts", None, None).await;
    assert_eq!(list["items"], json!([]));
    // Listing someone else's needs `core.draft.admin`.
    let (status, _) = call(
        &other,
        "GET",
        &format!("/v1/identities/{}/drafts", Uuid::now_v7()),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A service account has none.
    let account = person(&pool, EDITOR).await;
    let service = app(&pool, account, IdentityKind::ServiceAccount);
    let (status, _) = call(&service, "GET", "/v1/drafts", None, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &service,
        "POST",
        "/v1/drafts",
        None,
        Some(json!({"title": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[kartenn_testing::pg_test]
async fn the_limits_refuse_beyond_50_drafts_and_10000_operations(pool: PgPool) {
    let (author, app) = as_person(&pool, EDITOR).await;
    // REQ-024-7: 10 001 operations → 422 `draft-limit`.
    let many: Vec<Value> = (0..10_001).map(|i| domain(&format!("d{i}"))).collect();
    let (status, problem) = call(
        &app,
        "POST",
        "/v1/drafts",
        None,
        Some(json!({"title": "Many", "operations": many})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem["type"], "urn:kartenn:problem:draft-limit");
    let full: Vec<Value> = (0..10_000).map(|i| domain(&format!("d{i}"))).collect();
    let id = created(&app, json!({"title": "Full", "operations": full})).await;
    let (status, problem) = call(
        &app,
        "POST",
        &format!("/v1/drafts/{id}/operations"),
        Some(1),
        Some(json!({"operations": [domain("one-more")]})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem["type"], "urn:kartenn:problem:draft-limit");
    // A title out of bounds.
    let (status, _) = call(
        &app,
        "POST",
        "/v1/drafts",
        None,
        Some(json!({"title": " "})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // 50 drafts, then a 51st → 422.
    sqlx::query(
        "INSERT INTO draft (id, author_id, title)
         SELECT gen_random_uuid(), $1, 'd' || n FROM generate_series(2, 50) n",
    )
    .bind(author)
    .execute(&pool)
    .await
    .unwrap();
    let (status, problem) = call(
        &app,
        "POST",
        "/v1/drafts",
        None,
        Some(json!({"title": "51"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem["type"], "urn:kartenn:problem:draft-limit");
}

#[kartenn_testing::pg_test]
async fn the_retention_is_announced_then_deletes(pool: PgPool) {
    let (author, app) = as_person(&pool, EDITOR).await;
    let soon = created(&app, json!({"title": "Soon"})).await;
    let gone = created(&app, json!({"title": "Gone"})).await;
    let kept = created(&app, json!({"title": "Kept"})).await;
    for (id, days) in [(soon, 84), (gone, 91), (kept, 82)] {
        sqlx::query(
            "UPDATE draft SET updated_at = now() - make_interval(days => $2) WHERE id = $1",
        )
        .bind(id)
        .bind(days)
        .execute(&pool)
        .await
        .unwrap();
    }
    // REQ-024-7: announced 7 days before the 90.
    let (_, draft) = call(&app, "GET", &format!("/v1/drafts/{soon}"), None, None).await;
    assert!(draft["expires_at"].is_string(), "{draft}");
    let (_, draft) = call(&app, "GET", &format!("/v1/drafts/{kept}"), None, None).await;
    assert!(draft.get("expires_at").is_none(), "{draft}");

    let deleted = kartenn_store_pg::drafts::expire(&pool, &Instance::new("test"))
        .await
        .unwrap();
    assert_eq!(deleted, 1);
    let (_, list) = call(&app, "GET", "/v1/drafts", None, None).await;
    let left: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["title"].as_str().unwrap())
        .collect();
    assert_eq!(left, ["Kept", "Soon"]);
    assert_eq!(
        audited(&pool, "draft.state.deleted", gone).await,
        [json!({"cause": "expired", "author": author.to_string()})]
    );
}

async fn domains(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM addressing_domain")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn a_submission_replaces_the_draft_or_leaves_it(pool: PgPool) {
    let (_, app) = as_person(&pool, EDITOR).await;
    let id = created(&app, json!({"title": "Lab", "operations": [domain("lab")]})).await;
    let uri = format!("/v1/drafts/{id}");

    // REQ-024-4: simulated under the rights of its author, nothing written; the title
    // stands for a motive not written yet.
    let (status, report) = call(&app, "POST", &format!("{uri}/simulation"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["valid"], true);
    assert_eq!(domains(&pool).await, 0);

    // REQ-024-5: refused (no motive) → 422, the draft stays as it was.
    let (status, _) = call(
        &app,
        "POST",
        &format!("{uri}/submission?apply=own"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, draft) = call(&app, "GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(draft["version"], 1);
    let (_, _) = call(
        &app,
        "PATCH",
        &uri,
        Some(1),
        Some(json!({"reason": {"summary": "A lab", "description": "For the tests."}})),
    )
    .await;
    // A version named that is no longer the draft's: 412.
    let (status, _) = call(
        &app,
        "POST",
        &format!("{uri}/submission?apply=own"),
        Some(1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);

    // Applied by its author: 201, the draft gone, the changeset names it.
    let submit = |key: &'static str| {
        let mut r = request(
            "POST",
            &format!("{uri}/submission?apply=own"),
            Some(2),
            None,
        );
        r.headers_mut()
            .insert("idempotency-key", key.parse().unwrap());
        r
    };
    let response = send(&app, submit("k1")).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let applied = json_of(response).await;
    assert_eq!(applied["state"], "applied");
    let changeset = applied["changeset"]["id"].as_str().unwrap().to_owned();
    assert_eq!(domains(&pool).await, 1);
    let (status, _) = call(&app, "GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (draft_id, motive): (Option<Uuid>, String) =
        sqlx::query_as("SELECT draft_id, motive FROM changeset WHERE id = $1::uuid")
            .bind(&changeset)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(draft_id, Some(id));
    assert_eq!(motive, "A lab\n\nFor the tests.");
    assert_eq!(
        audited(&pool, "draft.state.submitted", id).await,
        [json!({"state": "applied"})]
    );
    // Retried with its key once the draft is gone: the same answer.
    let response = send(&app, submit("k1")).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        json_of(response).await["changeset"]["id"],
        changeset.as_str()
    );
    // Another key, or none: the draft is absent.
    let response = send(&app, submit("k2")).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[kartenn_testing::pg_test]
async fn a_draft_submitted_pending_is_deleted_too(pool: PgPool) {
    let (_, app) = as_person(&pool, EDIT_ONLY).await;
    let id = created(
        &app,
        json!({"title": "Lab", "reason": {"summary": "A lab"}, "operations": [domain("lab")]}),
    )
    .await;
    // `apply=own` without `apply_own`: pending, with its reason (REQ-024-13).
    let (status, pending) = call(
        &app,
        "POST",
        &format!("/v1/drafts/{id}/submission?apply=own"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{pending}");
    assert_eq!(pending["reason"]["code"], "changeset.apply_own.missing");
    let (status, _) = call(&app, "GET", &format!("/v1/drafts/{id}"), None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let changeset = pending["changeset"]["id"].as_str().unwrap();
    // The changeset names the draft it comes from (M3).
    let response = send(
        &app,
        request("GET", &format!("/v1/changesets/{changeset}"), None, None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_of(response).await["draft"], id.to_string());
    assert_eq!(
        audited(&pool, "draft.state.submitted", id).await,
        [json!({"state": "pending"})]
    );
}

#[kartenn_testing::pg_test]
async fn an_administrator_reads_deletes_and_takes_over_audited(pool: PgPool) {
    let (author, mine) = as_person(&pool, EDITOR).await;
    let (admin, admin_app) = as_person(&pool, &["core.draft.admin"]).await;
    let first = created(&mine, json!({"title": "First"})).await;
    let second = created(&mine, json!({"title": "Second"})).await;

    // REQ-024-7a: listed, audited with the identity.
    let (status, list) = call(
        &admin_app,
        "GET",
        &format!("/v1/identities/{author}/drafts"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    let listed = audited(&pool, "draft.admin.read", first).await;
    assert_eq!(listed, [json!({"author": author.to_string(), "count": 2})]);

    // Read, audited.
    let (status, draft) = call(
        &admin_app,
        "GET",
        &format!("/v1/drafts/{first}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(draft["author"], author.to_string());
    assert_eq!(
        audited(&pool, "draft.admin.read", first).await.len(),
        2,
        "the read is audited"
    );
    // The author's own read is not.
    call(&mine, "GET", &format!("/v1/drafts/{second}"), None, None).await;
    assert_eq!(audited(&pool, "draft.admin.read", second).await.len(), 1);

    // Deleted, audited with the author.
    let (status, _) = call(
        &admin_app,
        "DELETE",
        &format!("/v1/drafts/{first}"),
        Some(1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        audited(&pool, "draft.state.deleted", first).await,
        [json!({"cause": "admin", "author": author.to_string()})]
    );

    // Taken over: the administrator's now, out of the former author's list.
    let (status, taken) = call(
        &admin_app,
        "POST",
        &format!("/v1/drafts/{second}/takeover"),
        Some(1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{taken}");
    assert_eq!(taken["author"], admin.to_string());
    assert_eq!(taken["version"], 2);
    let (_, list) = call(&mine, "GET", "/v1/drafts", None, None).await;
    assert_eq!(list["items"], json!([]));
    let (status, _) = call(&mine, "GET", &format!("/v1/drafts/{second}"), None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        audited(&pool, "draft.admin.taken_over", second).await,
        [json!({"author": author.to_string()})]
    );
    let (status, _) = call(
        &admin_app,
        "POST",
        &format!("/v1/drafts/{second}/takeover"),
        Some(2),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
