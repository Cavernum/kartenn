//! The matrix of errors (spec 006, REQ-006-20 to 23; task 006-23): one test per line of
//! the table « Erreurs » of the design, each through the whole router, checking the
//! status, the `type`, the `Request-Id` (header and `instance`) and that no sensitive
//! value, token, query or panic message reaches the body; the first problem in the order
//! of HTTP; the same 404 for an invisible object as for an absent one.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_core::objects::AddressUsage;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::problem::{PROBLEM_JSON, ProblemKind, TYPE_PREFIX};
use kartenn_rest::quota::{Quotas, Rate, Rates};
use kartenn_rest::resources::MERGE_PATCH;
use kartenn_rest::transport::{ConnectionTls, MAX_URL, Settings};
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::ImportItem;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{Operation, Ref};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;

/// The access token of the tests; it never appears in a problem.
const TOKEN: &str = "token-of-the-matrix-7f3a";

/// The value of the sensitive property; it never appears in a problem.
const SECRET: &str = "s3cret-value-of-the-matrix";

/// What a problem never shows besides: a query, a panic, a database message.
const INTERNAL: [&str; 4] = ["SELECT", "panicked", "relation", "trigger"];

const PUBLIC: &str = "https://kartenn.example.invalid";

/// [`TOKEN`] and any session cookie are `who`; another token is refused.
struct Who(Principal);

impl Authenticator for Who {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let mut principal = self.0.clone();
        let answer = match proof {
            Proof::Bearer(token) if token == TOKEN => Ok(principal.into()),
            Proof::Bearer(_) => Err(Refusal::InvalidToken),
            Proof::Session(_) => {
                principal.method = Method::OidcSession;
                Ok(principal.into())
            }
            Proof::Basic(_) => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(answer))
    }
}

/// A person without issuer holding `entries` on `scope` (`("global", None)`,
/// `("domain", Some(id))`).
async fn person(pool: &PgPool, entries: &[&str], scope: (&str, Option<Uuid>)) -> Uuid {
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
        .bind(scope.0)
        .bind(scope.1)
        .execute(pool)
        .await
        .unwrap();
    identity
}

/// The rights to read every kind of object, without any write.
const READ_ALL: [&str; 6] = [
    "core.entity.read",
    "core.site.read",
    "core.domain.read",
    "core.prefix.read",
    "core.range.read",
    "core.address.read",
];

/// Rates that never refuse a request of a test.
fn generous() -> Rates {
    let rate = Rate::new(1_000_000, 1_000_000).unwrap();
    Rates {
        read: rate,
        write: rate,
        anonymous: rate,
    }
}

/// The application as `identity`, with `settings`, `rates`, and `extra` routes under
/// `/v1` beside the API.
fn app_with(
    pool: &PgPool,
    identity: Uuid,
    settings: Settings,
    rates: Rates,
    extra: Router,
) -> Router {
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from(["stdout".to_owned()]),
        modules: Arc::default(),
    };
    kartenn_rest::router(
        kartenn_rest::v1(api).merge(extra),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings {
            public_origin: Some(PUBLIC.into()),
            ..settings
        },
        Arc::new(Quotas::new(rates)),
        Arc::new(Who(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    )
}

fn app_as(pool: &PgPool, identity: Uuid) -> Router {
    app_with(
        pool,
        identity,
        Settings::default(),
        generous(),
        Router::new(),
    )
}

async fn admin(pool: &PgPool) -> Router {
    app_as(pool, person(pool, &["*"], ("global", None)).await)
}

/// `request` with [`TOKEN`], from a client over TLS.
fn bearer(mut request: Request<Body>) -> Request<Body> {
    request.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {TOKEN}")).unwrap(),
    );
    client(request)
}

/// `request` from a client over TLS, without proof.
fn client(mut request: Request<Body>) -> Request<Body> {
    request
        .extensions_mut()
        .insert(ConnectInfo::<std::net::SocketAddr>(
            "192.0.2.1:40000".parse().unwrap(),
        ));
    request.extensions_mut().insert(ConnectionTls(true));
    request
}

async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone()
        .oneshot(crate::common::applied(request))
        .await
        .unwrap()
}

async fn json_of(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn get_of(uri: &str) -> Request<Body> {
    bearer(Request::get(uri).body(Body::empty()).unwrap())
}

fn with_body(method: &str, uri: &str, content_type: &str, body: impl Into<Body>) -> Request<Body> {
    bearer(
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, content_type)
            .body(body.into())
            .unwrap(),
    )
}

