//! The objects of the core through the router (spec 006, REQ-006-1, 2, 4, 23; task
//! 006-12a): the natural key as a filter, a summary per linked object, the URL kept by a
//! rename, the history, and the same 404 for an invisible object as for an absent one;
//! the tree under a prefix (task 006-12b): what the caller may not read is absent; the
//! anomalies and their acknowledgement (task 006-12c); children and history by pages
//! (task 006-12d).

use crate::common;

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
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, 'reader_' || replace($1::text, '-', ''), jsonb_build_object('fr', 'reader'),
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
    sqlx::query(kartenn_testing::BIND_IDENTITY)
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
    app_bounded(pool, who, Duration::from_secs(60))
}

/// [`app`], the writes bounded by `bound`: past it, a read at `as_of` is frozen.
fn app_bounded(pool: &PgPool, who: Principal, bound: Duration) -> Router {
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound,
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

#[kartenn_testing::pg_test]
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

/// `within`, `nature` and `usage` filter the objects of a domain (design « Filtres »,
/// task 021-28): inclusion in a CIDR (the prefix itself included, both bounds of a
/// range), one of the values; with a property filter, on the page of the engine.
#[kartenn_testing::pg_test]
async fn within_nature_and_usage_filter_the_lists(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let container = prefix_of(&pool, lab, "10.1.0.0/16", "container").await;
    let inside = prefix(&pool, lab, "10.1.2.0/24").await;
    let outside = prefix(&pool, lab, "10.2.0.0/24").await;
    let gateway = address(&pool, lab, "10.1.2.1").await;
    address(&pool, lab, "10.2.0.1").await;
    let host = insert(
        &pool,
        &format!(
            "INSERT INTO address (id, domain_id, address, usage, public, registry_version,
                                  valid_period, changeset_id)
             VALUES (gen_random_uuid(), $1::uuid, '10.1.2.2', 'host', false, 'test', {SINCE})
             RETURNING id"
        ),
        &[&lab.to_string()],
    )
    .await;
    let range = |text: &'static str| {
        let pool = pool.clone();
        async move {
            insert(
                &pool,
                &format!(
                    "INSERT INTO ip_range (id, domain_id, range, usage, public,
                                           registry_version, valid_period, changeset_id)
                     VALUES (gen_random_uuid(), $1::uuid, $2::inetrange, 'dhcp', false,
                             'test', {SINCE})
                     RETURNING id"
                ),
                &[&lab.to_string(), text],
            )
            .await
        }
    };
    let held = range("[10.1.2.10,10.1.2.20]").await;
    range("[10.1.255.250,10.2.0.5]").await;
    let env = declare(&pool, "policy.env", false).await;
    assign(&pool, env, container, "prod", &[]).await;
    assign(&pool, env, outside, "prod", &[]).await;
    let app = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);
    let ids = |body: &Value| {
        let mut ids = ids_of(body);
        ids.sort();
        ids
    };
    let sorted = |mut ids: Vec<Uuid>| {
        ids.sort();
        ids.iter().map(Uuid::to_string).collect::<Vec<_>>()
    };

    let body = ok(&app, "/v1/prefixes?within=10.1.0.0/16").await;
    assert_eq!(ids(&body), sorted(vec![container, inside]), "{body}");
    let body = ok(&app, "/v1/prefixes?within=10.1.0.0/16&nature=network").await;
    assert_eq!(ids(&body), sorted(vec![inside]), "{body}");
    let body = ok(&app, "/v1/prefixes?nature=container&nature=network").await;
    assert_eq!(ids(&body).len(), 3, "{body}");
    let body = ok(&app, "/v1/addresses?within=10.1.2.0/24").await;
    assert_eq!(ids(&body), sorted(vec![gateway, host]), "{body}");
    let body = ok(&app, "/v1/addresses?within=10.1.0.0/16&usage=host").await;
    assert_eq!(ids(&body), sorted(vec![host]), "{body}");
    let body = ok(&app, "/v1/ranges?within=10.1.0.0/16").await;
    assert_eq!(ids(&body), sorted(vec![held]), "{body}");
    let body = ok(&app, "/v1/ranges?usage=static").await;
    assert_eq!(ids(&body), Vec::<String>::new(), "{body}");
    // Beside a property filter: the objects of the engine, then the fields of the core.
    let body = ok(
        &app,
        "/v1/prefixes?domain=lab&prop.policy.env=prod&within=10.1.0.0/16",
    )
    .await;
    assert_eq!(ids(&body), sorted(vec![container]), "{body}");

    // A value out of the enumeration, a CIDR with host bits, a filter of another
    // collection: 400 naming it.
    for (uri, named) in [
        ("/v1/prefixes?nature=dhcp", "nature"),
        ("/v1/ranges?usage=gateway", "usage"),
        ("/v1/addresses?within=10.1.2.1/24", "within"),
        ("/v1/prefixes?usage=host", "usage"),
        ("/v1/entities?within=10.0.0.0/8", "within"),
    ] {
        let response = get(&app, uri).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        let detail = json_of(response).await["detail"].to_string();
        assert!(detail.contains(named), "{uri}: {detail}");
    }
}

