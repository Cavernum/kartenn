//! The objects of the core through the router (spec 006, REQ-006-1, 2, 4, 23; task
//! 006-12a): the natural key as a filter, a summary per linked object, the URL kept by a
//! rename, the history, and the same 404 for an invisible object as for an absent one;
//! the tree under a prefix (task 006-12b): what the caller may not read is absent; the
//! anomalies and their acknowledgement (task 006-12c); children and history by pages
//! (task 006-12d).

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
        sources: kartenn_service::expected::Sources::none(),
        outputs: std::sync::Arc::from([]),
        modules: std::sync::Arc::default(),
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

/// An address of `domain`.
async fn address(pool: &PgPool, domain: Uuid, value: &str) -> Uuid {
    let sql = format!(
        "INSERT INTO address (id, domain_id, address, usage, public, registry_version,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1::uuid, $2::inet, 'gateway', false, 'test', {SINCE})
         RETURNING id"
    );
    insert(pool, &sql, &[&domain.to_string(), value]).await
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn children_and_history_page_by_cursor(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let block = prefix_of(&pool, lab, "10.1.0.0/16", "container").await;
    let first = address(&pool, lab, "10.1.0.5").await;
    let net = prefix(&pool, lab, "10.1.2.0/24").await;
    address(&pool, lab, "10.1.2.1").await;
    let other = prefix(&pool, lab, "10.1.3.0/24").await;
    let last = address(&pool, lab, "10.1.4.1").await;
    let app = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);

    // Children by two, in address order; a child created between two pages appears in
    // neither: the cursor froze `as_of`.
    let body = ok(&app, &format!("/v1/prefixes/{block}/children?page_size=2")).await;
    assert_eq!(ids_of(&body), [first.to_string(), net.to_string()]);
    let next = body["links"]["next"].as_str().unwrap().to_owned();
    assert!(next.starts_with(&format!("/v1/prefixes/{block}/children?cursor=")));
    prefix(&pool, lab, "10.1.1.0/24").await;
    let body = ok(&app, &next).await;
    assert_eq!(ids_of(&body), [other.to_string(), last.to_string()]);
    assert_eq!(body["links"]["next"], Value::Null);

    // An altered cursor, or one of another list, is refused.
    let (path, cursor) = next.split_once("cursor=").unwrap();
    let altered = format!("{path}cursor=A{}", &cursor[1..]);
    assert_eq!(get(&app, &altered).await.status(), StatusCode::BAD_REQUEST);
    let elsewhere = format!("/v1/prefixes/{net}/children?cursor={cursor}");
    assert_eq!(
        get(&app, &elsewhere).await.status(),
        StatusCode::BAD_REQUEST
    );
    let filtered = format!("/v1/prefixes/{block}/children?nature=network");
    assert_eq!(get(&app, &filtered).await.status(), StatusCode::BAD_REQUEST);

    // The history by two: three versions of a domain renamed twice.
    for slug in ["core", "edge"] {
        sqlx::query("UPDATE addressing_domain SET slug = $2, changeset_id = gen_random_uuid() WHERE id = $1")
            .bind(lab)
            .bind(slug)
            .execute(&pool)
            .await
            .unwrap();
    }
    let body = ok(&app, &format!("/v1/domains/{lab}/history?page_size=2")).await;
    let slugs = |body: &Value| -> Vec<String> {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["object"]["slug"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(slugs(&body), ["lab", "core"]);
    let next = body["links"]["next"].as_str().unwrap().to_owned();
    assert!(next.starts_with(&format!("/v1/domains/{lab}/history?cursor=")));
    let body = ok(&app, &next).await;
    assert_eq!(slugs(&body), ["edge"]);
    assert_eq!(body["links"]["next"], Value::Null);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn history_keeps_the_versions_valid_at_and_known_as_of(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let now = || async {
        let at: time::OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&pool)
            .await
            .unwrap();
        at.format(&time::format_description::well_known::Rfc3339)
            .unwrap()
            .replace('+', "%2B")
    };
    let before = now().await;
    // From 2025 on, the domain is named `wan`: its first version is retracted.
    sqlx::query(
        "UPDATE addressing_domain
         SET valid_period = tstzrange('2020-01-01', '2025-01-01'),
             changeset_id = gen_random_uuid()
         WHERE id = $1",
    )
    .bind(lab)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id)
         VALUES ($1, 'wan', tstzrange('2025-01-01', NULL), gen_random_uuid())",
    )
    .bind(lab)
    .execute(&pool)
    .await
    .unwrap();
    let app = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);
    let uri = format!("/v1/domains/{lab}/history");
    let slugs = |body: &Value| -> Vec<String> {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["object"]["slug"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(slugs(&ok(&app, &uri).await), ["lab", "lab", "wan"]);

    // `valid_at` keeps the versions valid then, on every page.
    let body = ok(
        &app,
        &format!("{uri}?valid_at=2024-06-01T00:00:00Z&page_size=1"),
    )
    .await;
    assert_eq!(slugs(&body), ["lab"]);
    let body = ok(&app, body["links"]["next"].as_str().unwrap()).await;
    assert_eq!(slugs(&body), ["lab"]);
    assert_eq!(body["links"]["next"], Value::Null);
    let body = ok(&app, &format!("{uri}?valid_at=2025-06-01T00:00:00Z")).await;
    assert_eq!(slugs(&body), ["lab", "wan"]);

    // As known before the change: one version, still asserted then.
    let body = ok(&app, &format!("{uri}?as_of={before}")).await;
    assert_eq!(slugs(&body), ["lab"]);
    assert_eq!(body["items"][0]["asserted_to"], Value::Null);
    assert_eq!(body["items"][0]["valid_to"], Value::Null);
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

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_roots_of_a_domain_list_as_the_children_of_a_prefix(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let dmz = domain(&pool, "dmz").await;
    let stray = address(&pool, lab, "10.0.0.1").await;
    let block = prefix_of(&pool, lab, "10.1.0.0/16", "container").await;
    prefix(&pool, lab, "10.1.2.0/24").await;
    address(&pool, lab, "10.1.2.1").await;
    let range = format!(
        "INSERT INTO ip_range (id, domain_id, range, usage, public, registry_version,
                               valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1::uuid, inetrange('10.2.0.10', '10.2.0.20', '[]'),
                 'dhcp', false, 'test', {SINCE}) RETURNING id"
    );
    let range = insert(&pool, &range, &[&lab.to_string()]).await;
    let v6 = prefix_of(&pool, lab, "2001:db8::/32", "container").await;
    prefix(&pool, dmz, "10.9.0.0/24").await;
    let reader = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);

    // Nodes without a parent of the domain, in the order of the children of a prefix, by
    // pages; those of another domain or under a prefix are absent.
    let body = ok(&reader, &format!("/v1/domains/{lab}/roots?page_size=2")).await;
    assert_eq!(ids_of(&body), [stray.to_string(), block.to_string()]);
    assert_eq!(body["items"][1]["kind"], "prefix");
    let next = body["links"]["next"].as_str().unwrap().to_owned();
    assert!(next.starts_with(&format!("/v1/domains/{lab}/roots?cursor=")));
    // A root created between two pages appears in neither: the cursor froze `as_of`.
    address(&pool, lab, "10.3.0.1").await;
    let body = ok(&reader, &next).await;
    assert_eq!(ids_of(&body), [range.to_string(), v6.to_string()]);
    assert_eq!(body["links"]["next"], Value::Null);
    // A cursor of the children of a prefix is refused here.
    let (_, cursor) = next.split_once("cursor=").unwrap();
    let elsewhere = format!("/v1/prefixes/{block}/children?cursor={cursor}");
    assert_eq!(
        get(&reader, &elsewhere).await.status(),
        StatusCode::BAD_REQUEST
    );

    // One by one, from root to root and through the gaps between them, the same list.
    let tail = prefix(&pool, lab, "10.4.0.0/24").await;
    let late = address(&pool, lab, "2001:db9::1").await;
    let uri = format!("/v1/domains/{lab}/roots");
    let all = ids_of(&ok(&reader, &uri).await);
    assert_eq!(all.len(), 7, "{all:?}");
    assert_eq!(
        all[4..],
        [tail.to_string(), v6.to_string(), late.to_string()]
    );
    assert_eq!(walk(&reader, &format!("{uri}?page_size=1")).await, all);

    // Dated: before its validity, or before kartenn asserted it, the domain is absent.
    for dates in [
        "valid_at=2019-01-01T00:00:00Z",
        "as_of=2020-01-01T00:00:00Z",
    ] {
        not_found(&reader, &format!("/v1/domains/{lab}/roots?{dates}")).await;
    }

    // Unreadable nodes are absent; an unreadable domain answers as an absent one.
    let partial = app(
        &pool,
        person(
            &pool,
            &["core.domain.read", "core.prefix.read", "core.range.read"],
            ("domain", Some(lab)),
        )
        .await,
    );
    assert_eq!(
        ids_of(&ok(&partial, &format!("/v1/domains/{lab}/roots")).await),
        [
            block.to_string(),
            range.to_string(),
            tail.to_string(),
            v6.to_string()
        ]
    );
    assert_eq!(
        walk(&partial, &format!("{uri}?page_size=1")).await,
        [
            block.to_string(),
            range.to_string(),
            tail.to_string(),
            v6.to_string()
        ]
    );
    // Unreadable root prefixes still bound the gaps: the addresses they hold stay out.
    let addresses = app(
        &pool,
        person(
            &pool,
            &["core.domain.read", "core.address.read"],
            ("domain", Some(lab)),
        )
        .await,
    );
    let outside = ids_of(&ok(&addresses, &uri).await);
    assert_eq!(outside, [&*all[0], &*all[3], &*all[6]]);
    assert_eq!(
        walk(&addresses, &format!("{uri}?page_size=1")).await,
        outside
    );
    let absent = not_found(&partial, &format!("/v1/domains/{}/roots", Uuid::now_v7())).await;
    assert_eq!(
        not_found(&partial, &format!("/v1/domains/{dmz}/roots")).await,
        absent
    );
}

