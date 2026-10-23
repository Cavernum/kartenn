//! Writes by resource through the router (spec 006, REQ-006-12 to 14; task 006-16):
//! `POST` creates with a link to its changeset, `PATCH` in JSON Merge Patch only, `DELETE`
//! closes; `If-Match` prevents a lost update, properties included; removing a property
//! and clearing it differ.

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
use kartenn_rest::resources::MERGE_PATCH;
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

async fn admin(pool: &PgPool) -> Router {
    app_as(pool, person(pool, &["*"]).await)
}

async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(common::bearer(request)).await.unwrap()
}

async fn json_of(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn post(uri: &str, body: &Value) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn patch(uri: &str, etag: Option<&str>, content_type: &str, body: &Value) -> Request<Body> {
    let mut request = Request::patch(uri).header(header::CONTENT_TYPE, content_type);
    if let Some(etag) = etag {
        request = request.header(header::IF_MATCH, etag);
    }
    request.body(Body::from(body.to_string())).unwrap()
}

/// The `ETag` and the body of `GET uri`.
async fn read(app: &Router, uri: &str) -> (String, Value) {
    let response = send(app, Request::get(uri).body(Body::empty()).unwrap()).await;
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    let etag = response.headers()[header::ETAG]
        .to_str()
        .unwrap()
        .to_owned();
    (etag, json_of(response).await)
}

/// Creates `body` in `collection`; the path of the object.
async fn created(app: &Router, collection: &str, body: &Value) -> String {
    let response = send(app, post(&format!("/v1/{collection}"), body)).await;
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .map(|l| l.to_str().unwrap().to_owned());
    assert_eq!(status, StatusCode::CREATED, "{}", json_of(response).await);
    location.unwrap()
}

fn id_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap()
}

/// The domain `lab` and the network `10.0.0.0/24` in it; their paths.
async fn lab(app: &Router) -> (String, String) {
    let domain = created(app, "domains", &json!({"motive": "a lab", "slug": "lab"})).await;
    let prefix = created(
        app,
        "prefixes",
        &json!({"motive": "a net", "domain": {"id": id_of(&domain)},
                "value": "10.0.0.0/24", "nature": "network"}),
    )
    .await;
    (domain, prefix)
}