#[kartenn_testing::pg_test]
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

/// Journals the changesets the versions of the domains name without the journal holding
/// them (the fixtures above draw them at random), by the system author: a version is
/// served with the motive and the author of its changeset.
async fn journal(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
         SELECT DISTINCT d.changeset_id, '00000000-0000-7000-8000-000000000001'::uuid, 'fixture',
                'applied', 'direct', tstzrange(now(), NULL)
         FROM (SELECT changeset_id FROM addressing_domain
               UNION SELECT changeset_id FROM addressing_domain_history) d
         WHERE NOT EXISTS (SELECT 1 FROM changeset c WHERE c.id = d.changeset_id)",
    )
    .execute(pool)
    .await
    .unwrap();
}

#[kartenn_testing::pg_test]
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

    journal(&pool).await;
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

#[kartenn_testing::pg_test]
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
    journal(&pool).await;
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

#[kartenn_testing::pg_test]
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
    journal(&pool).await;
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

/// The history of an object names the motive and the author of each changeset (task
/// 021-11p2, Q-021-24): the author read from the identities at each read, so that once
/// pseudonymised (REQ-004-33) the same history holds neither the name nor the email. A
/// reader of the object reads both, without `core.rbac.read` (Q-021-29, 30).
#[kartenn_testing::pg_test]
async fn history_names_the_motive_and_the_author_until_pseudonymised(pool: PgPool) {
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name, email)
         VALUES (gen_random_uuid(), 'Camille Martin', 'camille.martin@example.org')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let changeset: Uuid = sqlx::query_scalar(
        "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
         VALUES (gen_random_uuid(), $1, 'Bascule des VIP salle B', 'applied', 'direct',
                 tstzrange('2020-01-01', NULL))
         RETURNING id",
    )
    .bind(author)
    .fetch_one(&pool)
    .await
    .unwrap();
    let lab: Uuid = sqlx::query_scalar(
        "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'lab', tstzrange('2020-01-01', NULL), $1) RETURNING id",
    )
    .bind(changeset)
    .fetch_one(&pool)
    .await
    .unwrap();
    let asserted: time::OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let uri = format!("/v1/domains/{lab}/history");
    let past = format!(
        "{uri}?as_of={}",
        asserted
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
            .replace('+', "%2B")
    );
    // No bound on the writes: a second past `asserted`, a read at it is in the frozen
    // past, and would be `immutable` were its authors not read now.
    let reader = app_bounded(
        &pool,
        person(&pool, &["core.domain.read"], ("global", None)).await,
        Duration::ZERO,
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let before = get(&reader, &past).await;
    assert_eq!(before.status(), StatusCode::OK);
    assert_eq!(before.headers()["cache-control"], "private, no-cache");
    let tag = before.headers()["etag"].clone();
    assert!(json_of(before).await.to_string().contains("Camille Martin"));

    let body = ok(&reader, &uri).await;
    let version = &body["items"][0];
    assert_eq!(version["changeset"]["id"], changeset.to_string(), "{body}");
    assert_eq!(version["motive"], "Bascule des VIP salle B");
    assert_eq!(
        version["author"],
        json!({
            "id": author,
            "href": format!("/v1/identities/{author}"),
            "name": "Camille Martin",
        })
    );

    let officer = app(
        &pool,
        person(&pool, &["core.identity.pseudonymise"], ("global", None)).await,
    );
    let response = officer
        .clone()
        .oneshot(common::bearer(
            Request::post(format!("/v1/identities/{author}/pseudonymisation"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "motive": "droit à l'effacement" }).to_string(),
                ))
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = ok(&reader, &uri).await;
    let text = body.to_string();
    assert!(
        !text.contains("Camille") && !text.contains("Martin"),
        "{body}"
    );
    assert!(!text.contains("camille.martin@example.org"), "{body}");
    let pseudonym = format!("personne #{}", &author.to_string()[..8]);
    assert_eq!(body["items"][0]["author"]["name"], pseudonym.as_str());
    assert_eq!(body["items"][0]["author"]["id"], author.to_string());
    assert_eq!(body["items"][0]["motive"], "Bascule des VIP salle B");

    // The history at the same past `as_of` changed too: a new body, a new tag, no 304.
    let after = reader
        .clone()
        .oneshot(common::bearer(
            Request::get(&past)
                .header("if-none-match", tag.clone())
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(after.status(), StatusCode::OK);
    assert_eq!(after.headers()["cache-control"], "private, no-cache");
    assert_ne!(after.headers()["etag"], tag);
    let body = json_of(after).await;
    assert_eq!(
        body["items"][0]["author"]["name"],
        pseudonym.as_str(),
        "{body}"
    );
}

/// A 404 without what differs from one request to another.
async fn not_found(app: &Router, uri: &str) -> Value {
    let response = get(app, uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
    let mut body = json_of(response).await;
    body.as_object_mut().unwrap().remove("instance");
    body
}

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
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

/// Each node of a page of the tree carries its version, the one of its `ETag`, and a
/// prefix its occupancy, as `…/occupancy` serves it at the same moment (021/A11): for
/// the roots of a domain and the children of a prefix, now and dated; not the ancestors.
#[kartenn_testing::pg_test]
async fn tree_nodes_carry_their_version_and_occupancy(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    address(&pool, lab, "10.0.0.1").await;
    let block = prefix_of(&pool, lab, "10.1.0.0/16", "container").await;
    let net = prefix(&pool, lab, "10.1.2.0/24").await;
    let gone = address(&pool, lab, "10.1.2.1").await;
    address(&pool, lab, "10.1.2.15").await;
    let range = format!(
        "INSERT INTO ip_range (id, domain_id, range, usage, public, registry_version,
                               valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1::uuid, inetrange('10.1.2.10', '10.1.2.20', '[]'),
                 'dhcp', false, 'test', {SINCE}) RETURNING id"
    );
    insert(&pool, &range, &[&lab.to_string()]).await;
    let inner = prefix_of(&pool, lab, "10.1.4.0/22", "container").await;
    prefix(&pool, lab, "10.1.4.0/24").await;
    prefix(&pool, lab, "10.1.6.0/24").await;
    prefix(&pool, lab, "10.1.8.0/24").await;
    prefix_of(&pool, lab, "2001:db8::/32", "container").await;
    prefix(&pool, lab, "2001:db8:1::/48").await;
    let reader = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);

    // Known as of now: the read of the past, history included. The addresses added since
    // count in the present only; the one deleted since, kept in the history, still counts
    // as of then.
    let known = format!("?as_of={}", query_date(time::OffsetDateTime::now_utc()));
    address(&pool, lab, "10.1.2.2").await;
    address(&pool, lab, "10.1.2.3").await;
    sqlx::query("DELETE FROM address WHERE id = $1")
        .bind(gone)
        .execute(&pool)
        .await
        .unwrap();
    let mut prefixes = 0;
    for dates in ["", "?valid_at=2021-01-01T00:00:00Z", &known] {
        // The ancestors keep their form: neither version nor occupancy (A11 names the
        // roots and the children only).
        let body = ok(&reader, &format!("/v1/prefixes/{inner}/ancestors{dates}")).await;
        assert_eq!(ids_of(&body), [block.to_string()]);
        assert!(body["items"][0].get("version").is_none());
        assert!(body["items"][0].get("occupancy").is_none());
        for uri in [
            format!("/v1/domains/{lab}/roots{dates}"),
            format!("/v1/prefixes/{block}/children{dates}"),
            format!("/v1/prefixes/{inner}/children{dates}"),
        ] {
            let body = ok(&reader, &uri).await;
            let items = body["items"].as_array().unwrap();
            assert!(!items.is_empty(), "{uri}");
            for item in items {
                let href = item["href"].as_str().unwrap();
                let response = get(&reader, &format!("{href}{dates}")).await;
                assert_eq!(response.status(), StatusCode::OK, "{href}");
                let etag = response.headers()["etag"].to_str().unwrap().to_owned();
                let object = json_of(response).await;
                let version = item["version"].as_str().unwrap();
                assert_eq!(object["version"], version, "{uri}: {href}");
                assert!(etag.starts_with(&format!("\"{version}.")), "{etag}");
                if item["kind"] == "prefix" {
                    prefixes += 1;
                    let occupancy = ok(&reader, &format!("{href}/occupancy{dates}")).await;
                    assert_eq!(item["occupancy"], occupancy, "{uri}: {href}");
                } else {
                    assert!(item.get("occupancy").is_none(), "{uri}: {href}");
                }
            }
        }
    }
    assert_eq!(prefixes, 21);
    // The measures themselves: as of then, two addresses (one since deleted), one inside
    // the DHCP range of 11 addresses, in a /24 of 254; two /24 in a /22; two /24 and the
    // /22 in the /16. Now: the two added, outside the range.
    let children = ok(&reader, &format!("/v1/prefixes/{block}/children{known}")).await;
    let of = |value: &str| {
        children["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["value"] == value)
            .map(|item| item["occupancy"].clone())
            .unwrap()
    };
    assert_eq!(of("10.1.2.0/24")["used"], "12");
    assert_eq!(of("10.1.2.0/24")["total"], "254");
    assert_eq!(of("10.1.4.0/22")["used"], "512");
    assert_eq!(of("10.1.2.0/24")["prefix"]["id"], net.to_string());
    let now = ok(&reader, &format!("/v1/prefixes/{block}/children")).await;
    assert_eq!(now["items"][0]["occupancy"]["used"], "13");
    let roots = ok(&reader, &format!("/v1/domains/{lab}/roots")).await;
    assert_eq!(roots["items"][1]["id"], block.to_string());
    assert_eq!(roots["items"][1]["occupancy"]["used"], "1536");
    assert_eq!(
        roots["items"][2]["occupancy"]["total"],
        (1u128 << 96).to_string()
    );
}

/// A container with 10⁴ direct children (task 007-18): every page costs its own rows, the
/// last as the first, and the pages together list each direct child once, in order.
#[kartenn_testing::pg_test]
async fn the_children_of_a_wide_container_come_page_by_page(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let wide = prefix_of(&pool, lab, "10.3.0.0/16", "container").await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO prefix (id, domain_id, prefix, nature, public, registry_version,
                             valid_period, changeset_id)
         SELECT gen_random_uuid(), $1::uuid,
                set_masklen(('10.3.0.0'::inet + 4 * n), 30)::cidr, 'network', false, 'test',
                {SINCE}
         FROM generate_series(0, 9999) n"
    )))
    .bind(lab)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("ANALYZE prefix").execute(&pool).await.unwrap();
    // Nodes between the children are children too; a node of a child is not.
    let gap = address(&pool, lab, "10.3.200.1").await;
    address(&pool, lab, "10.3.0.1").await;
    let reader = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);

    let (mut seen, mut uri, mut times) = (
        Vec::new(),
        format!("/v1/prefixes/{wide}/children?page_size=100"),
        Vec::new(),
    );
    loop {
        let start = std::time::Instant::now();
        let body = ok(&reader, &uri).await;
        times.push(start.elapsed());
        seen.extend(
            body["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["value"].as_str().unwrap().to_owned()),
        );
        match body["links"]["next"].as_str() {
            Some(next) => next.clone_into(&mut uri),
            None => break,
        }
    }
    assert_eq!(seen.len(), 10_001);
    assert_eq!(seen[0], "10.3.0.0/30");
    assert_eq!(seen[1], "10.3.0.4/30");
    assert_eq!(seen[9_999], "10.3.156.60/30");
    assert_eq!(seen[10_000], "10.3.200.1");
    // The last page ends with the address between the children.
    assert!(ids_of(&ok(&reader, &uri).await).contains(&gap.to_string()));
    // A page costs its own rows: the median of the 101 pages, through the router, stays
    // far below the budget of `bench-core` (150 ms), even on a busy test machine; it was
    // 160 ms when a page recomputed the whole subtree under a generic plan.
    times.sort();
    let median = times[times.len() / 2];
    eprintln!("children of a wide container, page of 100: median {median:?}");
    assert!(median < std::time::Duration::from_millis(100), "{median:?}");
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

#[kartenn_testing::pg_test]
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

#[kartenn_testing::pg_test]
async fn properties_resolve_explain_and_mask(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let top = prefix_of(&pool, lab, "10.0.0.0/8", "container").await;
    let block = prefix_of(&pool, lab, "10.1.0.0/16", "container").await;
    let net = prefix(&pool, lab, "10.1.2.0/24").await;
    let zone = declare(&pool, "policy.zone", false).await;
    let secret = declare(&pool, "site.secret", true).await;
    assign(&pool, zone, top, "red", &[block, net]).await;
    assign(&pool, secret, top, "s3", &[block, net]).await;
    // A value inherited from a visible ancestor, and one of its own (task 021-11p1); their
    // keys after the others, for the pages of the schema below.
    let near = declare(&pool, "zone.near", false).await;
    let own = declare(&pool, "zone.own", false).await;
    assign(&pool, near, block, "amber", &[net]).await;
    assign(&pool, own, net, "green", &[]).await;
    // The default of a source of expected values, carried by the invisible /8: its
    // provenance is served, as by the explanation, its holder not named.
    let far = declare(&pool, "zone.far", false).await;
    assign(&pool, far, top, "grey", &[block, net]).await;
    sqlx::query(
        "UPDATE property_assignment SET provenance = 'source:inference@3'
         WHERE object_id = $1 AND property_id = $2",
    )
    .bind(top)
    .bind(far)
    .execute(&pool)
    .await
    .unwrap();
    // With the right to the sensitive value and to the whole tree.
    let mut rights = READ_ALL.to_vec();
    rights.push("prop.site.secret.read");
    let everything = app(&pool, person(&pool, &rights, ("global", None)).await);
    // Prefixes under the /16 only: the /8 that carries the values is invisible.
    let app = app(
        &pool,
        person(&pool, &["core.prefix.read"], ("prefix", Some(block))).await,
    );

    let resolved = format!("/v1/prefixes/{net}/properties");
    let body = ok(&app, &resolved).await;
    assert_eq!(
        body["properties"],
        json!({
            "policy.zone": "red", "zone.far": "grey", "zone.near": "amber", "zone.own": "green",
        }),
        "{body}"
    );
    assert_eq!(body["masked"], json!(["site.secret"]));
    // The source of each value shown: the /8 that carries `policy.zone` is invisible, its
    // existence undisclosed; the masked value has none.
    let node = |kind: &str, id: Uuid| json!({ "kind": kind, "id": id, "href": format!("/v1/prefixes/{id}") });
    assert_eq!(
        body["sources"],
        json!({
            "policy.zone": { "origin": null, "inherited": true, "provenance": "input" },
            "zone.far": {
                "origin": null, "inherited": true, "provenance": "source:inference@3",
            },
            "zone.near": { "origin": node("prefix", block), "inherited": true, "provenance": "input" },
            "zone.own": { "origin": node("prefix", net), "inherited": false, "provenance": "input" },
        }),
        "{body}"
    );
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
    // With the right to the whole tree, the origin of each value is named.
    let body = ok(&everything, &resolved).await;
    assert_eq!(
        body["sources"]["policy.zone"]["origin"],
        node("prefix", top),
        "{body}"
    );
    assert_eq!(
        body["sources"]["site.secret"]["origin"],
        node("prefix", top),
        "{body}"
    );

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

/// Declares `key`, carried by `carriers`, labelled by `labels` (`{<tag>: <text>}`).
async fn declare_for(pool: &PgPool, key: &str, carriers: &[&str], labels: Value) {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers,
                                           type_params, sensitive, labels, valid_period,
                                           changeset_id)
         VALUES (gen_random_uuid(), $1, 'bool', 'single', $2, '{{}}', false, $3, {SINCE})"
    )))
    .bind(key)
    .bind(carriers)
    .bind(labels)
    .execute(pool)
    .await
    .unwrap();
}