fn post(uri: &str, body: &Value) -> Request<Body> {
    with_body("POST", uri, "application/json", body.to_string())
}

fn patch(uri: &str, etag: Option<&str>, body: &Value) -> Request<Body> {
    let mut request = with_body("PATCH", uri, MERGE_PATCH, body.to_string());
    if let Some(etag) = etag {
        request
            .headers_mut()
            .insert(header::IF_MATCH, etag.parse().unwrap());
    }
    request
}

/// Checks that `response` is a problem of class `kind` of its request, without any
/// sensitive value; its body.
async fn expect(response: Response, kind: ProblemKind) -> Value {
    assert_eq!(response.status(), kind.status(), "{kind:?}");
    assert_eq!(response.headers()[header::CONTENT_TYPE], PROBLEM_JSON);
    let id: Uuid = response.headers()["request-id"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(id.get_version_num(), 7);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    for shown in [SECRET, TOKEN].iter().chain(&INTERNAL) {
        assert!(!text.contains(shown), "{kind:?} shows {shown}: {text}");
    }
    let body: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["type"], format!("{TYPE_PREFIX}{}", kind.slug()));
    assert_eq!(body["title"], kind.title());
    assert_eq!(body["status"], kind.status().as_u16());
    assert_eq!(
        body["instance"],
        id.urn().to_string(),
        "the request identifier"
    );
    body
}

/// Creates `body` in `collection` as `app`; the path of the object.
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

fn id_of(path: &str) -> Uuid {
    path.rsplit('/').next().unwrap().parse().unwrap()
}

/// The domain `slug` and the network `value` in it; their paths.
async fn network(app: &Router, slug: &str, value: &str) -> (String, String) {
    let domain = created(app, "domains", &json!({"motive": "a lab", "slug": slug})).await;
    let prefix = created(
        app,
        "prefixes",
        &json!({"motive": "a net", "domain": {"id": id_of(&domain)},
                "value": value, "nature": "network"}),
    )
    .await;
    (domain, prefix)
}

/// The `ETag` of `GET uri`.
async fn etag(app: &Router, uri: &str) -> String {
    let response = send(app, get_of(uri)).await;
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    response.headers()[header::ETAG]
        .to_str()
        .unwrap()
        .to_owned()
}

/// Declares the sensitive text property `custom.secret` on prefixes (64 characters at
/// most) and gives `prefix` the value [`SECRET`].
async fn secret(pool: &PgPool, prefix: Uuid) {
    let property: Uuid = sqlx::query_scalar(
        "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers,
                                           type_params, sensitive, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'custom.secret', 'text', 'single', ARRAY['prefix'],
                 '{\"min_len\": 1, \"max_len\": 64}', true,
                 tstzrange('2020-01-01', NULL), gen_random_uuid())
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO property_assignment (object_id, object_type, property_id, kind, v_text,
                                          provenance, valid_period, changeset_id)
         VALUES ($1, 'prefix', $2, 'value', $3, 'input',
                 tstzrange('2020-01-01', NULL), gen_random_uuid())",
    )
    .bind(prefix)
    .bind(property)
    .bind(SECRET)
    .execute(pool)
    .await
    .unwrap();
}

/// A changeset creating the domain `lab` and the prefixes `prefixes` in it.
fn lab(prefixes: &[&str]) -> Value {
    let mut operations = vec![json!({"op": "create_domain", "local": 1, "slug": "lab"})];
    operations.extend(prefixes.iter().map(|p| {
        json!({"op": "create_prefix", "local": null, "domain": {"local": 1},
               "prefix": p, "nature": "network"})
    }));
    json!({"motive": "a lab", "operations": operations})
}

fn keyed(key: &str, body: &Value) -> Request<Body> {
    let mut request = post("/v1/changesets", body);
    request
        .headers_mut()
        .insert("idempotency-key", key.parse().unwrap());
    request
}

/// Element `i` of an import: the domain first, then addresses of it.
fn element(i: u32) -> ImportItem {
    let operation = if i == 0 {
        Operation::CreateDomain {
            local: Some(0),
            slug: "imported".into(),
        }
    } else {
        Operation::CreateAddress {
            local: Some(i),
            domain: Ref::Local(0),
            address: format!("10.9.0.{i}"),
            usage: AddressUsage::Host,
        }
    };
    ImportItem {
        external_id: Some(format!("e-{i}")),
        operation,
    }
}