#[kartenn_testing::pg_test]
async fn a_creation_links_to_its_changeset(pool: PgPool) {
    let author = person(&pool, &["*"]).await;
    let app = app_as(&pool, author);
    let domain = created(&app, "domains", &json!({"motive": "a lab", "slug": "lab"})).await;
    let response = send(
        &app,
        post(
            "/v1/prefixes",
            &json!({"motive": "a net", "domain": {"id": id_of(&domain)},
                    "value": "10.0.0.0/24", "nature": "network"}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let link = response.headers()[header::LINK]
        .to_str()
        .unwrap()
        .to_owned();
    let body = json_of(response).await;
    let changeset = body["changeset"]["href"].as_str().unwrap().to_owned();
    assert_eq!(link, format!("<{changeset}>; rel=\"{REL_CHANGESET}\""));
    assert!(location.starts_with("/v1/prefixes/"), "{location}");
    let (_, prefix) = read(&app, &location).await;
    assert_eq!(prefix["value"], "10.0.0.0/24");
    assert_eq!(prefix["domain"]["slug"], "lab");

    // REQ-006-12: the changeset of the route has the author and the motive.
    let (by, motive): (Uuid, String) =
        sqlx::query_as("SELECT author_id, motive FROM changeset WHERE id = $1")
            .bind(Uuid::parse_str(body["changeset"]["id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((by, motive.as_str()), (author, "a net"));

    // A member outside the representation → 400; a refusal points at the body.
    let unknown = send(
        &app,
        post(
            "/v1/domains",
            &json!({"motive": "m", "slug": "x", "colour": 1}),
        ),
    )
    .await;
    assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
    let duplicate = send(
        &app,
        post(
            "/v1/prefixes",
            &json!({"motive": "again", "domain": {"id": id_of(&domain)},
                    "value": "10.0.0.0/24", "nature": "network"}),
        ),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let problem = json_of(duplicate).await;
    assert_eq!(problem["violations"][0]["pointer"], "", "{problem}");
    assert!(problem["violations"][0].get("operation").is_none());
}

#[kartenn_testing::pg_test]
async fn a_creation_replays_its_first_response(pool: PgPool) {
    let app = admin(&pool).await;
    let request = || {
        let mut request = post("/v1/domains", &json!({"motive": "a lab", "slug": "lab"}));
        request
            .headers_mut()
            .insert("idempotency-key", "k-1".parse().unwrap());
        request
    };
    let first = send(&app, request()).await;
    let second = send(&app, request()).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    assert_eq!(second.status(), StatusCode::CREATED);
    assert_eq!(
        first.headers()[header::LOCATION],
        second.headers()[header::LOCATION]
    );
    assert_eq!(json_of(first).await, json_of(second).await);
    let domains: i64 = sqlx::query_scalar("SELECT count(*) FROM addressing_domain")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(domains, 1);
}

#[kartenn_testing::pg_test]
async fn a_lost_update_is_prevented(pool: PgPool) {
    let app = admin(&pool).await;
    let (_, prefix) = lab(&app).await;
    // Two clients read the same version.
    let (etag, _) = read(&app, &prefix).await;
    let (same, _) = read(&app, &prefix).await;
    assert_eq!(etag, same);

    let first = send(
        &app,
        patch(
            &prefix,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "container", "nature": "container"}),
        ),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK, "{}", json_of(first).await);

    let second = send(
        &app,
        patch(
            &prefix,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "network", "nature": "network"}),
        ),
    )
    .await;
    let status = second.status();
    let problem = json_of(second).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "{problem}");
    assert_eq!(
        problem["violations"][0]["rule"],
        "changeset.operation.conflict"
    );
    let (now, object) = read(&app, &prefix).await;
    assert_ne!(now, etag);
    assert_eq!(object["nature"], "container");

    // REQ-006-14: without If-Match → 428, `*` too.
    for etag in [None, Some("*")] {
        let missing = send(
            &app,
            patch(
                &prefix,
                etag,
                MERGE_PATCH,
                &json!({"motive": "m", "anycast": true}),
            ),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::PRECONDITION_REQUIRED);
        let problem = json_of(missing).await;
        assert!(problem["detail"].as_str().unwrap().contains("If-Match"));
    }
}

#[kartenn_testing::pg_test]
async fn a_patch_is_a_merge_patch(pool: PgPool) {
    let app = admin(&pool).await;
    let (_, prefix) = lab(&app).await;
    let (etag, _) = read(&app, &prefix).await;
    // REQ-006-13: JSON Patch → 415, before the precondition.
    let json_patch = send(
        &app,
        patch(
            &prefix,
            None,
            "application/json-patch+json",
            &json!([{"op": "replace", "path": "/anycast", "value": true}]),
        ),
    )
    .await;
    assert_eq!(json_patch.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    // A member the service does not change → 422 pointing at it.
    let read_only = send(
        &app,
        patch(
            &prefix,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "m", "public": true}),
        ),
    )
    .await;
    assert_eq!(read_only.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let problem = json_of(read_only).await;
    assert_eq!(problem["type"], "urn:kartenn:problem:invalid-member");
    assert_eq!(problem["pointer"], "/public");
    let undeclared = send(
        &app,
        patch(
            &prefix,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "m", "properties": {"custom.none": "x"}}),
        ),
    )
    .await;
    assert_eq!(undeclared.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        json_of(undeclared).await["pointer"],
        "/properties/custom.none"
    );

    // A stale or absent object: 412 for a weak tag, 404 for an absent object.
    let weak = send(
        &app,
        patch(
            &prefix,
            Some(&format!("W/{etag}")),
            MERGE_PATCH,
            &json!({"motive": "m", "anycast": true}),
        ),
    )
    .await;
    assert_eq!(weak.status(), StatusCode::PRECONDITION_FAILED);
    let absent = send(
        &app,
        patch(
            &format!("/v1/prefixes/{}", Uuid::now_v7()),
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "m", "anycast": true}),
        ),
    )
    .await;
    assert_eq!(absent.status(), StatusCode::NOT_FOUND);
}