/// The keys of a page of declarations, and the link to the next one.
async fn declared(app: &Router, uri: &str) -> (Vec<String>, Option<String>) {
    let body = ok(app, uri).await;
    let keys = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["key"].as_str().unwrap().to_owned())
        .collect();
    let next = body["links"]["next"].as_str().map(str::to_owned);
    (keys, next)
}

#[kartenn_testing::pg_test]
async fn declarations_filter_by_carrier_and_prefix(pool: PgPool) {
    declare_for(
        &pool,
        "policy.zone",
        &["prefix"],
        json!({ "fr": "Zone de sécurité", "en": "Security zone" }),
    )
    .await;
    declare_for(&pool, "qos.class", &["address"], json!({ "en": "Class" })).await;
    declare_for(
        &pool,
        "site.code",
        &["site", "prefix"],
        json!({ "en": "Code" }),
    )
    .await;
    declare_for(&pool, "lab.slot", &["lab.rack"], json!({})).await;
    let app = app(&pool, person(&pool, &[], ("global", None)).await);
    let list = "/v1/property-declarations";
    let keys = |found: &[&str]| found.iter().map(|k| (*k).to_owned()).collect::<Vec<_>>();

    // By carrier: a kind of the core, a qualified type of a module, a type none carries.
    let (found, _) = declared(&app, &format!("{list}?carrier=prefix")).await;
    assert_eq!(found, keys(&["policy.zone", "site.code"]));
    let (found, _) = declared(&app, &format!("{list}?carrier=lab.rack")).await;
    assert_eq!(found, keys(&["lab.slot"]));
    let (found, _) = declared(&app, &format!("{list}?carrier=lab.shelf")).await;
    assert!(found.is_empty(), "{found:?}");
    // By prefix of the key, of a label in any language, ignoring case.
    let (found, _) = declared(&app, &format!("{list}?q=Site")).await;
    assert_eq!(found, keys(&["site.code"]));
    let (found, _) = declared(&app, &format!("{list}?q=zone")).await;
    assert_eq!(found, keys(&["policy.zone"]));
    let (found, _) = declared(&app, &format!("{list}?q=SECU")).await;
    assert_eq!(found, keys(&["policy.zone"]));
    let (found, _) = declared(&app, &format!("{list}?q=%25")).await;
    assert!(
        found.is_empty(),
        "a LIKE wildcard is a character: {found:?}"
    );
    // Both: `s` starts « Security zone » and `site.code`, only the latter on a site.
    let (found, _) = declared(&app, &format!("{list}?q=s")).await;
    assert_eq!(found, keys(&["policy.zone", "site.code"]));
    let (found, _) = declared(&app, &format!("{list}?carrier=site&q=s")).await;
    assert_eq!(found, keys(&["site.code"]));

    // The next page keeps the filter: `qos.class`, between the two, is left out.
    let (found, next) = declared(&app, &format!("{list}?carrier=prefix&page_size=1")).await;
    assert_eq!(found, keys(&["policy.zone"]));
    let (found, next) = declared(&app, &next.unwrap()).await;
    assert_eq!(found, keys(&["site.code"]));
    assert_eq!(next, None);
    let (found, next) = declared(&app, &format!("{list}?q=c&page_size=1")).await;
    assert_eq!(found, keys(&["qos.class"]));
    let (found, _) = declared(&app, &next.unwrap()).await;
    assert_eq!(found, keys(&["site.code"]));

    // A parameter given twice, empty, malformed or unknown: 400.
    for query in [
        "carrier=prefix&carrier=site",
        "q=s&q=z",
        "carrier=",
        "q=",
        "carrier=core.prefix",
        "carrier=Prefix",
        "carrier=nothing",
        "kind=prefix",
    ] {
        let response = get(&app, &format!("{list}?{query}")).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{query}");
    }
}