/// Opens an import as `app` and deposits `elements` in its chunk 0; its path.
async fn import(app: &Router, elements: &[u32]) -> String {
    let opened = send(app, post("/v1/imports", &json!({"source": "legacy"}))).await;
    assert_eq!(opened.status(), StatusCode::CREATED);
    let path = opened.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let chunk = json!({"elements": elements.iter().map(|&i| element(i)).collect::<Vec<_>>()});
    let deposited = send(
        app,
        with_body(
            "PUT",
            &format!("{path}/chunks/0"),
            "application/json",
            chunk.to_string(),
        ),
    )
    .await;
    assert_eq!(deposited.status(), StatusCode::CREATED);
    path
}

fn start(path: &str) -> Request<Body> {
    bearer(
        Request::post(format!("{path}/start"))
            .body(Body::empty())
            .unwrap(),
    )
}

/// 400: invalid JSON, unknown parameter, date without offset; `invalid-cursor`.
#[kartenn_testing::pg_test]
async fn line_400_a_malformed_request(pool: PgPool) {
    let app = admin(&pool).await;
    let (_, prefix) = network(&app, "lab", "10.0.0.0/24").await;
    let invalid = format!("{{\"motive\": \"{SECRET}\", ");
    let response = send(
        &app,
        with_body("POST", "/v1/changesets", "application/json", invalid),
    )
    .await;
    expect(response, ProblemKind::BadRequest).await;
    let response = send(&app, get_of("/v1/prefixes?colour=red")).await;
    expect(response, ProblemKind::BadRequest).await;
    let response = send(&app, get_of(&format!("{prefix}?as_of=2026-01-01T00:00:00"))).await;
    expect(response, ProblemKind::BadRequest).await;
    let response = send(&app, get_of("/v1/prefixes?cursor=AAAAAAAAAAAA")).await;
    expect(response, ProblemKind::InvalidCursor).await;
}

/// 401: no proof, a refused token.
#[kartenn_testing::pg_test]
async fn line_401_without_or_with_a_refused_authentication(pool: PgPool) {
    let app = admin(&pool).await;
    let anonymous = client(Request::get("/v1/prefixes").body(Body::empty()).unwrap());
    expect(send(&app, anonymous).await, ProblemKind::Unauthenticated).await;
    let refused = client(
        Request::get("/v1/prefixes")
            .header(header::AUTHORIZATION, "Bearer another-token")
            .body(Body::empty())
            .unwrap(),
    );
    let response = send(&app, refused).await;
    assert!(response.headers().contains_key(header::WWW_AUTHENTICATE));
    expect(response, ProblemKind::Unauthenticated).await;
}

/// 403: a right missing for an action no changeset carries (the administration of the
/// rights, a filter of an output); an unsafe method of a session from another site.
#[kartenn_testing::pg_test]
async fn line_403_a_right_missing_or_a_cross_site_request(pool: PgPool) {
    let admin = admin(&pool).await;
    let reader = app_as(&pool, person(&pool, &READ_ALL, ("global", None)).await);
    expect(
        send(&reader, get_of("/v1/roles")).await,
        ProblemKind::Forbidden,
    )
    .await;
    let response = send(
        &reader,
        with_body(
            "PUT",
            "/v1/log-filters/stdout",
            "application/json",
            json!({"filter": "debug"}).to_string(),
        ),
    )
    .await;
    expect(response, ProblemKind::Forbidden).await;

    let cross_site = client(
        Request::post("/v1/changesets")
            .header(header::COOKIE, "__Host-kartenn=s")
            .header("sec-fetch-site", "cross-site")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(lab(&[]).to_string()))
            .unwrap(),
    );
    expect(
        send(&admin, cross_site).await,
        ProblemKind::CrossSiteRequest,
    )
    .await;
}

/// 404: an absent object, an invisible one answered alike (REQ-006-23).
#[kartenn_testing::pg_test]
async fn line_404_an_absent_or_invisible_object(pool: PgPool) {
    let admin = admin(&pool).await;
    let (seen, _) = network(&admin, "lab", "10.0.0.0/24").await;
    let (_, hidden) = network(&admin, "other", "10.1.0.0/24").await;
    secret(&pool, id_of(&hidden)).await;
    let reader = app_as(
        &pool,
        person(&pool, &READ_ALL, ("domain", Some(id_of(&seen)))).await,
    );
    let without_instance = |mut body: Value| {
        body.as_object_mut().unwrap().remove("instance");
        body
    };
    let invisible = expect(send(&reader, get_of(&hidden)).await, ProblemKind::NotFound).await;
    let absent = expect(
        send(&reader, get_of(&format!("/v1/prefixes/{}", Uuid::now_v7()))).await,
        ProblemKind::NotFound,
    )
    .await;
    assert_eq!(without_instance(invisible), without_instance(absent));
    let unrouted = send(&reader, get_of("/v1/nothing")).await;
    expect(unrouted, ProblemKind::NotFound).await;
}

