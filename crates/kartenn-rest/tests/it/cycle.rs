//! The cycle of a changeset through the router (spec 006, REQ-006-16; task 006-17): its
//! read under `core.changeset.read`, the list, the differential by pages with the
//! sensitive values masked, the proposals, and the actions validate, reject, withdraw and
//! undo under `If-Match`.

use crate::common;

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

async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone()
        .oneshot(common::bearer(common::applied(request)))
        .await
        .unwrap()
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

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

/// `POST uri` with `If-Match: etag` when given, and `body` when given.
fn act(uri: &str, etag: Option<&str>, body: Option<&Value>) -> Request<Body> {
    let mut request = Request::post(uri);
    if let Some(etag) = etag {
        request = request.header(header::IF_MATCH, etag);
    }
    match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap()
}

/// Submits `operations`; the status and body.
async fn submit(app: &Router, operations: Value) -> (StatusCode, Value) {
    let response = send(
        app,
        post(
            "/v1/changesets",
            &json!({"motive": "m", "operations": operations}),
        ),
    )
    .await;
    (response.status(), json_of(response).await)
}

/// Creates the domain `slug`; its identifier and the changeset that created it.
async fn domain(app: &Router, pool: &PgPool, slug: &str) -> (String, String) {
    let (status, body) = submit(app, json!([{"op": "create_domain", "slug": slug}])).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let changeset = body["changeset"]["id"].as_str().unwrap().to_owned();
    let id: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = $1")
        .bind(slug)
        .fetch_one(pool)
        .await
        .unwrap();
    (id.to_string(), changeset)
}

/// Renames the domain `id` through its route; the response.
async fn rename(app: &Router, id: &str, slug: &str) -> Response {
    let path = format!("/v1/domains/{id}");
    let read = send(app, get(&path)).await;
    let etag = read.headers()[header::ETAG].to_str().unwrap().to_owned();
    send(
        app,
        Request::patch(&path)
            .header(header::CONTENT_TYPE, MERGE_PATCH)
            .header(header::IF_MATCH, etag)
            .body(Body::from(
                json!({"motive": "rename", "slug": slug}).to_string(),
            ))
            .unwrap(),
    )
    .await
}

