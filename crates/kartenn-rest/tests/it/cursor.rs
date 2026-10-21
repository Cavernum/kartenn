//! Pagination by signed cursor against the database (spec 006, REQ-006-7, 8; task
//! 006-10), on a route that lists the prefixes as the routes of 006-12 will: an altered
//! cursor refused, writes between two pages neither skip nor repeat an element, a binding
//! removed between two pages closes what it opened.

use std::time::Duration;

use axum::body::Body;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::cursor::{Page, next_href, respond_page};
use kartenn_rest::problem::{Problem, ProblemKind};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_store_pg::list::{self, PrefixKey};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::read;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PATH: &str = "/v1/prefixes";

#[derive(Clone)]
struct Lists {
    pool: PgPool,
    ring: KeyRing,
}

async fn prefixes(
    State(lists): State<Lists>,
    Extension(principal): Extension<Principal>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let internal = |_| Problem::new(ProblemKind::Internal);
    let page = Page::from_query(&query.unwrap_or_default(), &lists.ring, "prefixes")?;
    let after = page
        .after()
        .map(|key| serde_json::from_value::<(Uuid, String, Uuid)>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?
        .map(|(domain, prefix, id)| PrefixKey { domain, prefix, id });
    let now = read::now(&lists.pool).await.map_err(internal)?;
    let moment = page.dates.at(now, Duration::from_secs(60))?;
    let scope = PgAuthorizer::new(lists.pool.clone())
        .read_scope(&principal.subject(), now)
        .await
        .map_err(|_| Problem::new(ProblemKind::Internal))?;
    let size = usize::from(page.size);
    let mut rows = list::prefixes(
        &lists.pool,
        moment.valid_at,
        Some(moment.as_of()),
        after.as_ref(),
        i64::from(page.size) + 1,
        &scope,
    )
    .await
    .map_err(internal)?;
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1].key;
        let cursor = page.next(
            &lists.ring,
            "prefixes",
            &moment,
            json!([last.domain, last.prefix, last.id]),
        );
        next_href(PATH, &cursor)
    });
    let items: Vec<Value> = rows.into_iter().map(|row| row.fields).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// A person with an identity in the database, without issuer: no confinement.
async fn person(pool: &PgPool) -> Principal {
    let identity = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
    )
    .fetch_one(pool)
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

fn app(pool: PgPool, principal: Principal) -> Router {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    Router::new()
        .route(PATH, get(prefixes))
        .layer(Extension(principal))
        .with_state(Lists { pool, ring })
}

