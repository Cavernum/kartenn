//! Filters by parameters and by `QUERY` (spec 006, REQ-006-9, 10; task 006-11), on a
//! route that filters the prefixes of a domain by one property as the routes of 006-12
//! will: the same elements by `GET` and by `QUERY`, page after page; an unknown parameter
//! named; a `QUERY` without JSON refused; another method answered with `Allow`.

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::cursor::{Page, next_href, respond_page};
use kartenn_rest::filter::{self, Field, ListQuery};
use kartenn_rest::problem::{Problem, ProblemKind};
use kartenn_store_pg::filter::{FilterError, filter_in};
use kartenn_store_pg::rbac::ReadScope;
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

/// The prefixes of `domain` (an identifier) whose one property filter holds.
async fn prefixes(
    State(lists): State<Lists>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let internal = |_| Problem::new(ProblemKind::Internal);
    let bad = |detail: &str| Problem::new(ProblemKind::BadRequest).detail(detail);
    let page = Page::from_query(&query, &lists.ring, "prefixes")?;
    let filters = filter::parse(&page.filters, &["domain"])?;
    let (mut domain, mut property) = (None, None);
    for filter in filters {
        match (filter.field, filter.condition) {
            (Field::Core(_), kartenn_store_pg::filter::Condition::Equals(id)) => {
                domain = Some(Uuid::parse_str(&id).map_err(|_| bad("domain"))?);
            }
            (Field::Property(key), condition) => property = Some((key, condition)),
            _ => return Err(bad("domain")),
        }
    }
    let (Some(domain), Some((key, condition))) = (domain, property) else {
        return Err(bad("domain and one property"));
    };
    let now = read::now(&lists.pool).await.map_err(internal)?;
    let moment = page.dates.at(now, Duration::from_secs(60))?;
    let id: Uuid = sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = $1")
        .bind(&key)
        .fetch_optional(&lists.pool)
        .await
        .map_err(internal)?
        .ok_or_else(|| bad(&format!("prop.{key}: not declared")))?;
    let after = page
        .after()
        .map(|after| serde_json::from_value::<Uuid>(after.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?;
    let size = usize::from(page.size);
    let mut ids = filter_in(
        &lists.pool,
        id,
        domain,
        &condition,
        moment.valid_at,
        after,
        i64::from(page.size) + 1,
        &ReadScope::all(),
    )
    .await
    .map_err(|e| match e {
        FilterError::Database(_) => Problem::new(ProblemKind::Internal),
        e => bad(&format!("prop.{key}: {e}")),
    })?;
    let next = (ids.len() > size).then(|| {
        ids.truncate(size);
        let cursor = page.next(&lists.ring, "prefixes", &moment, json!(ids[size - 1]));
        next_href(PATH, &cursor)
    });
    let items: Vec<Value> = ids.into_iter().map(|id| json!({ "id": id })).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

fn app(pool: PgPool) -> Router {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    Router::new()
        .route(PATH, filter::collection(prefixes))
        .with_state(Lists { pool, ring })
}

async fn call(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn query(body: &Value) -> Request<Body> {
    Request::builder()
        .method("QUERY")
        .uri(PATH)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

async fn json_of(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// The identifiers of every page from `first`, following `next` by `GET`.
async fn walk(app: &Router, first: Request<Body>) -> Vec<String> {
    let mut seen = Vec::new();
    let mut request = first;
    loop {
        let response = call(app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["accept-query"], "\"application/json\"");
        let body = json_of(response).await;
        for item in body["items"].as_array().unwrap() {
            seen.push(item["id"].as_str().unwrap().to_owned());
        }
        let Some(next) = body["links"]["next"].as_str() else {
            return seen;
        };
        request = get(next);
    }
}

/// A domain, a text property `policy.env` on prefixes and a prefix per value of it.
async fn seed(pool: &PgPool) -> (Uuid, Vec<Uuid>) {
    let domain = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id)
         VALUES ($1, 'lab', tstzrange('2020-01-01', NULL), gen_random_uuid())",
    )
    .bind(domain)
    .execute(pool)
    .await
    .unwrap();
    let property: Uuid = sqlx::query_scalar(
        "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers,
                                           valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'policy.env', 'text', 'single', ARRAY['prefix'],
                 tstzrange('2020-01-01', NULL), gen_random_uuid())
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let mut ids = Vec::new();
    for (n, env) in ["prod", "test", "dev", "prod", "test"].iter().enumerate() {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO prefix (id, domain_id, prefix, nature, public, registry_version,
                                 valid_period, changeset_id)
             VALUES (gen_random_uuid(), $1, $2::cidr, 'network', false, 'test',
                     tstzrange('2020-01-01', NULL), gen_random_uuid())
             RETURNING id",
        )
        .bind(domain)
        .bind(format!("10.0.{n}.0/24"))
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO property_assignment (object_id, object_type, property_id, kind,
                                              v_text, provenance, valid_period, changeset_id)
             VALUES ($1, 'prefix', $2, 'value', $3, 'input',
                     tstzrange('2020-01-01', NULL), gen_random_uuid())",
        )
        .bind(id)
        .bind(property)
        .bind(env)
        .execute(pool)
        .await
        .unwrap();
        if *env != "dev" {
            ids.push(id);
        }
    }
    ids.sort();
    (domain, ids)
}

#[kartenn_testing::pg_test]
async fn get_and_query_read_the_same_elements(pool: PgPool) {
    let (domain, wanted) = seed(&pool).await;
    let app = app(pool);
    let wanted: Vec<String> = wanted.iter().map(Uuid::to_string).collect();
    let by_get = walk(
        &app,
        get(&format!(
            "{PATH}?domain={domain}&prop.policy.env=prod&prop.policy.env=test&page_size=2"
        )),
    )
    .await;
    assert_eq!(by_get, wanted);
    let search = json!({
        "page_size": 2,
        "filters": [
            { "field": "domain", "value": domain },
            { "field": "prop.policy.env", "value": ["prod", "test"] },
        ],
    });
    assert_eq!(walk(&app, query(&search)).await, wanted);
    // Replayed without an idempotency key: the same result.
    assert_eq!(walk(&app, query(&search)).await, wanted);
    // Bounds and the URL query of a `QUERY` (RFC 10008 §2).
    let mut between = query(&json!({
        "filters": [{ "field": "prop.policy.env", "op": "between", "value": ["prod", null] }],
    }));
    *between.uri_mut() = format!("{PATH}?domain={domain}").parse().unwrap();
    let mut texts = walk(&app, between).await;
    texts.sort();
    assert_eq!(texts, wanted, "prod and test sort after dev");
}

#[kartenn_testing::pg_test]
async fn a_wrong_search_is_refused(pool: PgPool) {
    let (domain, _) = seed(&pool).await;
    let app = app(pool);
    let unknown = call(&app, get(&format!("{PATH}?domain={domain}&colour=red"))).await;
    assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_of(unknown).await["detail"], "unknown parameter colour");
    let undeclared = call(&app, get(&format!("{PATH}?domain={domain}&prop.dns.ttl=1"))).await;
    assert_eq!(undeclared.status(), StatusCode::BAD_REQUEST);

    let mut bare = query(&json!({}));
    bare.headers_mut().remove(header::CONTENT_TYPE);
    let bare = call(&app, bare).await;
    assert_eq!(bare.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(bare.headers()["accept-query"], "\"application/json\"");
    let mut text = query(&json!({}));
    text.headers_mut()
        .insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
    assert_eq!(
        call(&app, text).await.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    let malformed = call(
        &app,
        query(&json!({ "filters": [{ "field": "a:b", "value": 1 }] })),
    );
    assert_eq!(malformed.await.status(), StatusCode::BAD_REQUEST);

    for method in ["PUT", "DELETE", "PATCH"] {
        let refused = call(
            &app,
            Request::builder()
                .method(method)
                .uri(PATH)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(refused.status(), StatusCode::METHOD_NOT_ALLOWED, "{method}");
        assert_eq!(refused.headers()[header::ALLOW], "GET, HEAD, QUERY");
        assert!(
            json_of(refused).await["type"]
                .as_str()
                .unwrap()
                .ends_with("method-not-allowed")
        );
    }
    let head = call(
        &app,
        Request::head(format!("{PATH}?domain={domain}&prop.policy.env=dev"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(head.status(), StatusCode::OK);
}