/// 409: an action the state refuses (an import started twice, another chunk under the
/// same number), an undo in conflict, an idempotency key in progress.
#[kartenn_testing::pg_test]
async fn line_409_a_conflict(pool: PgPool) {
    let app = admin(&pool).await;
    let path = import(&app, &[0]).await;
    let other = json!({"elements": [element(0), element(1)]});
    let response = send(
        &app,
        with_body(
            "PUT",
            &format!("{path}/chunks/0"),
            "application/json",
            other.to_string(),
        ),
    )
    .await;
    expect(response, ProblemKind::Conflict).await;
    assert_eq!(
        send(&app, start(&path)).await.status(),
        StatusCode::ACCEPTED
    );
    expect(send(&app, start(&path)).await, ProblemKind::Conflict).await;

    let submitted = send(
        &app,
        post(
            "/v1/changesets",
            &json!({"motive": "m", "operations": [{"op": "create_domain", "slug": "lab"}]}),
        ),
    )
    .await;
    assert_eq!(submitted.status(), StatusCode::CREATED);
    let creation = json_of(submitted).await["changeset"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let domain: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = 'lab'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let path = format!("/v1/domains/{domain}");
    let tag = etag(&app, &path).await;
    let renamed = send(
        &app,
        patch(&path, Some(&tag), &json!({"motive": "r", "slug": "lab2"})),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let mut undo = bearer(
        Request::post(format!("/v1/changesets/{creation}/undo"))
            .body(Body::empty())
            .unwrap(),
    );
    undo.headers_mut()
        .insert(header::IF_MATCH, "\"applied\"".parse().unwrap());
    expect(send(&app, undo).await, ProblemKind::UndoConflict).await;

    // The journal held by another transaction: the first request waits there, its key
    // taken.
    let body = lab(&["10.0.0.0/24"]);
    let mut hold = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE changeset IN EXCLUSIVE MODE")
        .execute(&mut *hold)
        .await
        .unwrap();
    let first = tokio::spawn({
        let (app, body) = (app.clone(), body.clone());
        async move { send(&app, keyed("same", &body)).await.status() }
    });
    loop {
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT FROM pg_locks
                            WHERE relation = 'changeset'::regclass AND NOT granted)",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let second = send(&app, keyed("same", &body)).await;
    expect(second, ProblemKind::IdempotencyInProgress).await;
    hold.commit().await.unwrap();
    assert_eq!(first.await.unwrap(), StatusCode::CREATED);
}

/// 412: an `ETag` gone stale, on a write of a sensitive value.
#[kartenn_testing::pg_test]
async fn line_412_a_stale_etag(pool: PgPool) {
    let app = admin(&pool).await;
    let (_, prefix) = network(&app, "lab", "10.0.0.0/24").await;
    secret(&pool, id_of(&prefix)).await;
    let stale = etag(&app, &prefix).await;
    let fresh = send(
        &app,
        patch(
            &prefix,
            Some(&stale),
            &json!({"motive": "m", "anycast": true}),
        ),
    )
    .await;
    assert_eq!(fresh.status(), StatusCode::OK);
    let response = send(
        &app,
        patch(
            &prefix,
            Some(&stale),
            &json!({"motive": "m", "properties": {"custom.secret": format!("{SECRET}-2")}}),
        ),
    )
    .await;
    expect(response, ProblemKind::PreconditionFailed).await;
}

/// 413: a body over the limit.
#[kartenn_testing::pg_test]
async fn line_413_a_body_too_large(pool: PgPool) {
    let identity = person(&pool, &["*"], ("global", None)).await;
    let settings = Settings {
        max_body_bytes: 1024,
        ..Settings::default()
    };
    let app = app_with(&pool, identity, settings, generous(), Router::new());
    let body = json!({"motive": SECRET.repeat(64), "operations": []});
    expect(
        send(&app, post("/v1/changesets", &body)).await,
        ProblemKind::PayloadTooLarge,
    )
    .await;
}

/// 414: a URL over 8 KiB, refused before the routing with a minimal body.
#[kartenn_testing::pg_test]
async fn line_414_a_url_too_long(pool: PgPool) {
    let app = admin(&pool).await;
    let uri = format!("/v1/prefixes?q={}", "a".repeat(MAX_URL));
    let response = send(&app, get_of(&uri)).await;
    assert_eq!(response.status(), StatusCode::URI_TOO_LONG);
    let id: Uuid = response.headers()["request-id"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(id.get_version_num(), 7);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(body.is_empty(), "minimal body");
}

/// 415: a media type the route does not take.
#[kartenn_testing::pg_test]
async fn line_415_an_unsupported_media_type(pool: PgPool) {
    let app = admin(&pool).await;
    let response = send(
        &app,
        with_body("POST", "/v1/changesets", "text/plain", lab(&[]).to_string()),
    )
    .await;
    expect(response, ProblemKind::UnsupportedMediaType).await;
}

/// 422: a refused changeset, a write without its right (a rule, REQ-005-22), a
/// sensitive value against its rule, a motive missing, too
/// many operations, a reused idempotency key, a member not written, an undeclared
/// property, an invalid filter.
#[kartenn_testing::pg_test]
async fn line_422_a_request_the_service_refuses(pool: PgPool) {
    let app = admin(&pool).await;
    let refused = send(&app, post("/v1/changesets", &lab(&["10.1.2.5/24"]))).await;
    let body = expect(refused, ProblemKind::ChangesetRefused).await;
    assert_eq!(body["violations"].as_array().unwrap().len(), 1);

    let (_, prefix) = network(&app, "lab", "10.0.0.0/24").await;
    secret(&pool, id_of(&prefix)).await;
    let tag = etag(&app, &prefix).await;
    let too_long = SECRET.repeat(4);
    let response = send(
        &app,
        patch(
            &prefix,
            Some(&tag),
            &json!({"motive": "m", "properties": {"custom.secret": too_long}}),
        ),
    )
    .await;
    expect(response, ProblemKind::ChangesetRefused).await;

    let reader = app_as(&pool, person(&pool, &READ_ALL, ("global", None)).await);
    let tag = etag(&reader, &prefix).await;
    let response = send(
        &reader,
        patch(
            &prefix,
            Some(&tag),
            &json!({"motive": SECRET, "anycast": true}),
        ),
    )
    .await;
    let body = expect(response, ProblemKind::ChangesetRefused).await;
    assert_eq!(body["violations"][0]["rule"], "core.object.forbidden");

    let mut unmotivated = lab(&[]);
    unmotivated["motive"] = json!(" ");
    let response = send(&app, post("/v1/changesets", &unmotivated)).await;
    expect(response, ProblemKind::MotiveRequired).await;

    let operations: Vec<Value> = (0..=MAX_OPERATIONS)
        .map(|i| json!({"op": "create_domain", "slug": format!("d{i}")}))
        .collect();
    let many = json!({"motive": "m", "operations": operations});
    let response = send(&app, post("/v1/changesets", &many)).await;
    expect(response, ProblemKind::TooManyOperations).await;

    let keyed_body =
        json!({"motive": "k", "operations": [{"op": "create_domain", "slug": "keyed"}]});
    let first = send(&app, keyed("once", &keyed_body)).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let mut other = keyed_body.clone();
    other["motive"] = json!(SECRET);
    let response = send(&app, keyed("once", &other)).await;
    expect(response, ProblemKind::IdempotencyKeyReused).await;

    let tag = etag(&app, &prefix).await;
    let response = send(
        &app,
        patch(&prefix, Some(&tag), &json!({"motive": "m", "public": true})),
    )
    .await;
    expect(response, ProblemKind::InvalidMember).await;
    let response = send(
        &app,
        patch(
            &prefix,
            Some(&tag),
            &json!({"motive": "m", "properties": {"custom.undeclared": SECRET}}),
        ),
    )
    .await;
    expect(response, ProblemKind::InvalidMember).await;

    let response = send(
        &app,
        with_body(
            "PUT",
            "/v1/log-filters/stdout",
            "application/json",
            json!({"filter": "debug", "ttl": -1}).to_string(),
        ),
    )
    .await;
    expect(response, ProblemKind::InvalidOverride).await;
    // An import that does not sort is no longer refused here: `start` answers 202 and
    // the problem `invalid-import` is the `error` of the import (H18, tests/it/imports.rs).
}

/// 428: a write by resource without `If-Match`.
#[kartenn_testing::pg_test]
async fn line_428_if_match_missing(pool: PgPool) {
    let app = admin(&pool).await;
    let (_, prefix) = network(&app, "lab", "10.0.0.0/24").await;
    let response = send(
        &app,
        patch(&prefix, None, &json!({"motive": "m", "anycast": true})),
    )
    .await;
    expect(response, ProblemKind::PreconditionRequired).await;
}

/// 429: the quota of the identity spent.
#[kartenn_testing::pg_test]
async fn line_429_the_quota_spent(pool: PgPool) {
    let identity = person(&pool, &["*"], ("global", None)).await;
    let rates = Rates {
        write: Rate::new(1, 1).unwrap(),
        ..generous()
    };
    let app = app_with(&pool, identity, Settings::default(), rates, Router::new());
    let body = lab(&[]);
    assert_eq!(
        send(&app, post("/v1/changesets", &body)).await.status(),
        StatusCode::CREATED
    );
    let response = send(&app, post("/v1/changesets", &body)).await;
    assert!(response.headers().contains_key(header::RETRY_AFTER));
    expect(response, ProblemKind::RateLimited).await;
}

/// 503: the ceiling of requests in flight, the delay, the audit journal down during a
/// read, the database down.
#[kartenn_testing::pg_test]
async fn line_503_the_service_unavailable(pool: PgPool) {
    let identity = person(&pool, &["*"], ("global", None)).await;
    let (entered, release) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
    let extra = Router::new()
        .route(
            "/held",
            get({
                let (entered, release) = (entered.clone(), release.clone());
                move || async move {
                    entered.notify_one();
                    release.notified().await;
                    "done"
                }
            }),
        )
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                "late"
            }),
        );
    let settings = Settings {
        max_in_flight: 1,
        request_timeout: Duration::from_millis(200),
        ..Settings::default()
    };
    let app = app_with(&pool, identity, settings, generous(), extra);
    let held = tokio::spawn({
        let app = app.clone();
        async move { send(&app, get_of("/v1/held")).await.status() }
    });
    entered.notified().await;
    let response = send(&app, get_of("/v1/prefixes")).await;
    assert_eq!(response.headers()[header::RETRY_AFTER], "1");
    expect(response, ProblemKind::Unavailable).await;
    release.notify_one();
    assert_eq!(held.await.unwrap(), StatusCode::OK);

    expect(
        send(&app, get_of("/v1/slow")).await,
        ProblemKind::Unavailable,
    )
    .await;

    let app = app_as(&pool, identity);
    let (_, prefix) = network(&app, "lab", "10.0.0.0/24").await;
    secret(&pool, id_of(&prefix)).await;
    sqlx::raw_sql(
        "CREATE FUNCTION audit_down() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'audit down: SELECT internal'; END $$;
         CREATE TRIGGER down BEFORE INSERT ON audit.event
         FOR EACH ROW EXECUTE FUNCTION audit_down();",
    )
    .execute(&pool)
    .await
    .unwrap();
    expect(send(&app, get_of(&prefix)).await, ProblemKind::Unavailable).await;

    pool.close().await;
    expect(
        send(&app, get_of("/v1/prefixes")).await,
        ProblemKind::Unavailable,
    )
    .await;
}

async fn boom() -> StatusCode {
    panic!("{SECRET}: SELECT internal")
}

/// 500: a panic, its message kept from the client.
#[kartenn_testing::pg_test]
async fn line_500_a_panic(pool: PgPool) {
    let identity = person(&pool, &["*"], ("global", None)).await;
    let extra = Router::new().route("/boom", get(boom));
    let app = app_with(&pool, identity, Settings::default(), generous(), extra);
    let body = expect(send(&app, get_of("/v1/boom")).await, ProblemKind::Internal).await;
    assert!(body.get("detail").is_none(), "{body}");
}

/// REQ-006-22: an unauthenticated and malformed request → 401, not 400.
#[kartenn_testing::pg_test]
async fn the_first_problem_in_the_order_of_http_wins(pool: PgPool) {
    let app = admin(&pool).await;
    let malformed = || {
        Request::post("/v1/changesets")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{"))
            .unwrap()
    };
    expect(
        send(&app, client(malformed())).await,
        ProblemKind::Unauthenticated,
    )
    .await;
    expect(
        send(&app, bearer(malformed())).await,
        ProblemKind::BadRequest,
    )
    .await;
}