async fn call(app: &Router, uri: &str) -> Response {
    app.clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn json_of(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// The prefixes of every page from `uri`, following `next`, with what each page sees.
async fn walk(app: &Router, uri: &str, between: impl AsyncFn(usize)) -> Vec<String> {
    let mut seen = Vec::new();
    let mut uri = uri.to_owned();
    for n in 0.. {
        let response = call(app, &uri).await;
        assert_eq!(response.status(), StatusCode::OK);
        let link = response
            .headers()
            .get(header::LINK)
            .map(|v| v.to_str().unwrap().to_owned());
        let body = json_of(response).await;
        for item in body["items"].as_array().unwrap() {
            seen.push(item["prefix"].as_str().unwrap().to_owned());
        }
        let Some(next) = body["links"]["next"].as_str() else {
            assert_eq!(link, None, "no next page, no link");
            return seen;
        };
        assert_eq!(link.unwrap(), format!("<{next}>; rel=\"next\""));
        uri = next.to_owned();
        between(n).await;
    }
    unreachable!()
}

async fn domain(pool: &PgPool, id: u128) -> Uuid {
    let id = Uuid::from_u128(id);
    sqlx::query(
        "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id)
         VALUES ($1, $2, tstzrange('2020-01-01', NULL), gen_random_uuid())",
    )
    .bind(id)
    .bind(id.to_string())
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn prefix(pool: &PgPool, domain: Uuid, value: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO prefix (id, domain_id, prefix, nature, public, registry_version,
                             valid_period, changeset_id)
         VALUES (gen_random_uuid(), $1, $2::cidr, 'network', false, 'test',
                 tstzrange('2020-01-01', NULL), gen_random_uuid())
         RETURNING id",
    )
    .bind(domain)
    .bind(value)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Binds a role reading the prefixes on `domain` to `who`; the binding's identifier.
async fn bind(pool: &PgPool, who: &Principal, domain: Uuid) -> Uuid {
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, label, valid_period, changeset_id)
         VALUES ($1, 'reader_' || replace($1::text, '-', ''), 'reader', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
         VALUES ($1, 'core.prefix.read', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'identity', $1, $2, 'domain', $3,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())
         RETURNING id",
    )
    .bind(who.identity)
    .bind(role)
    .bind(domain)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[kartenn_testing::pg_test]
async fn a_cursor_is_signed_and_a_page_bounded(pool: PgPool) {
    let who = person(&pool).await;
    let lab = domain(&pool, 1).await;
    bind(&pool, &who, lab).await;
    for value in ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"] {
        prefix(&pool, lab, value).await;
    }
    let app = app(pool, who);
    assert_eq!(
        call(&app, &format!("{PATH}?page_size=1001")).await.status(),
        StatusCode::BAD_REQUEST
    );
    let first = json_of(call(&app, &format!("{PATH}?page_size=2")).await).await;
    let next = first["links"]["next"].as_str().unwrap();
    let (base, cursor) = next.split_once("cursor=").unwrap();
    // One octet of the payload changed, the signature kept.
    let mut altered = cursor.to_owned().into_bytes();
    let i = altered.len() / 2;
    altered[i] = if altered[i] == b'A' { b'B' } else { b'A' };
    let altered = format!("{base}cursor={}", String::from_utf8(altered).unwrap());
    let refused = call(&app, &altered).await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    assert!(
        json_of(refused).await["type"]
            .as_str()
            .unwrap()
            .ends_with("invalid-cursor")
    );
    let beside = call(&app, &format!("{next}&as_of=2026-01-01T00:00:00Z")).await;
    assert_eq!(beside.status(), StatusCode::BAD_REQUEST);
    let last = json_of(call(&app, next).await).await;
    assert_eq!(last["items"].as_array().unwrap().len(), 1);
    assert_eq!(last["links"]["next"], Value::Null);
}

#[kartenn_testing::pg_test]
async fn writes_between_pages_neither_skip_nor_repeat(pool: PgPool) {
    let who = person(&pool).await;
    let lab = domain(&pool, 1).await;
    bind(&pool, &who, lab).await;
    let values = [
        "10.0.1.0/24",
        "10.0.2.0/24",
        "10.0.3.0/24",
        "10.0.4.0/24",
        "10.0.5.0/24",
    ];
    let mut ids = Vec::new();
    for value in values {
        ids.push(prefix(&pool, lab, value).await);
    }
    let app = app(pool.clone(), who);
    let seen = walk(&app, &format!("{PATH}?page_size=2"), async |n| {
        if n == 0 {
            // Before the cursor, after it, and one already read then removed.
            prefix(&pool, lab, "10.0.0.0/24").await;
            prefix(&pool, lab, "10.0.9.0/24").await;
            sqlx::query("DELETE FROM prefix WHERE id = $1")
                .bind(ids[0])
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("DELETE FROM prefix WHERE id = $1")
                .bind(ids[3])
                .execute(&pool)
                .await
                .unwrap();
        }
    })
    .await;
    assert_eq!(seen, values, "the list as of the first page");
    // A new list sees the writes.
    let fresh = walk(&app, &format!("{PATH}?page_size=2"), async |_| {}).await;
    assert_eq!(
        fresh,
        [
            "10.0.0.0/24",
            "10.0.2.0/24",
            "10.0.3.0/24",
            "10.0.5.0/24",
            "10.0.9.0/24"
        ]
    );
}

#[kartenn_testing::pg_test]
async fn a_binding_removed_between_pages_closes_what_it_opened(pool: PgPool) {
    let who = person(&pool).await;
    let (lab, other) = (domain(&pool, 1).await, domain(&pool, 2).await);
    let hidden = domain(&pool, 3).await;
    bind(&pool, &who, lab).await;
    let opened = bind(&pool, &who, other).await;
    for (domain, value) in [
        (lab, "10.0.1.0/24"),
        (lab, "10.0.2.0/24"),
        (other, "10.0.1.0/24"),
        (other, "10.0.2.0/24"),
        (hidden, "10.0.1.0/24"),
    ] {
        prefix(&pool, domain, value).await;
    }
    let app = app(pool.clone(), who);
    let first = json_of(call(&app, &format!("{PATH}?page_size=2")).await).await;
    let domains = |page: &Value| -> Vec<String> {
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["domain_id"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(domains(&first), [lab.to_string(), lab.to_string()]);
    let next = first["links"]["next"].as_str().unwrap();
    let granted = json_of(call(&app, next).await).await;
    assert_eq!(domains(&granted), [other.to_string(), other.to_string()]);
    assert_eq!(
        granted["links"]["next"],
        Value::Null,
        "never the hidden domain"
    );

    sqlx::query("DELETE FROM binding WHERE id = $1")
        .bind(opened)
        .execute(&pool)
        .await
        .unwrap();
    let revoked = json_of(call(&app, next).await).await;
    assert_eq!(domains(&revoked), Vec::<String>::new());
    assert_eq!(revoked["links"]["next"], Value::Null);
}