#[kartenn_testing::pg_test]
async fn removing_and_clearing_a_property_differ(pool: PgPool) {
    let app = admin(&pool).await;
    let (domain, prefix) = lab(&app).await;
    let declared = send(
        &app,
        post(
            "/v1/changesets",
            &json!({"motive": "zone", "operations": [
                {"op": "declare_property", "local": 1, "key": "custom.zone", "type": "text",
                 "max_len": 20, "cardinality": "single", "carriers": ["domain", "prefix"],
                 "inherit": "addressing"},
                {"op": "declare_property", "local": 2, "key": "custom.tag", "type": "text",
                 "max_len": 20, "cardinality": "single", "carriers": ["prefix"]}
            ]}),
        ),
    )
    .await;
    assert_eq!(
        declared.status(),
        StatusCode::CREATED,
        "{}",
        json_of(declared).await
    );
    let properties = format!("{prefix}/properties");
    let resolved = |body: Value| body["properties"].clone();

    // The domain carries a zone the network inherits.
    let (etag, _) = read(&app, &domain).await;
    let zoned = send(
        &app,
        patch(
            &domain,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "zone", "properties": {"custom.zone": "inner"}}),
        ),
    )
    .await;
    assert_eq!(zoned.status(), StatusCode::OK, "{}", json_of(zoned).await);

    // A table of properties is modified key by key.
    let (etag, _) = read(&app, &prefix).await;
    let own = send(
        &app,
        patch(
            &prefix,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "own", "properties": {"custom.zone": "dmz", "custom.tag": "a"}}),
        ),
    )
    .await;
    assert_eq!(own.status(), StatusCode::OK, "{}", json_of(own).await);
    let (_, body) = read(&app, &properties).await;
    assert_eq!(resolved(body)["custom.zone"], "dmz");

    // The version covers the properties: the former ETag is stale.
    let stale = send(
        &app,
        patch(
            &prefix,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "late", "properties": {"custom.tag": "b"}}),
        ),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::PRECONDITION_FAILED);

    // `null` removes the assignment: the inherited value comes back; the other key stays.
    let (etag, _) = read(&app, &prefix).await;
    let removed = send(
        &app,
        patch(
            &prefix,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "inherit", "properties": {"custom.zone": null}}),
        ),
    )
    .await;
    assert_eq!(
        removed.status(),
        StatusCode::OK,
        "{}",
        json_of(removed).await
    );
    let (_, body) = read(&app, &properties).await;
    assert_eq!(resolved(body.clone())["custom.zone"], "inner");
    assert_eq!(resolved(body)["custom.tag"], "a");

    // `{"clear": true}` clears explicitly: no value, inheritance blocked.
    let (etag, _) = read(&app, &prefix).await;
    let cleared = send(
        &app,
        patch(
            &prefix,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "none", "properties": {"custom.zone": {"clear": true}}}),
        ),
    )
    .await;
    assert_eq!(
        cleared.status(),
        StatusCode::OK,
        "{}",
        json_of(cleared).await
    );
    let (_, body) = read(&app, &properties).await;
    let zone = resolved(body);
    assert!(
        zone.as_object().unwrap().contains_key("custom.zone"),
        "{zone}"
    );
    assert_eq!(zone["custom.zone"], Value::Null);
}

#[kartenn_testing::pg_test]
async fn a_deletion_closes_the_object(pool: PgPool) {
    let app = admin(&pool).await;
    let (_, prefix) = lab(&app).await;
    let (etag, _) = read(&app, &prefix).await;
    let delete = |etag: Option<&str>, body: Option<Value>| {
        let mut request = Request::delete(&prefix);
        if let Some(etag) = etag {
            request = request.header(header::IF_MATCH, etag);
        }
        match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
            None => request.body(Body::empty()).unwrap(),
        }
    };
    assert_eq!(
        send(&app, delete(None, None)).await.status(),
        StatusCode::PRECONDITION_REQUIRED
    );
    // Without a body, the motive is missing.
    let unmotivated = send(&app, delete(Some(&etag), None)).await;
    assert_eq!(unmotivated.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        json_of(unmotivated).await["type"],
        "urn:kartenn:problem:motive-required"
    );
    let closed = send(&app, delete(Some(&etag), Some(json!({"motive": "gone"})))).await;
    assert_eq!(closed.status(), StatusCode::OK);
    assert!(closed.headers().get(header::LINK).is_some());
    let gone = send(&app, Request::get(&prefix).body(Body::empty()).unwrap()).await;
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

