//! Dated reads and their cache against the database (spec 006, REQ-006-5, 6; task 006-9),
//! on a route that reads one object as the routes of 006-12 will: a date without offset
//! refused, 304 while the object is unchanged then 200 after a changeset, `immutable`
//! only beyond the bound of the writes.

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use http_body_util::BodyExt;
use kartenn_rest::dated::{Dates, FROZEN, LIVE, respond_object};
use kartenn_rest::problem::{Problem, ProblemKind};
use kartenn_store_pg::read;
use sqlx::PgPool;
use sqlx::types::time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct Reads {
    pool: PgPool,
    bound: Duration,
}

async fn object(
    State(reads): State<Reads>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let internal = |_| Problem::new(ProblemKind::Internal);
    let now = read::now(&reads.pool).await.map_err(internal)?;
    let moment = dates.at(now, reads.bound)?;
    let version = read::version(&reads.pool, id, moment.valid_at, moment.as_of)
        .await
        .map_err(internal)?;
    let found = read::object(&reads.pool, id, moment.valid_at, moment.as_of)
        .await
        .map_err(internal)?;
    let (Some(version), Some(found)) = (version, found) else {
        return Err(Problem::new(ProblemKind::NotFound));
    };
    Ok(respond_object(&headers, &moment, version, &found.fields))
}

fn app(pool: PgPool, bound: Duration) -> Router {
    Router::new()
        .route("/v1/objects/{id}", get(object))
        .with_state(Reads { pool, bound })
}

async fn call(app: &Router, uri: &str, if_none_match: Option<&str>) -> Response {
    let mut request = Request::get(uri);
    if let Some(tag) = if_none_match {
        request = request.header(header::IF_NONE_MATCH, tag);
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body(response: Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

fn header_of<'a>(response: &'a Response, name: &header::HeaderName) -> &'a str {
    response.headers()[name].to_str().unwrap()
}

async fn domain(pool: &PgPool, id: Uuid, slug: &str) -> sqlx::Result<Uuid> {
    let changeset = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO addressing_domain (id, slug, valid_period, changeset_id)
         VALUES ($1, $2, tstzrange('2020-01-01', NULL), $3)",
    )
    .bind(id)
    .bind(slug)
    .bind(changeset)
    .execute(pool)
    .await?;
    Ok(changeset)
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap().replace('+', "%2B")
}

#[kartenn_testing::pg_test]
async fn a_date_without_offset_is_refused(pool: PgPool) -> sqlx::Result<()> {
    let app = app(pool.clone(), Duration::from_secs(60));
    let id = Uuid::now_v7();
    domain(&pool, id, "lab").await?;
    let local = call(
        &app,
        &format!("/v1/objects/{id}?as_of=2026-01-01T00:00:00"),
        None,
    )
    .await;
    assert_eq!(local.status(), StatusCode::BAD_REQUEST);
    let utc = call(
        &app,
        &format!("/v1/objects/{id}?as_of=2026-01-01T00:00:00Z"),
        None,
    )
    .await;
    assert_eq!(utc.status(), StatusCode::NOT_FOUND, "nothing asserted then");
    let future = call(
        &app,
        &format!("/v1/objects/{id}?as_of=2999-01-01T00:00:00Z"),
        None,
    )
    .await;
    assert_eq!(future.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[kartenn_testing::pg_test]
async fn not_modified_until_a_changeset(pool: PgPool) -> sqlx::Result<()> {
    let app = app(pool.clone(), Duration::from_secs(60));
    let id = Uuid::now_v7();
    let first = domain(&pool, id, "lab").await?;
    let uri = format!("/v1/objects/{id}");

    let fresh = call(&app, &uri, None).await;
    assert_eq!(fresh.status(), StatusCode::OK);
    assert_eq!(header_of(&fresh, &header::CACHE_CONTROL), LIVE);
    let etag = header_of(&fresh, &header::ETAG).to_owned();
    assert!(etag.starts_with(&format!("\"{first}.")), "{etag}");

    let unchanged = call(&app, &uri, Some(&etag)).await;
    assert_eq!(unchanged.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(header_of(&unchanged, &header::ETAG), etag);
    assert!(body(unchanged).await.is_empty());

    let second = Uuid::now_v7();
    sqlx::query("UPDATE addressing_domain SET slug = 'lab-2', changeset_id = $2 WHERE id = $1")
        .bind(id)
        .bind(second)
        .execute(&pool)
        .await?;
    let changed = call(&app, &uri, Some(&etag)).await;
    assert_eq!(changed.status(), StatusCode::OK);
    assert!(header_of(&changed, &header::ETAG).starts_with(&format!("\"{second}.")));
    Ok(())
}

#[kartenn_testing::pg_test]
async fn immutable_only_beyond_the_bound(pool: PgPool) -> sqlx::Result<()> {
    // No bound on the writes: only the clock margin (1 s) separates the past from the
    // frozen past.
    let app = app(pool.clone(), Duration::ZERO);
    let id = Uuid::now_v7();
    domain(&pool, id, "lab").await?;
    let asserted: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await?;
    let uri = format!("/v1/objects/{id}?as_of={}", rfc3339(asserted));

    let recent = call(&app, &uri, None).await;
    assert_eq!(recent.status(), StatusCode::OK);
    assert_eq!(header_of(&recent, &header::CACHE_CONTROL), LIVE);

    tokio::time::sleep(Duration::from_millis(1100)).await;
    let first = call(&app, &uri, None).await;
    assert_eq!(header_of(&first, &header::CACHE_CONTROL), FROZEN);
    let second = call(&app, &uri, None).await;
    assert_eq!(header_of(&second, &header::CACHE_CONTROL), FROZEN);
    assert_eq!(body(first).await, body(second).await);

    let now: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await?;
    let newer = call(
        &app,
        &format!("/v1/objects/{id}?as_of={}", rfc3339(now)),
        None,
    )
    .await;
    assert_eq!(header_of(&newer, &header::CACHE_CONTROL), LIVE);
    Ok(())
}
