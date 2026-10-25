//! The administration of the rights through the router (spec 006, REQ-006-4, 32; task
//! 006-18): a delegate binds without escalation (005, REQ-005-26), the secret of a
//! service account is shown once and never compressed nor stored, the reads need
//! `core.rbac.read` (`GET /v1/me`: `tests/sessions.rs`).

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

/// A person granted `entries` on `scope` (`None`: everywhere).
async fn person_on(pool: &PgPool, entries: &[&str], scope: Option<(&str, Uuid)>) -> Uuid {
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
        .bind(scope.map_or("global", |(kind, _)| kind))
        .bind(scope.map(|(_, id)| id))
        .execute(pool)
        .await
        .unwrap();
    identity
}

async fn person(pool: &PgPool, entries: &[&str]) -> Uuid {
    person_on(pool, entries, None).await
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

fn with_body(method: &str, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT_ENCODING, "zstd, br, gzip")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

/// `PUT /v1/roles/{key}` by `app`; the status.
async fn put_role(app: &Router, key: &str, entries: &[&str]) -> StatusCode {
    let response = send(
        app,
        with_body(
            "PUT",
            &format!("/v1/roles/{key}"),
            &json!({"motive": "a role", "labels": { "fr": key }, "entries": entries}),
        ),
    )
    .await;
    let status = response.status();
    assert!(status.is_success(), "{}", json_of(response).await);
    status
}

/// The rules of the violations of a refused response.
async fn refused_by(response: Response) -> Vec<String> {
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    json_of(response).await["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["rule"].as_str().unwrap().to_owned())
        .collect()
}

#[kartenn_testing::pg_test]
async fn a_delegate_binds_without_escalation(pool: PgPool) {
    let admin = app_as(&pool, person(&pool, &["*"]).await);
    let response = send(
        &admin,
        with_body(
            "POST",
            "/v1/entities",
            &json!({"motive": "a unit", "slug": "unit"}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let unit: Uuid = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        put_role(&admin, "viewer", &["core.entity.read"]).await,
        StatusCode::CREATED
    );
    assert_eq!(
        put_role(&admin, "viewer", &["core.entity.read"]).await,
        StatusCode::OK
    );
    put_role(
        &admin,
        "writer",
        &[
            "core.entity.read",
            "core.entity.edit",
            "core.entity.apply_own",
        ],
    )
    .await;
    let delegate = app_as(
        &pool,
        person_on(
            &pool,
            &["core.rbac.admin", "core.entity.read"],
            Some(("entity", unit)),
        )
        .await,
    );
    let someone = person(&pool, &[]).await;
    let group: String = sqlx::query_scalar(kartenn_testing::GROUP_OF)
        .bind(someone)
        .fetch_one(&pool)
        .await
        .unwrap();
    let binding = |role: &str, scope: Value| {
        with_body(
            "POST",
            "/v1/bindings",
            &json!({"motive": "delegated", "group": group, "role": role, "scope": scope}),
        )
    };

    // A role whose entries the delegate holds on its scope: bound.
    let response = send(
        &delegate,
        binding("viewer", json!({"kind": "entity", "id": unit})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(
        response.headers()[header::LINK]
            .to_str()
            .unwrap()
            .contains(REL_CHANGESET)
    );
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let read = json_of(send(&admin, get(&location)).await).await;
    assert_eq!(read["role"], "viewer");
    assert_eq!(read["group"], json!(group));
    assert_eq!(read["scope"], json!({"kind": "entity", "id": unit}));

    // A role holding a permission the delegate lacks: escalation.
    let response = send(
        &delegate,
        binding("writer", json!({"kind": "entity", "id": unit})),
    )
    .await;
    assert_eq!(refused_by(response).await, ["core.binding.escalation"]);

    // Outside its scope: forbidden.
    let response = send(&delegate, binding("viewer", json!({"kind": "global"}))).await;
    assert_eq!(refused_by(response).await, ["core.rbac.forbidden"]);

    // Nor may it read the administration of the rights.
    let response = send(&delegate, get(&location)).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // The binding ends by DELETE.
    let response = send(
        &admin,
        with_body("DELETE", &location, &json!({"motive": "over"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        send(&admin, get(&location)).await.status(),
        StatusCode::NOT_FOUND
    );
}

/// The secret of a write that shows it: the response is marked `Secret` (no `no-store`
/// otherwise), never compressed although the client accepts it; the value once.
async fn shown_secret(response: Response) -> (Option<String>, Value) {
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(response.headers().get(header::CONTENT_ENCODING).is_none());
    let location = response
        .headers()
        .get(header::LOCATION)
        .map(|l| l.to_str().unwrap().to_owned());
    let body = json_of(response).await;
    assert_eq!(body["secret"]["value"].as_str().unwrap().len(), 43);
    (location, body["secret"].clone())
}

#[kartenn_testing::pg_test]
async fn a_secret_is_never_compressed(pool: PgPool) {
    let admin = app_as(&pool, person(&pool, &["*"]).await);
    let response = send(
        &admin,
        with_body(
            "POST",
            "/v1/service-accounts",
            &json!({"motive": "a robot", "name": "robot", "expires_at": "2099-01-01T00:00:00Z"}),
        ),
    )
    .await;
    let (location, first) = shown_secret(response).await;
    let location = location.unwrap();
    assert_eq!(first["expires_at"], "2099-01-01T00:00:00Z");

    let response = send(
        &admin,
        with_body(
            "POST",
            &format!("{location}/secrets"),
            &json!({"motive": "a rotation"}),
        ),
    )
    .await;
    let (_, second) = shown_secret(response).await;

    // Read back: the secrets, never their value nor digest.
    let response = send(&admin, get(&location)).await;
    assert!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .is_none_or(|v| v != "no-store")
    );
    let account = json_of(response).await;
    assert_eq!(account["name"], "robot");
    let secrets = account["secrets"].as_array().unwrap();
    assert_eq!(secrets.len(), 2);
    assert_eq!(secrets[0]["id"], first["id"]);
    assert!(
        secrets
            .iter()
            .all(|s| s.get("value").is_none() && s.get("digest").is_none())
    );

    // A revocation.
    let response = send(
        &admin,
        with_body(
            "DELETE",
            &format!("{location}/secrets/{}", second["id"].as_str().unwrap()),
            &json!({"motive": "leaked"}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let account = json_of(send(&admin, get(&location)).await).await;
    assert_eq!(account["secrets"][1]["revoked"], true);
}

#[kartenn_testing::pg_test]
async fn a_replay_does_not_show_the_secret_again(pool: PgPool) {
    let admin = app_as(&pool, person(&pool, &["*"]).await);
    let create = || {
        let mut request = with_body(
            "POST",
            "/v1/service-accounts",
            &json!({"motive": "a robot", "name": "robot"}),
        );
        request
            .headers_mut()
            .insert("idempotency-key", "robot-1".parse().unwrap());
        request
    };
    shown_secret(send(&admin, create()).await).await;
    let response = send(&admin, create()).await;
    let status = response.status();
    let body = json_of(response).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body.get("secret").is_none());
}

#[kartenn_testing::pg_test]
async fn reading_the_rights_needs_core_rbac_read(pool: PgPool) {
    let reader = app_as(&pool, person(&pool, &["core.rbac.read"]).await);
    let nobody = app_as(&pool, person(&pool, &["core.entity.read"]).await);
    for uri in [
        "/v1/roles",
        "/v1/bindings",
        "/v1/issuer-attachments",
        "/v1/service-accounts",
        "/v1/identities",
    ] {
        let response = send(&reader, get(uri)).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert!(json_of(response).await["items"].is_array());
        assert_eq!(
            send(&nobody, get(uri)).await.status(),
            StatusCode::FORBIDDEN,
            "{uri}"
        );
    }
    let denied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'authz.access.denied'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(denied, 5);
    let read: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'data.objects.read'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(read, 5);
    let roles = json_of(send(&reader, get("/v1/roles?page_size=1")).await).await;
    assert_eq!(roles["items"].as_array().unwrap().len(), 1);
    assert!(roles["links"]["next"].is_string());
}

#[kartenn_testing::pg_test]
async fn attachments_and_identities_are_administered(pool: PgPool) {
    let admin = app_as(&pool, person(&pool, &["*"]).await);
    let issuer = "https://idp.example.invalid/realm";
    let response = send(
        &admin,
        with_body(
            "POST",
            "/v1/issuer-attachments",
            &json!({"motive": "an idp", "issuer": issuer}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let listed = json_of(send(&admin, get("/v1/issuer-attachments")).await).await;
    assert_eq!(listed["items"], json!([{"issuer": issuer, "entity": null}]));
    let query: String = form_urlencoded::Serializer::new(String::new())
        .append_pair("issuer", issuer)
        .finish();
    let response = send(
        &admin,
        Request::delete(format!("/v1/issuer-attachments?{query}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"motive": "gone"}).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let listed = json_of(send(&admin, get("/v1/issuer-attachments")).await).await;
    assert_eq!(listed["items"], json!([]));

    let someone = person(&pool, &[]).await;
    let uri = format!("/v1/identities/{someone}");
    let response = send(
        &admin,
        Request::patch(&uri)
            .header(header::CONTENT_TYPE, MERGE_PATCH)
            .body(Body::from(
                json!({"motive": "departed", "disabled": true, "revoke_sessions": true})
                    .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json_of(response).await
    );
    let state = json_of(send(&admin, get(&uri)).await).await;
    assert_eq!(state["disabled"], true);
    assert!(state["revoked_after"].is_string());
}

/// REQ-018-7, R20: a role carries its labels as a table neutral in language, without a
/// resolved `label`; the body and its `ETag` do not vary with `Accept-Language`.
#[kartenn_testing::pg_test]
async fn a_body_does_not_vary_with_the_language(pool: PgPool) {
    let admin = app_as(&pool, person(&pool, &["*"]).await);
    let body = json!({"motive": "m", "labels": {"fr": "Lecteur des flux", "en": "Flow reader"},
                      "entries": ["core.entity.read"]});
    let response = send(&admin, with_body("PUT", "/v1/roles/flow_reader", &body)).await;
    assert!(
        response.status().is_success(),
        "{}",
        json_of(response).await
    );
    let mut seen = Vec::new();
    for language in ["fr", "en", "de-CH, en;q=0.5"] {
        let request = Request::get("/v1/roles/flow_reader")
            .header(header::ACCEPT_LANGUAGE, language)
            .body(Body::empty())
            .unwrap();
        let response = send(&admin, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::CONTENT_LANGUAGE).is_none());
        let etag = response.headers().get(header::ETAG).cloned();
        let role = json_of(response).await;
        assert_eq!(
            role["labels"],
            json!({"fr": "Lecteur des flux", "en": "Flow reader"})
        );
        assert!(role.get("label").is_none(), "{role}");
        seen.push((etag, role));
    }
    assert!(seen.windows(2).all(|w| w[0] == w[1]), "{seen:?}");
}