#[kartenn_testing::pg_test]
async fn an_approval_policy_answers_202(pool: PgPool) {
    let app = admin(&pool).await;
    let (domain, _) = lab(&app).await;
    let policy = send(
        &app,
        post(
            "/v1/changesets",
            &json!({"motive": "review", "operations": [
                {"op": "set_approval_policy", "name": "all", "min_operations": 1}
            ]}),
        ),
    )
    .await;
    assert_eq!(
        policy.status(),
        StatusCode::CREATED,
        "{}",
        json_of(policy).await
    );
    let (etag, _) = read(&app, &domain).await;
    let pending = send(
        &app,
        patch(
            &domain,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "rename", "slug": "lab2"}),
        ),
    )
    .await;
    assert_eq!(pending.status(), StatusCode::ACCEPTED);
    assert!(pending.headers().get(header::LINK).is_some());
    let body = json_of(pending).await;
    assert_eq!(body["state"], "pending");
    let (_, unchanged) = read(&app, &domain).await;
    assert_eq!(unchanged["slug"], "lab");
}

#[kartenn_testing::pg_test]
async fn a_patch_renames_and_moves(pool: PgPool) {
    let app = admin(&pool).await;
    let root = created(&app, "entities", &json!({"motive": "m", "slug": "root"})).await;
    let child = created(
        &app,
        "entities",
        &json!({"motive": "m", "slug": "child", "parent": {"id": id_of(&root)}}),
    )
    .await;
    let (etag, before) = read(&app, &child).await;
    assert_eq!(before["parent"]["slug"], "root");
    // Two operations under one guard: `null` moves the entity to the root.
    let moved = send(
        &app,
        patch(
            &child,
            Some(&etag),
            MERGE_PATCH,
            &json!({"motive": "m", "slug": "alone", "parent": null}),
        ),
    )
    .await;
    assert_eq!(moved.status(), StatusCode::OK, "{}", json_of(moved).await);
    let (_, after) = read(&app, &child).await;
    assert_eq!(after["slug"], "alone");
    assert_eq!(after["parent"], Value::Null);
}

/// The strong `ETag` of an object is its version (REQ-006-14), which a renaming of the
/// object it links to leaves as it is while the summary of the link shows the new slug:
/// a revalidation answers 304 on a stale body. Fixing it changes the contract of the tag
/// (H8, H13b), an arbitration pending.
#[kartenn_testing::pg_test]
#[ignore = "stale ETag of a link summary: contract arbitration pending (REQ-006-14, H13b)"]
async fn renaming_the_parent_changes_the_etag_of_the_child(pool: PgPool) {
    let app = admin(&pool).await;
    let (domain, prefix) = lab(&app).await;
    let (etag, before) = read(&app, &prefix).await;
    assert_eq!(before["domain"]["slug"], "lab");
    let (domain_etag, _) = read(&app, &domain).await;
    let renamed = send(
        &app,
        patch(
            &domain,
            Some(&domain_etag),
            MERGE_PATCH,
            &json!({"motive": "m", "slug": "lab-2"}),
        ),
    )
    .await;
    assert_eq!(
        renamed.status(),
        StatusCode::OK,
        "{}",
        json_of(renamed).await
    );
    let again = send(
        &app,
        Request::get(&prefix)
            .header(header::IF_NONE_MATCH, &etag)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(again.status(), StatusCode::OK, "the domain summary changed");
    assert_eq!(json_of(again).await["domain"]["slug"], "lab-2");
}
