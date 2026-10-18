//! The objects of the core through the router (spec 006, REQ-006-1, 2, 4, 23; task
//! 006-12a): the natural key as a filter, a summary per linked object, the URL kept by a
//! rename, the history, and the same 404 for an invisible object as for an absent one;
//! the tree under a prefix (task 006-12b): what the caller may not read is absent.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
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

/// A person without issuer (no confinement) holding `permissions` on `scope`
/// (`("global", None)`, `("domain", Some(id))`…).
async fn person(pool: &PgPool, permissions: &[&str], scope: (&str, Option<Uuid>)) -> Principal {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, label, valid_period, changeset_id)
         VALUES ($1, 'reader_' || replace($1::text, '-', ''), 'reader',
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
         VALUES (gen_random_uuid(), 'identity', $1, $2, $3, $4,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(identity)
    .bind(role)
    .bind(scope.0)
    .bind(scope.1)
    .execute(pool)
    .await
    .unwrap();
    Principal {
        identity,
        kind: IdentityKind::Person,
        issuer: None,
        groups: Vec::new(),
        method: Method::Bearer,
    }
}

const READ_ALL: [&str; 6] = [
    "core.entity.read",
    "core.site.read",
    "core.domain.read",
    "core.prefix.read",
    "core.range.read",
    "core.address.read",
];

fn app(pool: &PgPool, who: Principal) -> Router {
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
    };
    kartenn_rest::router(
        kartenn_rest::v1(api),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates::default())),
        Arc::new(common::As(who)),
    )
}

async fn get(app: &Router, uri: &str) -> Response {
    app.clone()
        .oneshot(common::bearer(
            Request::get(uri).body(Body::empty()).unwrap(),
        ))
        .await
        .unwrap()
}

async fn json_of(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn ok(app: &Router, uri: &str) -> Value {
    let response = get(app, uri).await;
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    json_of(response).await
}

async fn insert(pool: &PgPool, sql: &str, binds: &[&str]) -> Uuid {
    let mut query = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()));
    for bind in binds {
        query = query.bind(*bind);
    }
    query.fetch_one(pool).await.unwrap()
}

const SINCE: &str = "tstzrange('2020-01-01', NULL), gen_random_uuid()";

async fn domain(pool: &PgPool, slug: &str) -> Uuid {
    let sql = format!(
        "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1, {SINCE}) RETURNING id"
    );
    insert(pool, &sql, &[slug]).await
}

async fn prefix(pool: &PgPool, domain: Uuid, value: &str) -> Uuid {
    prefix_of(pool, domain, value, "network").await
}

async fn prefix_of(pool: &PgPool, domain: Uuid, value: &str, nature: &str) -> Uuid {
    let sql = format!(
        "INSERT INTO prefix (id, domain_id, prefix, nature, public, registry_version,
                             valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1::uuid, $2::cidr, $3, false, 'test', {SINCE})
         RETURNING id"
    );
    insert(pool, &sql, &[&domain.to_string(), value, nature]).await
}