async fn acknowledge(app: &Router, id: Uuid, body: &str) -> Response {
    app.clone()
        .oneshot(common::bearer(
            Request::post(format!("/v1/anomalies/{id}/acknowledgement"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        ))
        .await
        .unwrap()
}

/// An open anomaly of `rule` on `object`, opened by a changeset of `author`.
async fn anomaly(pool: &PgPool, author: Uuid, object: Uuid, rule: &str) -> Uuid {
    let changeset: Uuid = sqlx::query_scalar(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
         VALUES (gen_random_uuid(), $1, 'm', 'applied', 'direct', tstzrange(now(), NULL))
         RETURNING id",
    )
    .bind(author)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO anomaly (rule_id, object_id, message, opened_by)
         VALUES ($1, $2, 'warning', $3) RETURNING id",
    )
    .bind(rule)
    .bind(object)
    .bind(changeset)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn anomalies_follow_the_visibility_of_their_object(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let dmz = domain(&pool, "dmz").await;
    let seen = prefix(&pool, lab, "10.1.2.0/24").await;
    let hidden = prefix(&pool, dmz, "10.9.0.0/24").await;
    let acknowledger = person(
        &pool,
        &["core.prefix.read", "core.anomaly.acknowledge"],
        ("domain", Some(lab)),
    )
    .await;
    let author = acknowledger.identity;
    let mine = anomaly(&pool, author, seen, "core.test.seen").await;
    let theirs = anomaly(&pool, author, hidden, "core.test.hidden").await;
    let other = anomaly(&pool, author, seen, "core.test.other").await;
    let reader = app(
        &pool,
        person(&pool, &["core.prefix.read"], ("domain", Some(lab))).await,
    );
    let app = app(&pool, acknowledger);

    let body = ok(&app, "/v1/anomalies?rule=core.test.seen").await;
    assert_eq!(ids_of(&body), [mine.to_string()]);
    assert_eq!(
        body["items"][0]["object"]["href"],
        format!("/v1/prefixes/{seen}")
    );
    assert_eq!(
        ids_of(&ok(&app, "/v1/anomalies").await),
        [mine.to_string(), other.to_string()],
        "invisible anomaly listed"
    );
    assert_eq!(
        get(&app, "/v1/anomalies?as_of=2020-01-01T00:00:00Z")
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );

    // An invisible anomaly answers as an absent one, whatever the body.
    let absent = acknowledge(&app, Uuid::now_v7(), "{}").await;
    assert_eq!(absent.status(), StatusCode::NOT_FOUND);
    let mut absent = json_of(absent).await;
    let hidden = acknowledge(&app, theirs, r#"{"motive":"known"}"#).await;
    assert_eq!(hidden.status(), StatusCode::NOT_FOUND);
    let mut hidden = json_of(hidden).await;
    absent.as_object_mut().unwrap().remove("instance");
    hidden.as_object_mut().unwrap().remove("instance");
    assert_eq!(hidden, absent);

    let missing = acknowledge(&app, mine, "{}").await;
    assert_eq!(missing.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let problem = json_of(missing).await;
    assert!(
        problem["type"]
            .as_str()
            .unwrap()
            .ends_with("motive-required"),
        "{problem}"
    );
    assert_eq!(
        acknowledge(&app, mine, r#"{"motive":"known"}"#)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        ids_of(&ok(&app, "/v1/anomalies").await),
        [other.to_string()]
    );
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event WHERE name = 'anomaly.state.acknowledged'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, 1, "acknowledgement not audited");
    let motive: String = sqlx::query_scalar("SELECT motive FROM anomaly WHERE id = $1")
        .bind(mine)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(motive, "known");

    // A reader who sees the object but may not acknowledge.
    assert_eq!(
        acknowledge(&reader, other, r#"{"motive":"known"}"#)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

/// Declares the heritable text property `key` on prefixes, sensitive or not.
async fn declare(pool: &PgPool, key: &str, sensitive: bool) -> Uuid {
    let sql = format!(
        "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers,
                                           type_params, inherit_tree, sensitive, valid_period,
                                           changeset_id)
         VALUES (gen_random_uuid(), $1, 'text', 'single', ARRAY['prefix'],
                 '{{\"min_len\": 1, \"max_len\": 64}}', 'addressing', $2::boolean, {SINCE})
         RETURNING id"
    );
    insert(pool, &sql, &[key, &sensitive.to_string()]).await
}

/// Assigns `value` to `property` on `holder`, the origin of itself and of `heirs`.
async fn assign(pool: &PgPool, property: Uuid, holder: Uuid, value: &str, heirs: &[Uuid]) {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO property_assignment (object_id, object_type, property_id, kind, v_text,
                                          provenance, valid_period, changeset_id)
         VALUES ($1, 'prefix', $2, 'value', $3, 'input', {SINCE})"
    )))
    .bind(holder)
    .bind(property)
    .bind(value)
    .execute(pool)
    .await
    .unwrap();
    for object in std::iter::once(&holder).chain(heirs) {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO property_origin (object_id, property_id, origin_object_id,
                                          valid_period, changeset_id)
             VALUES ($1, $2, $3, {SINCE})"
        )))
        .bind(object)
        .bind(property)
        .bind(holder)
        .execute(pool)
        .await
        .unwrap();
    }
}