#[kartenn_testing::pg_test]
async fn a_deviation_of_a_sensitive_property_hides_its_value(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let net = prefix(&pool, lab, "10.1.2.0/24").await;
    declare(&pool, "site.secret", true).await;
    declare(&pool, "policy.zone", false).await;
    let author = person(&pool, &READ_ALL, ("global", None)).await.identity;
    // Two deviations on one object: the rule, the object and the property of its detail
    // key an anomaly (021/A39, task 11n).
    let detail = |key: &str, resolved: &str, expected: &str| {
        json!({
            "property": key, "resolved": resolved, "expected": expected,
            "source": "src", "revision": "1",
        })
    };
    for (net, message, detail) in [
        (
            net,
            "site.secret: resolved \"s3\", expected \"s4\" by src@1 (rule)",
            detail("site.secret", "s3", "s4"),
        ),
        (
            net,
            "policy.zone: resolved \"red\", expected \"blue\" by src@1 (rule)",
            detail("policy.zone", "red", "blue"),
        ),
    ] {
        let id = anomaly(&pool, author, net, "prop.resolution.expected_mismatch").await;
        sqlx::query("UPDATE anomaly SET message = $2, detail = $3 WHERE id = $1")
            .bind(id)
            .bind(message)
            .bind(detail)
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
    // Its members, the values left out for the sensitive one (021/A36).
    let item = |key: &str| items.iter().find(|i| i["property"] == key).unwrap();
    let zone = item("policy.zone");
    assert_eq!(zone["rule"], "prop.resolution.expected_mismatch", "{body}");
    assert_eq!(
        (&zone["resolved"], &zone["expected"]),
        (&json!("red"), &json!("blue")),
        "{body}"
    );
    assert_eq!(
        (&zone["source"], &zone["revision"]),
        (&json!("src"), &json!("1"))
    );
    let secret = item("site.secret");
    assert!(!secret.to_string().contains("s3") && !secret.to_string().contains("s4"));
    assert_eq!(
        (&secret["resolved"], &secret["expected"]),
        (&Value::Null, &Value::Null)
    );
    assert_eq!(secret["source"], "src", "{body}");
    let body = ok(&app, "/v1/property-gaps?property=policy.zone").await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1, "{body}");
    // The list of the anomalies masks it as well.
    let body = ok(&app, "/v1/anomalies").await;
    assert!(!body.to_string().contains("s3"), "{body}");
    let unknown = get(&app, "/v1/property-gaps?rule=x").await;
    assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
}