/// An approval policy that holds every changeset.
async fn hold_everything(app: &Router) {
    let (status, body) = submit(
        app,
        json!([{"op": "set_approval_policy", "name": "all", "min_operations": 1}]),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

/// Spec 024, REQ-024-12, 13: a policy holds the changeset of an author who asks to apply
/// it, even under `*`: 202, pending, `changeset.approval.self`, and nothing written.
#[kartenn_testing::pg_test]
async fn a_policy_keeps_apply_own_pending(pool: PgPool) {
    let app = app_as(&pool, person(&pool, &["*"]).await);
    hold_everything(&app).await;
    let response = send(
        &app,
        post(
            "/v1/changesets?apply=own",
            &json!({"motive": "m", "operations": [{"op": "create_domain", "slug": "lab"}]}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = json_of(response).await;
    assert_eq!(body["state"], "pending", "{body}");
    assert_eq!(body["kind"], "approval", "{body}");
    assert_eq!(body["reason"]["code"], "changeset.approval.self", "{body}");
    let domains: i64 = sqlx::query_scalar("SELECT count(*) FROM addressing_domain")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(domains, 0);
}

/// The identifier of the changeset a 202 holds.
async fn held(response: Response) -> String {
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    json_of(response).await["changeset"]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The slug of the domain `id` now.
async fn slug_of(pool: &PgPool, id: &str) -> String {
    sqlx::query_scalar(
        "SELECT slug FROM addressing_domain WHERE id = $1::uuid AND valid_period @> now()",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[kartenn_testing::pg_test]
async fn an_author_who_validates_gets_403(pool: PgPool) {
    let author = app_as(&pool, person(&pool, &["*"]).await);
    let approver = app_as(&pool, person(&pool, &["*"]).await);
    let (id, _) = domain(&author, &pool, "lab").await;
    hold_everything(&author).await;
    let pending = held(rename(&author, &id, "lab2").await).await;
    let validation = format!("/v1/changesets/{pending}/validation");

    // The changeset reads with its state as a strong ETag.
    let read = send(&author, get(&format!("/v1/changesets/{pending}"))).await;
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(read.headers()[header::ETAG], "\"pending\"");

    let own = send(&author, act(&validation, Some("\"pending\""), None)).await;
    assert_eq!(own.status(), StatusCode::FORBIDDEN);
    let denied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'authz.access.denied'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(denied >= 1);

    // An action needs If-Match with the current state.
    assert_eq!(
        send(&approver, act(&validation, None, None)).await.status(),
        StatusCode::PRECONDITION_REQUIRED
    );
    assert_eq!(
        send(&approver, act(&validation, Some("\"applied\""), None))
            .await
            .status(),
        StatusCode::PRECONDITION_FAILED
    );

    let applied = send(&approver, act(&validation, Some("\"pending\""), None)).await;
    assert_eq!(applied.status(), StatusCode::OK);
    assert!(applied.headers().get(header::LINK).is_some());
    assert_eq!(json_of(applied).await["state"], "applied");
    assert_eq!(slug_of(&pool, &id).await, "lab2");

    // Decided: the ETag of the action is stale.
    assert_eq!(
        send(&approver, act(&validation, Some("\"pending\""), None))
            .await
            .status(),
        StatusCode::PRECONDITION_FAILED
    );
}

// REQ-016-11, INV-61: operations joined to a validation in its body.
#[kartenn_testing::pg_test]
async fn a_validation_body_joins_operations_of_review_only(pool: PgPool) {
    let author = app_as(&pool, person(&pool, &["*"]).await);
    let approver = app_as(&pool, person(&pool, &["*"]).await);
    let (id, _) = domain(&author, &pool, "lab").await;
    hold_everything(&author).await;
    let pending = held(rename(&author, &id, "lab2").await).await;
    let validation = format!("/v1/changesets/{pending}/validation");

    // An operation of the core is not one of review: 422, recorded, still pending.
    let smuggled = json!({"operations": [{"op": "create_domain", "slug": "smuggled"}]});
    let refused = send(
        &approver,
        act(&validation, Some("\"pending\""), Some(&smuggled)),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        json_of(refused)
            .await
            .to_string()
            .contains("core.review.operation")
    );
    let recorded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event
         WHERE name = 'authz.access.denied' AND reason = 'core.review.operation'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recorded, 1);
    // The author, with an empty list: 403, as without a body.
    let own = send(
        &author,
        act(
            &validation,
            Some("\"pending\""),
            Some(&json!({"operations": []})),
        ),
    )
    .await;
    assert_eq!(own.status(), StatusCode::FORBIDDEN);
    assert_eq!(slug_of(&pool, &id).await, "lab");
}

#[kartenn_testing::pg_test]
async fn an_undo_in_conflict_answers_409_with_the_list(pool: PgPool) {
    let app = app_as(&pool, person(&pool, &["*"]).await);
    let (id, creation) = domain(&app, &pool, "lab").await;
    let renamed = rename(&app, &id, "lab2").await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let renaming = json_of(renamed).await["changeset"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // The creation was written over by the renaming.
    let conflict = send(
        &app,
        act(
            &format!("/v1/changesets/{creation}/undo"),
            Some("\"applied\""),
            None,
        ),
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let problem = json_of(conflict).await;
    assert_eq!(problem["type"], "urn:kartenn:problem:undo-conflict");
    assert_eq!(
        problem["conflicts"],
        json!([{"kind": "domain", "id": id, "property": null, "by": renaming}])
    );
    assert_eq!(slug_of(&pool, &id).await, "lab2");

    // The renaming is the last write: simulated, then undone.
    let undo = format!("/v1/changesets/{renaming}/undo");
    let simulated = send(
        &app,
        act(&format!("{undo}?simulate=true"), Some("\"applied\""), None),
    )
    .await;
    assert_eq!(simulated.status(), StatusCode::OK);
    let simulated = json_of(simulated).await;
    assert_eq!(simulated["valid"], true);
    assert_eq!(simulated["undoes"]["id"], renaming.as_str());
    assert!(!simulated["operations"].as_array().unwrap().is_empty());
    assert_eq!(slug_of(&pool, &id).await, "lab2");

    let undone = send(&app, act(&undo, Some("\"applied\""), None)).await;
    assert_eq!(undone.status(), StatusCode::CREATED);
    assert!(undone.headers().get(header::LOCATION).is_some());
    let body = json_of(undone).await;
    assert_eq!(body["undoes"]["id"], renaming.as_str());
    assert_eq!(body["unsupported"], json!([]));
    assert_eq!(slug_of(&pool, &id).await, "lab");
}

#[kartenn_testing::pg_test]
async fn reading_needs_core_changeset_read(pool: PgPool) {
    let author = person(&pool, &["*"]).await;
    let app = app_as(&pool, author);
    let (_, creation) = domain(&app, &pool, "lab").await;
    let reader = app_as(
        &pool,
        person(&pool, &["core.domain.read", "core.domain.edit"]).await,
    );
    for uri in [
        format!("/v1/changesets/{creation}"),
        "/v1/changesets".to_owned(),
        format!("/v1/changesets/{creation}/diff"),
        format!("/v1/changesets/{}", Uuid::now_v7()),
    ] {
        assert_eq!(
            send(&reader, get(&uri)).await.status(),
            StatusCode::FORBIDDEN,
            "{uri}"
        );
    }
    assert_eq!(
        send(&app, get(&format!("/v1/changesets/{}", Uuid::now_v7())))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    let read = send(&app, get(&format!("/v1/changesets/{creation}"))).await;
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(read.headers()[header::ETAG], "\"applied\"");
    let body = json_of(read).await;
    assert_eq!(body["state"], "applied");
    assert_eq!(body["author"], author.to_string());
    assert_eq!(body["operations"][0]["op"], "create_domain");
    assert_eq!(
        body["links"]["diff"],
        format!("/v1/changesets/{creation}/diff")
    );
}

#[kartenn_testing::pg_test]
async fn the_list_filters_by_state_and_author_by_pages(pool: PgPool) {
    let author = person(&pool, &["*"]).await;
    let app = app_as(&pool, author);
    for slug in ["a", "b", "c"] {
        domain(&app, &pool, slug).await;
    }
    let (status, _) = submit(&app, json!([{"op": "create_domain", "slug": "Bad slug"}])).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let first = send(
        &app,
        get(&format!(
            "/v1/changesets?state=applied&author={author}&page_size=2"
        )),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first = json_of(first).await;
    let items = first["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i["state"] == "applied"));
    // Newest first.
    assert!(items[0]["submitted_at"].as_str() >= items[1]["submitted_at"].as_str());
    let next = first["links"]["next"].as_str().unwrap();
    let second = json_of(send(&app, get(next)).await).await;
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert!(second["links"]["next"].is_null());

    let refused = json_of(send(&app, get("/v1/changesets?state=refused")).await).await;
    assert_eq!(refused["items"].as_array().unwrap().len(), 1);
    for bad in [
        "/v1/changesets?state=done",
        "/v1/changesets?author=me",
        "/v1/changesets?state=applied&state=refused",
        "/v1/changesets?valid_at=2026-01-01T00:00:00Z",
        "/v1/changesets?other=1",
    ] {
        assert_eq!(
            send(&app, get(bad)).await.status(),
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
}

/// The ids of the items of a list.
async fn listed(app: &Router, uri: &str) -> Vec<String> {
    let page = send(app, get(uri)).await;
    assert_eq!(page.status(), StatusCode::OK, "{uri}");
    json_of(page).await["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

#[kartenn_testing::pg_test]
async fn the_list_searches_by_object_period_and_retroactivity(pool: PgPool) {
    let app = app_as(&pool, person(&pool, &["*"]).await);
    let (lab, created) = domain(&app, &pool, "lab").await;
    let (_, other) = domain(&app, &pool, "other").await;
    assert_eq!(rename(&app, &lab, "lab2").await.status(), StatusCode::OK);
    let renamed: Uuid =
        sqlx::query_scalar("SELECT id FROM changeset ORDER BY submitted_at DESC, id DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let renamed = renamed.to_string();

    // Current version and history alike.
    let of_lab = listed(&app, &format!("/v1/changesets?object={lab}")).await;
    assert_eq!(of_lab, [renamed.clone(), created.clone()]);

    // `from` included, `to` excluded, on the submission time.
    let at: String = sqlx::query_scalar(
        "SELECT to_char(submitted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')
         FROM changeset WHERE id = $1",
    )
    .bind(Uuid::parse_str(&other).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    let at: String = form_urlencoded::byte_serialize(at.as_bytes()).collect();
    let since = listed(&app, &format!("/v1/changesets?from={at}")).await;
    assert_eq!(since, [renamed.clone(), other.clone()]);
    let before = listed(&app, &format!("/v1/changesets?to={at}")).await;
    assert_eq!(before, std::slice::from_ref(&created));

    assert_eq!(
        listed(&app, "/v1/changesets?retroactive=false").await.len(),
        3
    );
    assert!(
        listed(&app, "/v1/changesets?retroactive=true")
            .await
            .is_empty()
    );
    assert_eq!(
        listed(
            &app,
            &format!("/v1/changesets?object={lab}&retroactive=false")
        )
        .await,
        [renamed, created]
    );

    for bad in [
        "/v1/changesets?object=lab",
        "/v1/changesets?from=2026-01-01T00:00:00",
        "/v1/changesets?to=yesterday",
        "/v1/changesets?retroactive=maybe",
        "/v1/changesets?retroactive=true&retroactive=false",
    ] {
        assert_eq!(
            send(&app, get(bad)).await.status(),
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
}

#[kartenn_testing::pg_test]
async fn rejection_and_withdrawal_close_a_pending_changeset(pool: PgPool) {
    let author = app_as(&pool, person(&pool, &["*"]).await);
    let approver = app_as(&pool, person(&pool, &["*"]).await);
    let (id, _) = domain(&author, &pool, "lab").await;
    hold_everything(&author).await;

    let first = held(rename(&author, &id, "lab2").await).await;
    let rejection = format!("/v1/changesets/{first}/rejection");
    assert_eq!(
        send(
            &approver,
            act(&rejection, Some("\"pending\""), Some(&json!({})))
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(
            &author,
            act(
                &rejection,
                Some("\"pending\""),
                Some(&json!({"motive": "no"}))
            )
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let rejected = send(
        &approver,
        act(
            &rejection,
            Some("\"pending\""),
            Some(&json!({"motive": "no"})),
        ),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::OK);
    assert_eq!(json_of(rejected).await["state"], "rejected");
    let read = json_of(send(&author, get(&format!("/v1/changesets/{first}"))).await).await;
    assert_eq!(read["rejection"], "no");

    let second = held(rename(&author, &id, "lab3").await).await;
    let withdrawal = format!("/v1/changesets/{second}/withdrawal");
    assert_eq!(
        send(&approver, act(&withdrawal, Some("\"pending\""), None))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let withdrawn = send(&author, act(&withdrawal, Some("\"pending\""), None)).await;
    assert_eq!(withdrawn.status(), StatusCode::OK);
    assert_eq!(json_of(withdrawn).await["state"], "withdrawn");
    // Its state as If-Match, but no longer pending: 409.
    assert_eq!(
        send(&author, act(&withdrawal, Some("\"withdrawn\""), None))
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(slug_of(&pool, &id).await, "lab");
}

#[kartenn_testing::pg_test]
async fn the_proposals_are_those_the_caller_may_validate(pool: PgPool) {
    let admin = app_as(&pool, person(&pool, &["*"]).await);
    let proposer = app_as(
        &pool,
        person(&pool, &["core.domain.read", "core.domain.edit"]).await,
    );
    let outsider = app_as(&pool, person(&pool, &["core.domain.read"]).await);
    let (status, body) = submit(&proposer, json!([{"op": "create_domain", "slug": "lab"}])).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let proposal = body["changeset"]["id"].as_str().unwrap().to_owned();

    let listed = json_of(send(&admin, get("/v1/proposals")).await).await;
    let items = listed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], proposal.as_str());
    assert_eq!(items[0]["state"], "pending");
    assert_eq!(items[0]["required"][0]["permission"], "core.domain.edit");

    for app in [&proposer, &outsider] {
        let listed = json_of(send(app, get("/v1/proposals")).await).await;
        assert_eq!(listed["items"], json!([]));
    }
}

#[kartenn_testing::pg_test]
async fn a_sensitive_value_is_masked_without_its_right(pool: PgPool) {
    let admin = app_as(&pool, person(&pool, &["*"]).await);
    let auditor = app_as(
        &pool,
        person(&pool, &["core.changeset.read", "core.prefix.read"]).await,
    );
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
    let (lab, _) = domain(&admin, &pool, "lab").await;
    let (status, body) = submit(
        &admin,
        json!([
            {"op": "create_prefix", "local": 1, "domain": {"id": lab},
             "prefix": "10.0.0.0/24", "nature": "network"},
            {"op": "create_prefix", "local": 2, "domain": {"id": lab},
             "prefix": "10.0.1.0/24", "nature": "network"},
        ]),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let prefixes: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM prefix ORDER BY prefix")
        .fetch_all(&pool)
        .await
        .unwrap();
    let assign = |prefix: Uuid| {
        json!({"op": "assign_property", "object": {"id": prefix}, "kind": "prefix",
               "property": {"id": property}, "values": ["s3cret"]})
    };
    let (status, body) = submit(&admin, json!([assign(prefixes[0]), assign(prefixes[1])])).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let changeset = body["changeset"]["id"].as_str().unwrap().to_owned();

    let shown = json_of(send(&admin, get(&format!("/v1/changesets/{changeset}"))).await).await;
    assert_eq!(shown["operations"][0]["values"], json!(["s3cret"]));
    let masked = json_of(send(&auditor, get(&format!("/v1/changesets/{changeset}"))).await).await;
    assert!(masked["operations"][0]["values"].is_null());
    assert_eq!(masked["operations"][0]["masked"], true);
    assert!(!masked.to_string().contains("s3cret"), "{masked}");

    let diff = format!("/v1/changesets/{changeset}/diff");
    let shown = json_of(send(&admin, get(&diff)).await).await;
    assert_eq!(shown["items"][0]["property"], "sec.code");
    assert_eq!(shown["items"][0]["after"], "s3cret");
    assert_eq!(shown["items"][0]["masked"], false);
    // By pages, in the order of the journal.
    let first = json_of(send(&admin, get(&format!("{diff}?page_size=1"))).await).await;
    assert_eq!(first["items"].as_array().unwrap().len(), 1);
    assert_eq!(first["items"][0], shown["items"][0]);
    let next = first["links"]["next"].as_str().unwrap();
    let second = json_of(send(&admin, get(next)).await).await;
    assert_eq!(second["items"], json!([shown["items"][1]]));
    assert!(second["links"]["next"].is_null());
    let masked = json_of(send(&auditor, get(&diff)).await).await;
    assert!(masked["items"][0]["after"].is_null());
    assert_eq!(masked["items"][0]["masked"], true);
    assert!(!masked.to_string().contains("s3cret"), "{masked}");
    assert!(!masked["counts"].is_null());
}