fn query_date(at: time::OffsetDateTime) -> String {
    at.format(&time::format_description::well_known::Rfc3339)
        .unwrap()
        .replace('+', "%2B")
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn properties_resolve_explain_and_mask(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let top = prefix_of(&pool, lab, "10.0.0.0/8", "container").await;
    let block = prefix_of(&pool, lab, "10.1.0.0/16", "container").await;
    let net = prefix(&pool, lab, "10.1.2.0/24").await;
    let zone = declare(&pool, "policy.zone", false).await;
    let secret = declare(&pool, "site.secret", true).await;
    assign(&pool, zone, top, "red", &[block, net]).await;
    assign(&pool, secret, top, "s3", &[block, net]).await;
    // With the right to the sensitive value and to the whole tree.
    let mut rights = READ_ALL.to_vec();
    rights.push("prop.site.secret.read");
    let everything = app(&pool, person(&pool, &rights, ("global", None)).await);
    // Prefixes under the /16 only: the /8 that carries the values is invisible.
    let app = app(
        &pool,
        person(&pool, &["core.prefix.read"], ("prefix", Some(block))).await,
    );

    let body = ok(&app, &format!("/v1/prefixes/{net}/properties")).await;
    assert_eq!(
        body["properties"],
        json!({ "policy.zone": "red" }),
        "{body}"
    );
    assert_eq!(body["masked"], json!(["site.secret"]));

    let uri = format!("/v1/prefixes/{net}/properties/policy.zone/explanation");
    let body = ok(&app, &uri).await;
    assert_eq!(body["value"], "red", "{body}");
    assert_eq!(body["masked"], false);
    assert_eq!(body["provenance"], "input");
    // The origin and its place in the chain are hidden, not removed.
    assert_eq!(body["origin"], Value::Null);
    let chain = body["chain"].as_array().unwrap();
    assert_eq!(chain.len(), 3, "{body}");
    assert_eq!(chain[0]["id"], net.to_string());
    assert_eq!(chain[1]["href"], format!("/v1/prefixes/{block}"));
    assert_eq!(chain[2], Value::Null);

    let secret_uri = format!("/v1/prefixes/{net}/properties/site.secret/explanation");
    let body = ok(&app, &secret_uri).await;
    assert_eq!(body["value"], Value::Null, "{body}");
    assert_eq!(body["masked"], true);

    // The explanation as asserted before a change of the value.
    let before: time::OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE property_assignment SET v_text = 'blue', changeset_id = gen_random_uuid()
         WHERE object_id = $1 AND property_id = $2",
    )
    .bind(top)
    .bind(zone)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(ok(&app, &uri).await["value"], "blue");
    let past = format!("{uri}?as_of={}", query_date(before));
    assert_eq!(ok(&app, &past).await["value"], "red");

    let body = ok(&everything, &secret_uri).await;
    assert_eq!(body["value"], "s3", "{body}");
    assert_eq!(body["origin"]["id"], top.to_string());
    let disclosed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'data.sensitive.read'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(disclosed, 1);

    // The schema, for any caller.
    let body = ok(&app, "/v1/property-declarations?page_size=1").await;
    assert_eq!(body["items"][0]["key"], "policy.zone", "{body}");
    assert_eq!(body["items"][0]["inherit"], "addressing");
    let next = body["links"]["next"].as_str().unwrap().to_owned();
    assert_eq!(ok(&app, &next).await["items"][0]["key"], "site.secret");
    let values = get(&app, "/v1/property-declarations/policy.zone/values").await;
    assert_eq!(values.status(), StatusCode::CONFLICT);
    let unknown = get(&app, "/v1/property-declarations/policy.none/values").await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);

    // No source expects a value; a suggestion is never computed in the past.
    let suggestion = format!("/v1/prefixes/{net}/properties/policy.zone/suggestion");
    assert_eq!(ok(&app, &suggestion).await["suggestion"], Value::Null);
    let past = get(&app, &format!("{suggestion}?as_of={}", query_date(before))).await;
    assert_eq!(past.status(), StatusCode::BAD_REQUEST);
    // An invisible object, an object of another kind, an undeclared property: 404.
    not_found(
        &app,
        &format!("/v1/prefixes/{top}/properties/policy.zone/explanation"),
    )
    .await;
    not_found(
        &app,
        &format!("/v1/addresses/{net}/properties/policy.zone/explanation"),
    )
    .await;
    not_found(
        &app,
        &format!("/v1/prefixes/{net}/properties/policy.none/explanation"),
    )
    .await;
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_deviation_of_a_sensitive_property_hides_its_value(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let net = prefix(&pool, lab, "10.1.2.0/24").await;
    let other = prefix(&pool, lab, "10.1.3.0/24").await;
    declare(&pool, "site.secret", true).await;
    declare(&pool, "policy.zone", false).await;
    let author = person(&pool, &READ_ALL, ("global", None)).await.identity;
    // One open deviation per object (the rule and the object key an anomaly).
    for (net, message) in [
        (
            net,
            "site.secret: resolved s3, expected [Text(\"s4\")] by src@1 (rule)",
        ),
        (
            other,
            "policy.zone: resolved red, expected [Text(\"blue\")] by src@1 (rule)",
        ),
    ] {
        let id = anomaly(&pool, author, net, "prop.resolution.expected_mismatch").await;
        sqlx::query("UPDATE anomaly SET message = $2 WHERE id = $1")
            .bind(id)
            .bind(message)
            .execute(&pool)
            .await
            .unwrap();
    }
    let app = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);

    let body = ok(&app, "/v1/property-gaps").await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{body}");
    let message = |key: &str| {
        items
            .iter()
            .find(|i| i["property"] == key)
            .map(|i| i["message"].as_str().unwrap().to_owned())
            .unwrap()
    };
    assert!(!message("site.secret").contains("s3"), "{body}");
    assert!(message("policy.zone").contains("red"), "{body}");
    let body = ok(&app, "/v1/property-gaps?property=policy.zone").await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1, "{body}");
    // The list of the anomalies masks it as well.
    let body = ok(&app, "/v1/anomalies").await;
    assert!(!body.to_string().contains("s3"), "{body}");
    let unknown = get(&app, "/v1/property-gaps?rule=x").await;
    assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
}