/// The anomalies and the deviations page by signed cursor, oldest first, to the last
/// page (task 021-29); an altered cursor is refused, as dates beside the first page.
#[kartenn_testing::pg_test]
async fn anomalies_and_deviations_page_by_cursor(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let author = person(&pool, &READ_ALL, ("global", None)).await.identity;
    let mut opened = Vec::new();
    let mut gaps = Vec::new();
    for n in 0..5 {
        let net = prefix(&pool, lab, &format!("10.1.{n}.0/24")).await;
        opened.push(anomaly(&pool, author, net, "core.test.paged").await);
        let gap = anomaly(&pool, author, net, "prop.resolution.expected_mismatch").await;
        let key = if n % 2 == 0 {
            "policy.zone"
        } else {
            "policy.env"
        };
        sqlx::query("UPDATE anomaly SET message = $2, detail = $3 WHERE id = $1")
            .bind(gap)
            .bind(format!("{key}: resolved a, expected b"))
            .bind(json!({ "property": key }))
            .execute(&pool)
            .await
            .unwrap();
        if n % 2 == 0 {
            gaps.push(gap);
        }
    }
    // The same opening for all: the identifier orders them.
    sqlx::query("UPDATE anomaly SET opened_at = '2026-01-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    opened.sort();
    gaps.sort();
    let app = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);
    let strings = |ids: &[Uuid]| ids.iter().map(Uuid::to_string).collect::<Vec<_>>();

    let uri = "/v1/anomalies?rule=core.test.paged&page_size=1";
    assert_eq!(walk(&app, uri).await, strings(&opened));
    let uri = "/v1/property-gaps?property=policy.zone&page_size=1";
    assert_eq!(walk(&app, uri).await, strings(&gaps));
    // Two pages of two, then one: the filter cut before the page.
    let body = ok(&app, "/v1/property-gaps?property=policy.zone&page_size=2").await;
    assert_eq!(ids_of(&body), strings(&gaps[..2]));
    let next = body["links"]["next"].as_str().unwrap().to_owned();
    let body = ok(&app, &next).await;
    assert_eq!(ids_of(&body), strings(&gaps[2..]));
    assert!(body["links"]["next"].is_null(), "{body}");

    // An altered cursor, a date: 400.
    let first = ok(&app, "/v1/anomalies?page_size=1").await;
    let next = first["links"]["next"].as_str().unwrap();
    let (head, last) = next.split_at(next.len() - 1);
    let altered = format!("{head}{}", if last == "A" { "B" } else { "A" });
    let response = get(&app, &altered).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_of(response).await["type"],
        "urn:kartenn:problem:invalid-cursor"
    );
    for uri in [
        "/v1/anomalies?valid_at=2026-01-01T00:00:00Z",
        "/v1/property-gaps?as_of=2026-01-01T00:00:00Z",
    ] {
        let response = get(&app, uri).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
    }
}