async fn node(pool: &PgPool, table: &str, slug: &str, parent: Option<Uuid>) -> Uuid {
    let sql = format!(
        "INSERT INTO {table} (id, slug, parent_id, valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1, $2::uuid, {SINCE}) RETURNING id"
    );
    let parent = parent.map(|p| p.to_string());
    let mut query = sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).bind(slug);
    query = query.bind(parent);
    query.fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_natural_key_is_a_filter(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let dmz = domain(&pool, "dmz").await;
    let wanted = prefix(&pool, lab, "10.1.2.0/24").await;
    prefix(&pool, lab, "10.1.3.0/24").await;
    prefix(&pool, dmz, "10.1.2.0/24").await;
    let app = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);

    let body = ok(&app, "/v1/prefixes?domain=lab&value=10.1.2.0/24").await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{body}");
    let found = &items[0];
    assert_eq!(found["id"], wanted.to_string());
    assert_eq!(found["links"]["self"], format!("/v1/prefixes/{wanted}"));
    assert_eq!(found["value"], "10.1.2.0/24");
    assert_eq!(found["domain"]["slug"], "lab");
    let linked = ok(&app, found["domain"]["href"].as_str().unwrap()).await;
    assert_eq!(linked["id"], lab.to_string());

    // A repeated value is one of them; sorted by domain, value, identifier.
    let body = ok(&app, "/v1/prefixes?value=10.1.2.0/24&domain=lab&domain=dmz").await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2, "{body}");
    let body = ok(&app, "/v1/domains?slug=dmz").await;
    assert_eq!(body["items"][0]["id"], dmz.to_string());

    // A malformed value, an unknown parameter: 400 naming it.
    for (uri, named) in [
        ("/v1/prefixes?value=10.1.2.1/24", "value"),
        ("/v1/domains?domain=lab", "domain"),
    ] {
        let response = get(&app, uri).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        let detail = json_of(response).await["detail"].to_string();
        assert!(detail.contains(named), "{uri}: {detail}");
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn every_collection_reads_with_its_summaries(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    prefix(&pool, lab, "10.1.2.0/24").await;
    let range = format!(
        "INSERT INTO ip_range (id, domain_id, range, usage, public, registry_version,
                               valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1::uuid, inetrange('10.1.2.10', '10.1.2.20', '[]'),
                 'dhcp', false, 'test', {SINCE}) RETURNING id"
    );
    let range = insert(&pool, &range, &[&lab.to_string()]).await;
    let address = format!(
        "INSERT INTO address (id, domain_id, address, usage, public, registry_version,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1::uuid, '10.1.2.1', 'gateway', false, 'test', {SINCE})
         RETURNING id"
    );
    let address = insert(&pool, &address, &[&lab.to_string()]).await;
    let group = node(&pool, "entity", "group", None).await;
    let team = node(&pool, "entity", "team", Some(group)).await;
    let site = node(&pool, "site", "north", None).await;
    let app = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);

    let body = ok(&app, &format!("/v1/ranges/{range}")).await;
    assert_eq!(
        (body["first"].as_str(), body["last"].as_str()),
        (Some("10.1.2.10"), Some("10.1.2.20"))
    );
    assert_eq!(body["domain"]["id"], lab.to_string());
    let body = ok(&app, "/v1/ranges?domain=lab&first=10.1.2.10").await;
    assert_eq!(body["items"][0]["id"], range.to_string());
    let body = ok(&app, "/v1/addresses?value=10.1.2.1").await;
    assert_eq!(body["items"][0]["id"], address.to_string());
    assert_eq!(body["items"][0]["usage"], "gateway");
    let body = ok(&app, "/v1/entities?parent=group").await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1, "{body}");
    assert_eq!(body["items"][0]["id"], team.to_string());
    assert_eq!(body["items"][0]["parent"]["slug"], "group");
    let body = ok(&app, &format!("/v1/entities/{group}")).await;
    assert_eq!(body["parent"], Value::Null);
    let body = ok(&app, "/v1/sites").await;
    assert_eq!(body["items"][0]["id"], site.to_string());
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_rename_keeps_the_url_and_makes_history(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let id = prefix(&pool, lab, "10.1.2.0/24").await;
    let app = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);
    let uri = format!("/v1/prefixes/{id}");
    assert_eq!(ok(&app, &uri).await["domain"]["slug"], "lab");

    sqlx::query("UPDATE addressing_domain SET slug = 'core', changeset_id = gen_random_uuid() WHERE id = $1")
        .bind(lab)
        .execute(&pool)
        .await
        .unwrap();
    let body = ok(&app, &uri).await;
    assert_eq!(body["domain"]["slug"], "core");
    assert_eq!(body["links"]["self"], uri);

    let history = ok(&app, &format!("/v1/domains/{lab}/history")).await;
    let versions = history["items"].as_array().unwrap();
    assert_eq!(versions.len(), 2, "{history}");
    assert_eq!(versions[0]["object"]["slug"], "lab");
    assert!(versions[0]["asserted_to"].is_string(), "{history}");
    assert_eq!(versions[1]["object"]["slug"], "core");
    assert_eq!(versions[1]["asserted_to"], Value::Null);
    assert_eq!(versions[1]["valid_to"], Value::Null);
    assert!(
        versions[1]["changeset"]["href"]
            .as_str()
            .unwrap()
            .starts_with("/v1/changesets/")
    );

    // Every read is audited.
    let reads: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'data.objects.read'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(reads >= 3, "{reads}");
}