/// The identifiers of every page of the list at `uri`, following `next`.
async fn walk(app: &Router, uri: &str) -> Vec<String> {
    let (mut ids, mut uri) = (Vec::new(), uri.to_owned());
    loop {
        let body = ok(app, &uri).await;
        assert!(body["items"].as_array().unwrap().len() <= 1, "{body}");
        ids.extend(ids_of(&body));
        match body["links"]["next"].as_str() {
            Some(next) => next.clone_into(&mut uri),
            None => return ids,
        }
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn properties_filter_the_objects_of_one_domain(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let dmz = domain(&pool, "dmz").await;
    let env = declare(&pool, "policy.env", false).await;
    let zone = declare(&pool, "policy.zone", false).await;
    let a = prefix(&pool, lab, "10.1.0.0/24").await;
    let b = prefix(&pool, lab, "10.2.0.0/24").await;
    let c = prefix(&pool, lab, "10.3.0.0/24").await;
    let d = prefix(&pool, lab, "10.4.0.0/24").await;
    let elsewhere = prefix(&pool, dmz, "10.1.0.0/24").await;
    let gateway = address(&pool, lab, "10.1.0.1").await;
    assign(&pool, env, a, "prod", &[gateway]).await;
    assign(&pool, env, b, "test", &[]).await;
    assign(&pool, env, c, "dev", &[]).await;
    assign(&pool, env, d, "prod", &[]).await;
    assign(&pool, env, elsewhere, "prod", &[]).await;
    assign(&pool, zone, a, "red", &[]).await;
    assign(&pool, zone, b, "red", &[]).await;
    assign(&pool, zone, d, "blue", &[]).await;
    let app = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);
    let sorted = |mut ids: Vec<Uuid>| {
        ids.sort();
        ids.iter().map(Uuid::to_string).collect::<Vec<_>>()
    };

    // One of the values, in one domain, of the kind of the collection, page after page by
    // identifier.
    let uri = "/v1/prefixes?domain=lab&prop.policy.env=prod&prop.policy.env=test&page_size=1";
    assert_eq!(walk(&app, uri).await, sorted(vec![a, b, d]));
    let body = ok(&app, "/v1/prefixes?domain=lab&prop.policy.env=dev").await;
    assert_eq!(body["items"][0]["value"], "10.3.0.0/24", "{body}");
    assert_eq!(body["items"][0]["domain"]["slug"], "lab", "{body}");
    let body = ok(&app, "/v1/addresses?domain=lab&prop.policy.env=prod").await;
    assert_eq!(ids_of(&body), vec![gateway.to_string()]);

    // Conditions are intersected.
    let uri = "/v1/prefixes?domain=lab&prop.policy.env=prod&prop.policy.env=test\
               &prop.policy.zone=red&page_size=1";
    assert_eq!(walk(&app, uri).await, sorted(vec![a, b]));
    let uri = "/v1/prefixes?domain=lab&prop.policy.zone:exists=false";
    assert_eq!(ids_of(&ok(&app, uri).await), vec![c.to_string()]);

    // The values of the natural key are intersected as well.
    let uri = "/v1/prefixes?domain=lab&prop.policy.env=prod&prop.policy.env=test\
               &value=10.1.0.0/24&value=10.3.0.0/24&value=10.4.0.0/24&page_size=1";
    assert_eq!(walk(&app, uri).await, sorted(vec![a, d]));
    let uri = "/v1/addresses?domain=lab&prop.policy.env=prod&value=10.1.0.1";
    assert_eq!(ids_of(&ok(&app, uri).await), vec![gateway.to_string()]);
    let uri = "/v1/addresses?domain=lab&prop.policy.env=prod&value=10.1.0.2";
    assert_eq!(ids_of(&ok(&app, uri).await), Vec::<String>::new());

    // A next page reads the current state: an object assigned between two pages is in it
    // when its identifier comes after the cursor (REQ-006-7, exception).
    let body = ok(
        &app,
        "/v1/prefixes?domain=lab&prop.policy.env=prod&page_size=1",
    )
    .await;
    let first = ids_of(&body);
    let next = body["links"]["next"].as_str().unwrap().to_owned();
    let e = prefix(&pool, lab, "10.5.0.0/24").await;
    assign(&pool, env, e, "prod", &[]).await;
    let rest: Vec<String> = sorted(vec![a, d, e])
        .into_iter()
        .filter(|id| id > &first[0])
        .collect();
    assert_eq!(walk(&app, &next).await, rest);

    // An unknown domain holds nothing.
    let body = ok(&app, "/v1/prefixes?domain=core&prop.policy.env=prod").await;
    assert_eq!(ids_of(&body), Vec::<String>::new());

    // Without one domain, on entities, in the past, undeclared, mistyped:
    // 400 naming the parameter.
    for (uri, named) in [
        ("/v1/prefixes?prop.policy.env=prod", "domain"),
        (
            "/v1/prefixes?domain=lab&domain=dmz&prop.policy.env=prod",
            "domain",
        ),
        ("/v1/entities?prop.policy.env=prod", "prop.policy.env"),
        (
            "/v1/prefixes?domain=lab&prop.policy.env=prod&as_of=2025-01-01T00:00:00Z",
            "as_of",
        ),
        ("/v1/prefixes?domain=lab&prop.dns.ttl=1", "prop.dns.ttl"),
        (
            "/v1/prefixes?domain=lab&prop.policy.env:exists=maybe",
            "prop.policy.env",
        ),
    ] {
        let response = get(&app, uri).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        let detail = json_of(response).await["detail"].to_string();
        assert!(detail.contains(named), "{uri}: {detail}");
    }
}