#[kartenn_testing::pg_test]
async fn the_anomalies_of_one_object_read_by_its_filter(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let dmz = domain(&pool, "dmz").await;
    let net = prefix(&pool, lab, "10.1.2.0/24").await;
    let neighbour = prefix(&pool, lab, "10.1.3.0/24").await;
    let hidden = prefix(&pool, dmz, "10.9.0.0/24").await;
    let site = node(&pool, "site", "north", None).await;
    let author = person(&pool, &READ_ALL, ("global", None)).await.identity;
    let mut own = Vec::new();
    for rule in ["core.test.a", "core.test.b", "core.test.c"] {
        own.push(anomaly(&pool, author, net, rule).await);
    }
    let of_b = own[1];
    anomaly(&pool, author, neighbour, "core.test.a").await;
    anomaly(&pool, author, hidden, "core.test.a").await;
    let outside = anomaly(&pool, author, site, "core.test.a").await;
    // The same opening for all: the identifier orders them.
    sqlx::query("UPDATE anomaly SET opened_at = '2026-01-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    own.sort();
    let strings = |ids: &[Uuid]| ids.iter().map(Uuid::to_string).collect::<Vec<_>>();
    let lab_reader = app(
        &pool,
        person(
            &pool,
            &["core.prefix.read", "core.site.read"],
            ("domain", Some(lab)),
        )
        .await,
    );
    let global = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);

    // Those of the object only, page by page, under the other filters.
    let uri = format!("/v1/anomalies?object={net}&page_size=1");
    assert_eq!(walk(&lab_reader, &uri).await, strings(&own));
    let uri = format!("/v1/anomalies?object={net}&rule=core.test.b");
    assert_eq!(ids_of(&ok(&lab_reader, &uri).await), [of_b.to_string()]);
    // An object outside any domain (a site).
    let uri = format!("/v1/anomalies?object={site}");
    assert_eq!(ids_of(&ok(&global, &uri).await), [outside.to_string()]);
    // An object the caller may not read: an empty list, as an unknown one.
    let unreadable = ok(&lab_reader, &format!("/v1/anomalies?object={hidden}")).await;
    let unknown = ok(
        &lab_reader,
        &format!("/v1/anomalies?object={}", Uuid::now_v7()),
    )
    .await;
    assert_eq!(unreadable["items"], serde_json::json!([]));
    assert_eq!(unreadable["items"], unknown["items"]);
    assert_eq!(unreadable["links"], unknown["links"]);
    assert_eq!(
        ids_of(&ok(&global, &format!("/v1/anomalies?object={hidden}")).await).len(),
        1
    );
    // Not a UUID: 400.
    let response = get(&global, "/v1/anomalies?object=north").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// The deviations of one object, as its anomalies (task 021-11p1, 021/A37): under the
/// other filters, empty for an object the caller may not read as for an unknown one.
#[kartenn_testing::pg_test]
async fn the_deviations_of_one_object_read_by_its_filter(pool: PgPool) {
    let lab = domain(&pool, "lab").await;
    let dmz = domain(&pool, "dmz").await;
    let net = prefix(&pool, lab, "192.0.2.0/25").await;
    let neighbour = prefix(&pool, lab, "192.0.2.128/25").await;
    let hidden = prefix(&pool, dmz, "198.51.100.0/24").await;
    let author = person(&pool, &READ_ALL, ("global", None)).await.identity;
    let gap = |object: Uuid, key: &'static str| {
        let pool = pool.clone();
        async move {
            let id = anomaly(&pool, author, object, "prop.resolution.expected_mismatch").await;
            sqlx::query("UPDATE anomaly SET message = $2, detail = $3 WHERE id = $1")
                .bind(id)
                .bind(format!("{key}: resolved a, expected b"))
                .bind(json!({ "property": key }))
                .execute(&pool)
                .await
                .unwrap();
            id
        }
    };
    let zone = gap(net, "policy.zone").await;
    let env = gap(net, "policy.env").await;
    gap(neighbour, "policy.zone").await;
    gap(hidden, "policy.zone").await;
    let lab_reader = app(
        &pool,
        person(&pool, &["core.prefix.read"], ("domain", Some(lab))).await,
    );
    let global = app(&pool, person(&pool, &READ_ALL, ("global", None)).await);

    let mut own = ids_of(&ok(&lab_reader, &format!("/v1/property-gaps?object={net}")).await);
    own.sort();
    let mut wanted = vec![zone.to_string(), env.to_string()];
    wanted.sort();
    assert_eq!(own, wanted);
    // The read is audited with its object among its criteria.
    let criteria: Value = sqlx::query_scalar(
        "SELECT details->'criteria' FROM audit.event WHERE name = 'data.objects.read'
         ORDER BY seq DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        criteria["property_gaps"]["object"],
        net.to_string(),
        "{criteria}"
    );
    let uri = format!("/v1/property-gaps?object={net}&property=policy.zone");
    assert_eq!(ids_of(&ok(&lab_reader, &uri).await), [zone.to_string()]);
    // Page by page under the filter.
    let uri = format!("/v1/property-gaps?object={net}&page_size=1");
    let mut walked = walk(&lab_reader, &uri).await;
    walked.sort();
    assert_eq!(walked, wanted);
    // An object the caller may not read: an empty list, as an unknown one.
    let unreadable = ok(&lab_reader, &format!("/v1/property-gaps?object={hidden}")).await;
    let unknown = ok(
        &lab_reader,
        &format!("/v1/property-gaps?object={}", Uuid::now_v7()),
    )
    .await;
    assert_eq!(unreadable["items"], json!([]));
    assert_eq!(unreadable["items"], unknown["items"]);
    assert_eq!(unreadable["links"], unknown["links"]);
    let uri = format!("/v1/property-gaps?object={hidden}");
    assert_eq!(ids_of(&ok(&global, &uri).await).len(), 1);
    // Not a UUID, or twice: 400.
    for uri in [
        "/v1/property-gaps?object=north".to_owned(),
        format!("/v1/property-gaps?object={net}&object={net}"),
    ] {
        let response = get(&global, &uri).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
    }
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

#[kartenn_testing::pg_test]
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
    // Each object of a filtered page carries its own version, read with its row in one
    // snapshot: the one its read by identifier states (task 021-11c).
    let body = ok(
        &app,
        "/v1/prefixes?domain=lab&prop.policy.env=prod&prop.policy.env=test",
    )
    .await;
    assert_eq!(ids_of(&body), sorted(vec![a, b, d]));
    for item in body["items"].as_array().unwrap() {
        let href = format!("/v1/prefixes/{}", item["id"].as_str().unwrap());
        let response = get(&app, &href).await;
        let etag = response.headers()["etag"].to_str().unwrap().to_owned();
        let version = item["version"].as_str().unwrap();
        assert_eq!(json_of(response).await["version"], version, "{href}");
        assert!(etag.starts_with(&format!("\"{version}.")), "{etag}");
    }
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