/// A 404 without what differs from one request to another.
async fn not_found(app: &Router, uri: &str) -> Value {
    let response = get(app, uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
    let mut body = json_of(response).await;
    body.as_object_mut().unwrap().remove("instance");
    body
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_invisible_object_answers_as_an_absent_one(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let dmz = domain(&pool, "dmz").await;
    let seen = prefix(&pool, lab, "10.1.2.0/24").await;
    let hidden = prefix(&pool, dmz, "10.9.0.0/24").await;
    let app = app(
        &pool,
        person(&pool, &["core.prefix.read"], ("domain", Some(lab))).await,
    );

    ok(&app, &format!("/v1/prefixes/{seen}")).await;
    let absent = not_found(&app, &format!("/v1/prefixes/{}", Uuid::now_v7())).await;
    assert_eq!(
        not_found(&app, &format!("/v1/prefixes/{hidden}")).await,
        absent
    );
    assert_eq!(
        not_found(&app, &format!("/v1/prefixes/{hidden}/history")).await,
        absent
    );
    assert_eq!(
        not_found(&app, &format!("/v1/prefixes/{}/history", Uuid::now_v7())).await,
        absent
    );
    // An object under the URL of another kind is absent there.
    assert_eq!(
        not_found(&app, &format!("/v1/addresses/{seen}")).await,
        absent
    );

    let body = ok(&app, "/v1/prefixes").await;
    let ids: Vec<&str> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [seen.to_string()]);
}

/// The identifiers of the items of a list.
fn ids_of(body: &Value) -> Vec<String> {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_tree_shows_only_what_the_caller_may_read(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let top = prefix_of(&pool, lab, "10.0.0.0/8", "container").await;
    let block = prefix_of(&pool, lab, "10.1.0.0/16", "container").await;
    let net = prefix(&pool, lab, "10.1.2.0/24").await;
    let other = prefix(&pool, lab, "10.1.3.0/24").await;
    let range = format!(
        "INSERT INTO ip_range (id, domain_id, range, usage, public, registry_version,
                               valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1::uuid, inetrange('10.1.2.100', '10.1.2.199', '[]'),
                 'dhcp', false, 'test', {SINCE}) RETURNING id"
    );
    let range = insert(&pool, &range, &[&lab.to_string()]).await;
    let address = format!(
        "INSERT INTO address (id, domain_id, address, usage, public, registry_version,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1::uuid, '10.1.2.1', 'gateway', false, 'test', {SINCE})
         RETURNING id"
    );
    insert(&pool, &address, &[&lab.to_string()]).await;
    // Prefixes and ranges under the /16, no address.
    let app = app(
        &pool,
        person(
            &pool,
            &["core.prefix.read", "core.range.read"],
            ("prefix", Some(block)),
        )
        .await,
    );

    let body = ok(&app, &format!("/v1/prefixes/{block}")).await;
    assert_eq!(
        body["links"]["children"],
        format!("/v1/prefixes/{block}/children")
    );
    assert_eq!(
        ids_of(&ok(&app, &format!("/v1/prefixes/{block}/children")).await),
        [net.to_string(), other.to_string()]
    );
    let body = ok(&app, &format!("/v1/prefixes/{net}/children")).await;
    assert_eq!(
        ids_of(&body),
        [range.to_string()],
        "invisible address listed"
    );
    assert_eq!(body["items"][0]["kind"], "range");
    assert_eq!(body["items"][0]["href"], format!("/v1/ranges/{range}"));
    // The invisible /8 is not an ancestor the caller sees.
    assert_eq!(
        ids_of(&ok(&app, &format!("/v1/prefixes/{net}/ancestors")).await),
        [block.to_string()]
    );

    // Occupancy and next free values count the whole content, the address included.
    let body = ok(&app, &format!("/v1/prefixes/{net}/occupancy")).await;
    assert_eq!(
        (&body["used"], &body["total"]),
        (&json!("101"), &json!("254"))
    );
    let body = ok(&app, &format!("/v1/prefixes/{block}/occupancy")).await;
    assert_eq!(
        (&body["used"], &body["total"]),
        (&json!("512"), &json!("65536"))
    );
    let body = ok(&app, &format!("/v1/prefixes/{net}/next-free?kind=address")).await;
    assert_eq!(body["value"], "10.1.2.2");
    let uri = format!("/v1/prefixes/{block}/next-free?kind=prefix&length=24");
    assert_eq!(ok(&app, &uri).await["value"], "10.1.0.0/24");
    for (uri, status) in [
        (
            format!("/v1/prefixes/{net}/next-free?kind=prefix&length=26"),
            StatusCode::CONFLICT,
        ),
        (
            format!("/v1/prefixes/{block}/next-free?kind=address"),
            StatusCode::CONFLICT,
        ),
        (
            format!("/v1/prefixes/{block}/next-free?kind=prefix&length=16"),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("/v1/prefixes/{block}/next-free?kind=prefix"),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        assert_eq!(get(&app, &uri).await.status(), status, "{uri}");
    }

    // An invisible prefix answers as an absent one on every route of the tree, and so
    // does a prefix before its validity or before kartenn asserted it.
    for route in [
        "children",
        "ancestors",
        "occupancy",
        "next-free?kind=prefix&length=24",
    ] {
        let absent = not_found(&app, &format!("/v1/prefixes/{}/{route}", Uuid::now_v7())).await;
        assert_eq!(
            not_found(&app, &format!("/v1/prefixes/{top}/{route}")).await,
            absent,
            "{route}"
        );
    }
    not_found(
        &app,
        &format!("/v1/prefixes/{block}/children?valid_at=2019-01-01T00:00:00Z"),
    )
    .await;
    not_found(
        &app,
        &format!("/v1/prefixes/{block}/children?as_of=2020-01-01T00:00:00Z"),
    )
    .await;
}
