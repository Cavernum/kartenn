//! The collection of the clusters (task 019-8a; REQ-019-5, 38; design « Routes »):
//! `GET, POST /v1/kube-clusters` (`?slug=`, `q`) and `GET, PATCH, DELETE
//! /v1/kube-clusters/{id}`, dated reads, writes by changeset under `If-Match`, a cluster
//! the caller may not read answered as an absent one.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_service::authn::IdentityKind;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use tower::ServiceExt;

use super::super::Env;
use super::{app, person, principal, rfc3339};

/// The answer of `method uri` with the JSON `body` of `media` and `headers`.
async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<(&str, Value)>,
    headers: &[(&str, String)],
) -> (StatusCode, HeaderMap, Value) {
    let uri = kartenn_testing::applied_by_author(method, uri);
    let mut request = Request::builder().method(method).uri(uri).header(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer test"),
    );
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let request = match body {
        Some((media, body)) => request
            .header(header::CONTENT_TYPE, media)
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, value)
}

async fn get(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Value) {
    call(app, "GET", uri, None, &[]).await
}

/// The slugs of a page.
fn slugs(page: &Value) -> Vec<&str> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["slug"].as_str().unwrap())
        .collect()
}

/// The living prefix of `domain`.
async fn prefix(pool: &PgPool, domain: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM prefix WHERE domain_id = $1 AND valid_period @> now()")
        .bind(domain)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// REQ-019-5, 38: a cluster declared, listed by its slug, read at a date and as asserted
/// before its change, changed under `If-Match`, closed.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // one life of a cluster through its routes
async fn cluster_routes_round_trip(pool: PgPool) {
    let env = Env::new(&pool).await;
    let admin = person(&pool, &["*"]).await;
    let app = app(&pool, principal(admin, IdentityKind::Person), None);
    let (container, network) = (
        prefix(&pool, env.pods).await,
        prefix(&pool, env.services).await,
    );
    let before = OffsetDateTime::now_utc();

    // Declared: 201 and its URL; the defaults of the handler where nothing is given.
    let creation = json!({
        "motive": "declare",
        "slug": "staging",
        "owner": {"id": env.acme},
        "pod_domain": {"id": env.pods},
        "service_domain": {"id": env.services},
        "pod_containers": [{"id": container}],
        "service_networks": [{"id": network}],
        "retention_days": 14,
        "tracked_label_keys": ["app.example.org/tier"],
    });
    let (status, headers, body) = call(
        &app,
        "POST",
        "/v1/kube-clusters",
        Some(("application/json", creation.clone())),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let location = headers[header::LOCATION].to_str().unwrap().to_owned();
    let id: Uuid = location
        .strip_prefix("/v1/kube-clusters/")
        .unwrap()
        .parse()
        .unwrap();
    // The slug is unique among the living clusters.
    let (status, _, body) = call(
        &app,
        "POST",
        "/v1/kube-clusters",
        Some(("application/json", creation)),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    // Listed by slug, suggested by a prefix of it, the whole collection by slug.
    let (status, _, page) = get(&app, "/v1/kube-clusters?slug=staging").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(slugs(&page), ["staging"]);
    let item = &page["items"][0];
    assert_eq!(item["id"], json!(id));
    assert_eq!(item["href"], json!(location));
    assert_eq!(item["owner"], json!({"id": env.acme}));
    assert_eq!(item["site"], Value::Null);
    assert_eq!(item["pod_domain"], json!({"id": env.pods}));
    assert_eq!(item["service_domain"], json!({"id": env.services}));
    assert_eq!(item["external_domain"], Value::Null);
    assert_eq!(item["pod_containers"], json!([{"id": container}]));
    assert_eq!(item["service_networks"], json!([{"id": network}]));
    assert_eq!(item["retention_days"], 14);
    assert_eq!(item["tracked_label_keys"], json!(["app.example.org/tier"]));
    assert_eq!(item["batch_window_seconds"], 10);
    assert_eq!(item["batch_max_operations"], 500);
    assert_eq!(item["create_node_machines"], false);
    assert_eq!(item["connector_group"], Value::Null);
    assert!(item["version"].is_string(), "{item}");
    let (_, _, page) = get(&app, "/v1/kube-clusters").await;
    assert_eq!(slugs(&page), ["prod", "staging"]);
    let (_, _, page) = get(&app, "/v1/kube-clusters?q=sta").await;
    assert_eq!(slugs(&page), ["staging"]);
    let (status, _, body) = get(&app, "/v1/kube-clusters?name=staging").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // Read by identifier with its ETag; absent before its creation.
    let one = format!("/v1/kube-clusters/{id}");
    let (status, headers, read) = get(&app, &one).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(&read, item);
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    let (status, _, _) = get(&app, &format!("{one}?valid_at={}", rfc3339(before))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let created = OffsetDateTime::now_utc();

    // Changed under If-Match only, in JSON Merge Patch; a stale version refused.
    let patch = json!({"motive": "keep longer", "retention_days": 60});
    let merge = "application/merge-patch+json";
    let (status, _, body) = call(&app, "PATCH", &one, Some((merge, patch.clone())), &[]).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED, "{body}");
    let (status, _, body) = call(
        &app,
        "PATCH",
        &one,
        Some((merge, json!({"motive": "drop", "pod_domain": null}))),
        &[("If-Match", etag.clone())],
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (status, _, body) = call(
        &app,
        "PATCH",
        &one,
        Some((merge, patch.clone())),
        &[("If-Match", etag.clone())],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, body) = call(
        &app,
        "PATCH",
        &one,
        Some((merge, patch)),
        &[("If-Match", etag)],
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "{body}");
    let (_, headers, read) = get(&app, &one).await;
    assert_eq!(read["retention_days"], 60);
    // As asserted before the change, the value of then.
    let (status, _, then) = get(&app, &format!("{one}?as_of={}", rfc3339(created))).await;
    assert_eq!(status, StatusCode::OK, "{then}");
    assert_eq!(then["retention_days"], 14);

    // Closed under If-Match: no longer read, no longer listed.
    let etag = headers[header::ETAG].to_str().unwrap().to_owned();
    let (status, _, body) = call(
        &app,
        "DELETE",
        &one,
        Some(("application/json", json!({"motive": "decommissioned"}))),
        &[("If-Match", etag)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, _) = get(&app, &one).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, _, page) = get(&app, "/v1/kube-clusters").await;
    assert_eq!(slugs(&page), ["prod"]);
}

/// REQ-019-38, ADR 0021: a cluster the caller may not read is not listed and is answered
/// as an absent one, 404.
#[kartenn_testing::pg_test]
async fn cluster_unreadable_is_absent(pool: PgPool) {
    let env = Env::new(&pool).await;
    let nobody = person(&pool, &["core.entity.read"]).await;
    let app = app(&pool, principal(nobody, IdentityKind::Person), None);
    let (status, _, page) = get(&app, "/v1/kube-clusters").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["items"], json!([]));
    let (status, _, _) = get(&app, &format!("/v1/kube-clusters/{}", env.prod)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = get(&app, &format!("/v1/kube-clusters/{}", Uuid::now_v7())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let reader = person(&pool, &["kube.cluster.read"]).await;
    let app = super::app(&pool, principal(reader, IdentityKind::Person), None);
    let (_, _, page) = get(&app, "/v1/kube-clusters").await;
    assert_eq!(slugs(&page), ["prod"]);
}
