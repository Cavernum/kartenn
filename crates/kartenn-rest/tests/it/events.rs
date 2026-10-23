//! The event stream `GET /v1/events` through the router (spec 022, tasks 7 to 11): the
//! format and the headers, the filters, the rights of each subscriber, the resumption by
//! cursor on either of two instances, the checks of the cookie, the revocation.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_core::objects::Nature;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::events::hub::{Hub, Settings as EventSettings};
use kartenn_rest::events::reader::Reader;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rate, Rates};
use kartenn_rest::transport::{ConnectionTls, Settings};
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::authz::Unrestricted;
use kartenn_service::events::{Parsed, Parser};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::watch;
use tower::ServiceExt;
use uuid::Uuid;

const PUBLIC: &str = "https://kartenn.example.invalid";

/// Any `Bearer` token is this principal; a session cookie the same, by session.
struct Who(Principal);

impl Authenticator for Who {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let method = match proof {
            Proof::Bearer(_) => Ok(Method::Bearer),
            Proof::Session(_) => Ok(Method::OidcSession),
            Proof::Basic(_) => Err(Refusal::Unauthenticated),
        };
        let principal = self.0.clone();
        Box::pin(std::future::ready(method.map(|method| {
            Principal {
                method,
                ..principal
            }
            .into()
        })))
    }
}

/// A person without issuer holding `permissions` on `scope`.
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

/// An instance of the stream on `pool`: its hub, its reader, its checks.
struct Instance1 {
    hub: Arc<Hub>,
    _stop: watch::Sender<bool>,
}

fn quick() -> EventSettings {
    EventSettings {
        heartbeat: Duration::from_secs(5),
        checkpoint: Duration::from_millis(300),
        poll: Duration::from_millis(200),
        recheck: Duration::from_millis(300),
        public_origin: Some(PUBLIC.into()),
        ..EventSettings::default()
    }
}

async fn instance(pool: &PgPool, ring: &KeyRing, settings: EventSettings) -> Instance1 {
    let hub = Hub::new(settings, pool.clone(), ring.clone(), Arc::default());
    let reader = Reader::start(hub.clone(), pool.clone()).await.unwrap();
    let (stop, stopped) = watch::channel(false);
    tokio::spawn(reader.run(stopped.clone()));
    tokio::spawn(hub.clone().watch(stopped));
    Instance1 { hub, _stop: stop }
}

fn app(pool: &PgPool, ring: &KeyRing, hub: &Arc<Hub>, who: Principal) -> Router {
    let api = Api {
        pool: pool.clone(),
        ring: ring.clone(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: Arc::default(),
    };
    kartenn_rest::router(
        kartenn_rest::v1(api).layer(axum::Extension(hub.clone())),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings {
            public_origin: Some(PUBLIC.into()),
            ..Settings::default()
        },
        Arc::new(Quotas::new(Rates {
            read: Rate::new(1000, 1000).unwrap(),
            ..Rates::default()
        })),
        Arc::new(Who(who)),
    )
}

/// A stream being read.
struct Client {
    body: Body,
    parser: Parser,
    events: std::collections::VecDeque<Parsed>,
}

impl Client {
    /// The next event, within `wait`.
    async fn next(&mut self, wait: Duration) -> Option<Parsed> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            if let Some(e) = self.events.pop_front() {
                return Some(e);
            }
            let frame = tokio::time::timeout_at(deadline, self.body.frame())
                .await
                .ok()??;
            if let Ok(data) = frame.unwrap().into_data() {
                let text = String::from_utf8(data.to_vec()).unwrap();
                self.events.extend(self.parser.feed(&text));
            }
        }
    }

    /// The next event of `kind`, the others skipped.
    async fn next_of(&mut self, kind: &str, wait: Duration) -> Option<Parsed> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            let e = self.next(left).await?;
            if e.event == kind {
                return Some(e);
            }
        }
    }

    /// Every event within `wait`.
    async fn drain(&mut self, wait: Duration) -> Vec<Parsed> {
        let mut out = Vec::new();
        while let Some(e) = self.next(wait).await {
            out.push(e);
        }
        out
    }
}

fn stream_request(uri: &str, last: Option<&str>) -> Request<Body> {
    let mut r = Request::get(uri)
        .header(header::ACCEPT, "text/event-stream")
        .header(header::AUTHORIZATION, "Bearer t")
        .body(Body::empty())
        .unwrap();
    if let Some(last) = last {
        r.headers_mut()
            .insert("last-event-id", HeaderValue::from_str(last).unwrap());
    }
    r
}

async fn open(app: &Router, uri: &str, last: Option<&str>) -> (Response<()>, Client) {
    let response = app
        .clone()
        .oneshot(stream_request(uri, last))
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    (
        Response::from_parts(parts, ()),
        Client {
            body,
            parser: Parser::default(),
            events: std::collections::VecDeque::new(),
        },
    )
}

fn author_request(author: Uuid, operations: Vec<Operation>) -> ChangesetRequest {
    ChangesetRequest {
        author,
        motive: "test".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
    }
}

async fn author(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'w') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Applies `operations` and returns the changeset.
async fn apply(pool: &PgPool, author: Uuid, operations: Vec<Operation>) -> Uuid {
    let store = PgStore::new(pool.clone());
    match submit(
        &store,
        &Unrestricted,
        &author_request(author, operations),
        MAX_OPERATIONS,
    )
    .await
    .unwrap()
    {
        Outcome::Applied { changeset, .. } => changeset,
        other => panic!("not applied: {other:?}"),
    }
}

/// A domain `slug` and its id, by a changeset of its own.
async fn domain(pool: &PgPool, author: Uuid, slug: &str) -> Uuid {
    apply(
        pool,
        author,
        vec![Operation::CreateDomain {
            local: None,
            slug: slug.into(),
        }],
    )
    .await;
    sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = $1")
        .bind(slug)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn prefix_in(domain: Uuid, prefix: &str) -> Operation {
    Operation::CreatePrefix {
        local: None,
        domain: Ref::Id(domain),
        prefix: prefix.into(),
        nature: Nature::Container,
        point_to_point: false,
        anycast: false,
    }
}

fn data(e: &Parsed) -> Value {
    serde_json::from_str(&e.data).unwrap()
}

const WAIT: Duration = Duration::from_secs(5);

/// REQ-022-1, 4, 11, 13: `ready` at once with `retry:`, the headers, no compression; a
/// changeset gives its event of keys and revision.
#[kartenn_testing::pg_test]
async fn a_changeset_reaches_an_open_stream(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let i = instance(&pool, &ring, quick()).await;
    let reader = person(&pool, &READ_ALL, ("global", None)).await;
    let app = app(&pool, &ring, &i.hub, reader);
    let mut request = stream_request("/v1/events", None);
    request.headers_mut().insert(
        header::ACCEPT_ENCODING,
        HeaderValue::from_static("gzip, br, zstd"),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let h = response.headers();
    assert_eq!(h[header::CONTENT_TYPE], "text/event-stream");
    assert_eq!(h[header::CACHE_CONTROL], "no-store");
    assert_eq!(h["x-accel-buffering"], "no");
    assert!(h.get(header::CONTENT_ENCODING).is_none());
    assert!(h.get(header::CONTENT_LENGTH).is_none());
    assert!(h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
    assert!(h.get("incremental").is_none());
    let mut client = Client {
        body: response.into_body(),
        parser: Parser::default(),
        events: std::collections::VecDeque::new(),
    };
    let ready = client
        .next(Duration::from_secs(1))
        .await
        .expect("ready within 1 s");
    assert_eq!(ready.event, "ready");
    assert!(ready.last_id.starts_with("k1."));
    let retry = client.parser.retry.unwrap();
    assert!((1_000..=3_000).contains(&retry), "{retry}");
    assert_eq!(data(&ready)["ring_seconds"], 900);
    let writer = author(&pool).await;
    let d = domain(&pool, writer, "prod").await;
    let changeset = apply(&pool, writer, vec![prefix_in(d, "10.0.0.0/8")]).await;
    let prefix: Uuid = sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '10.0.0.0/8'")
        .fetch_one(&pool)
        .await
        .unwrap();
    // The domain first, then the prefix.
    let first = client.next_of("changes", WAIT).await.unwrap();
    assert_eq!(data(&first)["objects"][0]["type"], "core.domain");
    let second = client.next_of("changes", WAIT).await.unwrap();
    assert_eq!(
        data(&second),
        json!({"changeset": changeset, "objects": [{"type": "core.prefix", "id": prefix}],
               "domains": [d]})
    );
    assert!(!second.data.contains('\n'));
    assert_ne!(second.last_id, ready.last_id);
}

/// REQ-022-1, 2: 406 without the media type, 400 for a filter unknown, out of bounds or
/// of a type absent.
#[kartenn_testing::pg_test]
async fn the_opening_refuses_what_it_cannot_serve(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let i = instance(&pool, &ring, quick()).await;
    let app = app(
        &pool,
        &ring,
        &i.hub,
        person(&pool, &READ_ALL, ("global", None)).await,
    );
    let mut json = stream_request("/v1/events", None);
    json.headers_mut()
        .insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    assert_eq!(
        app.clone().oneshot(json).await.unwrap().status(),
        StatusCode::NOT_ACCEPTABLE
    );
    for uri in [
        "/v1/events?types=inconnu.truc",
        "/v1/events?since=1",
        "/v1/events?kinds=ready",
        "/v1/events?objects=not-a-uuid",
    ] {
        let response = app
            .clone()
            .oneshot(stream_request(uri, None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let problem: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(problem["code"], "events.filter.invalid", "{uri}");
    }
    // Without authentication: 401, never a stream.
    let anonymous = Request::get("/v1/events")
        .header(header::ACCEPT, "text/event-stream")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(anonymous).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

/// REQ-022-2, 16: a filter by type, a subscriber who reads one domain only: nothing of
/// the other, no trace of it.
#[kartenn_testing::pg_test]
async fn a_subscriber_receives_only_what_it_may_read_and_asked_for(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let i = instance(&pool, &ring, quick()).await;
    let writer = author(&pool).await;
    let mine = domain(&pool, writer, "mine").await;
    let other = domain(&pool, writer, "other").await;
    let reader = person(&pool, &READ_ALL, ("domain", Some(mine))).await;
    let app = app(&pool, &ring, &i.hub, reader);
    let (_, mut all) = open(&app, "/v1/events", None).await;
    let (_, mut vlans) = open(&app, "/v1/events?types=core.address", None).await;
    assert_eq!(all.next(WAIT).await.unwrap().event, "ready");
    assert_eq!(vlans.next(WAIT).await.unwrap().event, "ready");
    apply(&pool, writer, vec![prefix_in(other, "10.0.0.0/8")]).await;
    let seen = apply(&pool, writer, vec![prefix_in(mine, "10.1.0.0/16")]).await;
    let e = all.next_of("changes", WAIT).await.unwrap();
    assert_eq!(
        data(&e)["changeset"],
        json!(seen),
        "nothing of the other domain"
    );
    assert_eq!(data(&e)["domains"], json!([mine]));
    assert!(
        vlans
            .drain(Duration::from_millis(800))
            .await
            .iter()
            .all(|e| e.event != "changes"),
        "a prefix is not an address"
    );
}

/// REQ-022-7, 8, 27: a cut during three changesets, resumed on another instance with the
/// last cursor: the three come, in order; a cursor altered or of another identity starts
/// over (`reset`), never an error.
#[kartenn_testing::pg_test]
async fn a_resumption_on_another_instance_loses_nothing(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let first = instance(&pool, &ring, quick()).await;
    let second = instance(&pool, &ring, quick()).await;
    let writer = author(&pool).await;
    let d = domain(&pool, writer, "prod").await;
    let reader = person(&pool, &READ_ALL, ("global", None)).await;
    let a = app(&pool, &ring, &first.hub, reader.clone());
    let b = app(&pool, &ring, &second.hub, reader.clone());
    let (_, mut client) = open(&a, "/v1/events", None).await;
    let ready = client.next(WAIT).await.unwrap();
    let mut last = ready.last_id.clone();
    let c0 = apply(&pool, writer, vec![prefix_in(d, "10.0.0.0/8")]).await;
    let e = client.next_of("changes", WAIT).await.unwrap();
    assert_eq!(data(&e)["changeset"], json!(c0));
    last.clone_from(&client.parser.last_id);
    drop(client);
    let mut written = Vec::new();
    for (n, p) in ["10.1.0.0/16", "10.2.0.0/16", "10.3.0.0/16"]
        .iter()
        .enumerate()
    {
        written.push(apply(&pool, writer, vec![prefix_in(d, p)]).await);
        assert_eq!(written.len(), n + 1);
    }
    // The second instance has read them.
    tokio::time::sleep(Duration::from_millis(600)).await;
    let (_, mut resumed) = open(&b, "/v1/events", Some(&last)).await;
    assert_eq!(resumed.next(WAIT).await.unwrap().event, "ready");
    let mut got = Vec::new();
    while got.len() < written.len() {
        let e = resumed
            .next_of("changes", WAIT)
            .await
            .expect("every changeset");
        let c: Uuid = serde_json::from_value(data(&e)["changeset"].clone()).unwrap();
        // Duplicates of changesets received before the cut (the floor of the cursor may
        // lie below them) are allowed, recognised by their identity.
        if written.contains(&c) && !got.contains(&c) {
            got.push(c);
        } else {
            assert!(
                c == c0 || !written.contains(&c),
                "twice after the resumption: {c}"
            );
        }
    }
    assert_eq!(got, written, "in the order of the domain");
    // An altered cursor, or one of another identity: `reset`, then the stream.
    let mut altered = last.clone().into_bytes();
    let n = altered.len() - 3;
    altered[n] = if altered[n] == b'A' { b'B' } else { b'A' };
    let (status, mut client) = open(
        &b,
        "/v1/events",
        Some(std::str::from_utf8(&altered).unwrap()),
    )
    .await;
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(client.next(WAIT).await.unwrap().event, "ready");
    let reset = client.next(WAIT).await.unwrap();
    assert_eq!(reset.event, "reset");
    assert_eq!(data(&reset)["reason"], "invalid_cursor");
    let stranger = person(&pool, &READ_ALL, ("global", None)).await;
    let s = app(&pool, &ring, &second.hub, stranger);
    let (_, mut client) = open(&s, &format!("/v1/events?last_event_id={last}"), None).await;
    client.next(WAIT).await.unwrap();
    assert_eq!(
        data(&client.next(WAIT).await.unwrap())["reason"],
        "invalid_cursor"
    );
}

/// REQ-022-9: a subscriber who sees nothing still has its cursor moved, by resumption
/// points that dispatch no event.
#[kartenn_testing::pg_test]
async fn resumption_points_move_the_cursor_of_an_idle_subscriber(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let i = instance(&pool, &ring, quick()).await;
    let writer = author(&pool).await;
    let d = domain(&pool, writer, "prod").await;
    let nobody = person(&pool, &[], ("global", None)).await;
    let app = app(&pool, &ring, &i.hub, nobody);
    let (_, mut client) = open(&app, "/v1/events", None).await;
    let ready = client.next(WAIT).await.unwrap();
    for p in ["10.1.0.0/16", "10.2.0.0/16"] {
        apply(&pool, writer, vec![prefix_in(d, p)]).await;
    }
    assert!(
        client.drain(Duration::from_secs(2)).await.is_empty(),
        "nothing visible"
    );
    assert_ne!(client.parser.last_id, ready.last_id, "the cursor moved");
}

/// REQ-022-15, 19: a cookie session opens the stream from the public origin only; `Bearer`
/// is not concerned.
#[kartenn_testing::pg_test]
async fn a_cookie_opens_the_stream_from_the_public_origin_only(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let i = instance(&pool, &ring, quick()).await;
    let app = app(
        &pool,
        &ring,
        &i.hub,
        person(&pool, &READ_ALL, ("global", None)).await,
    );
    let cases: [(Option<&str>, Option<&str>, StatusCode); 5] = [
        (Some("same-origin"), Some(PUBLIC), StatusCode::OK),
        (None, None, StatusCode::OK),
        (Some("cross-site"), None, StatusCode::FORBIDDEN),
        (
            None,
            Some("https://other.example.invalid"),
            StatusCode::FORBIDDEN,
        ),
        (Some("same-site"), Some(PUBLIC), StatusCode::FORBIDDEN),
    ];
    for (site, origin, expected) in cases {
        let mut r = Request::get("/v1/events")
            .header(header::ACCEPT, "text/event-stream")
            .header(header::COOKIE, "__Host-kartenn=x")
            .body(Body::empty())
            .unwrap();
        // A session cookie travels over TLS only (REQ-006-27).
        r.extensions_mut().insert(ConnectionTls(true));
        if let Some(site) = site {
            r.headers_mut()
                .insert("sec-fetch-site", HeaderValue::from_static(site));
        }
        if let Some(origin) = origin {
            r.headers_mut()
                .insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
        }
        let response = app.clone().oneshot(r).await.unwrap();
        assert_eq!(response.status(), expected, "{site:?} × {origin:?}");
        assert!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
    }
    let mut bearer = stream_request("/v1/events", None);
    bearer
        .headers_mut()
        .insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
    assert_eq!(
        app.clone().oneshot(bearer).await.unwrap().status(),
        StatusCode::OK
    );
}

/// REQ-022-18: the identity disabled, the stream ends within the check; a binding
/// removed, `reset` `rights_changed`, then nothing of the former scope.
#[kartenn_testing::pg_test]
async fn a_revocation_ends_the_stream_and_a_binding_removed_starts_it_over(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let i = instance(&pool, &ring, quick()).await;
    let writer = author(&pool).await;
    let d = domain(&pool, writer, "prod").await;
    let reader = person(&pool, &READ_ALL, ("global", None)).await;
    let app = app(&pool, &ring, &i.hub, reader.clone());
    let (_, mut client) = open(&app, "/v1/events", None).await;
    client.next(WAIT).await.unwrap();
    sqlx::query(
        "UPDATE binding SET valid_period = tstzrange(lower(valid_period), now())
                 WHERE subject_id = $1",
    )
    .bind(reader.identity)
    .execute(&pool)
    .await
    .unwrap();
    let reset = client.next_of("reset", WAIT).await.expect("reset");
    assert_eq!(data(&reset)["reason"], "rights_changed");
    apply(&pool, writer, vec![prefix_in(d, "10.9.0.0/16")]).await;
    assert!(
        client
            .drain(Duration::from_secs(1))
            .await
            .iter()
            .all(|e| e.event != "changes"),
        "nothing of the former scope"
    );
    sqlx::query("UPDATE person SET disabled_at = now() WHERE id = $1")
        .bind(reader.identity)
        .execute(&pool)
        .await
        .unwrap();
    let ended = tokio::time::timeout(WAIT, async {
        loop {
            match client.body.frame().await {
                None | Some(Err(_)) => return true,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert_eq!(ended, Ok(true), "the stream ends");
}

/// REQ-022-22: beyond the ceiling of an identity, 429 with `Retry-After`; beyond that of
/// the instance, 503; an ordinary read still passes.
#[kartenn_testing::pg_test]
async fn the_ceilings_refuse_before_the_base(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let i = instance(
        &pool,
        &ring,
        EventSettings {
            per_identity: 2,
            max_streams: 3,
            ..quick()
        },
    )
    .await;
    let alice = app(
        &pool,
        &ring,
        &i.hub,
        person(&pool, &READ_ALL, ("global", None)).await,
    );
    let bob = app(
        &pool,
        &ring,
        &i.hub,
        person(&pool, &READ_ALL, ("global", None)).await,
    );
    let (r1, _c1) = open(&alice, "/v1/events", None).await;
    let (r2, _c2) = open(&alice, "/v1/events", None).await;
    assert_eq!((r1.status(), r2.status()), (StatusCode::OK, StatusCode::OK));
    let (r3, _) = open(&alice, "/v1/events", None).await;
    assert_eq!(r3.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(r3.headers().contains_key(header::RETRY_AFTER));
    let (r4, c4) = open(&bob, "/v1/events", None).await;
    assert_eq!(r4.status(), StatusCode::OK);
    let (r5, _) = open(&bob, "/v1/events", None).await;
    assert_eq!(r5.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(r5.headers().contains_key(header::RETRY_AFTER));
    let read = alice
        .clone()
        .oneshot(
            Request::get("/v1/capabilities")
                .header(header::AUTHORIZATION, "Bearer t")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    // A stream closed frees its place.
    drop(c4);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let (r6, _c6) = open(&bob, "/v1/events", None).await;
    assert_eq!(r6.status(), StatusCode::OK);
}

/// REQ-022-12: at the stop, a `retry:` drawn between 0.5 and 5 s, then the end; new
/// streams 503.
#[kartenn_testing::pg_test]
async fn the_stop_spreads_the_reconnections_and_ends_the_streams(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let i = instance(&pool, &ring, quick()).await;
    let app = app(
        &pool,
        &ring,
        &i.hub,
        person(&pool, &READ_ALL, ("global", None)).await,
    );
    let mut clients = Vec::new();
    for _ in 0..10 {
        let (_, mut c) = open(&app, "/v1/events", None).await;
        c.next(WAIT).await.unwrap();
        clients.push(c);
    }
    let started = std::time::Instant::now();
    i.hub.shutdown();
    for mut c in clients {
        let ended = tokio::time::timeout(Duration::from_secs(2), async {
            while let Some(Ok(frame)) = c.body.frame().await {
                if let Ok(data) = frame.into_data() {
                    c.parser.feed(std::str::from_utf8(&data).unwrap());
                }
            }
        })
        .await;
        assert!(ended.is_ok(), "ended");
        let retry = c.parser.retry.unwrap();
        assert!((500..=5_000).contains(&retry), "{retry}");
    }
    assert!(started.elapsed() < Duration::from_secs(2));
    let (r, _) = open(&app, "/v1/events", None).await;
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
}

/// REQ-022-21: a client that stops reading delays no one; when it reads again, `reset`
/// `overflow` first.
#[kartenn_testing::pg_test]
async fn a_slow_subscriber_delays_no_one_and_starts_over(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let i = instance(
        &pool,
        &ring,
        EventSettings {
            buffer: 2,
            ..quick()
        },
    )
    .await;
    let writer = author(&pool).await;
    let d = domain(&pool, writer, "prod").await;
    let app = app(
        &pool,
        &ring,
        &i.hub,
        person(&pool, &READ_ALL, ("global", None)).await,
    );
    let (_, mut slow) = open(&app, "/v1/events", None).await;
    let (_, mut quick_one) = open(&app, "/v1/events", None).await;
    assert_eq!(quick_one.next(WAIT).await.unwrap().event, "ready");
    // The quick one reads each event as it comes, the slow one nothing.
    for n in 0..8 {
        let c = apply(&pool, writer, vec![prefix_in(d, &format!("10.{n}.0.0/16"))]).await;
        let started = std::time::Instant::now();
        loop {
            // The domain, written before the opening, may come first.
            let e = quick_one.next_of("changes", WAIT).await.expect("on time");
            if data(&e)["changeset"] == json!(c) {
                break;
            }
        }
        assert!(started.elapsed() < Duration::from_secs(2));
    }
    // The slow one reads again: `ready`, then the `reset` of its overflow.
    assert_eq!(slow.next(WAIT).await.unwrap().event, "ready");
    let reset = slow.next_of("reset", WAIT).await.expect("reset");
    assert_eq!(data(&reset)["reason"], "overflow");
}

/// A generator of draws, from a seed.
struct Draws(u64);

impl Draws {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

/// REQ-022-16, 17: for subscribers of every kind of scope, an external operator confined
/// to its entity among them, the objects delivered are exactly those the unitary decision
/// lets them read.
#[kartenn_testing::pg_test]
async fn the_objects_delivered_are_those_each_subscriber_may_read(pool: PgPool) {
    use kartenn_service::ObjectKind;
    use kartenn_service::authz::Authorizer;
    use kartenn_service::declarations::{core_declarations, sync_modules};
    use kartenn_service::operations::RawInput;
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let store = PgStore::new(pool.clone());
    sync_modules(
        &store,
        &Unrestricted,
        &[core_declarations()],
        MAX_OPERATIONS,
    )
    .await
    .unwrap();
    let writer = author(&pool).await;
    let entity = |local, slug: &str| Operation::CreateEntity {
        local: Some(local),
        slug: slug.into(),
        parent: None,
    };
    apply(&pool, writer, vec![entity(1, "group"), entity(2, "vendor")]).await;
    let id_of = |table: &'static str, column: &'static str, value: String| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(sqlx::AssertSqlSafe(format!(
                "SELECT id FROM {table} WHERE {column}::text = $1 LIMIT 1"
            )))
            .bind(value)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let group = id_of("entity", "slug", "group".into()).await;
    let vendor = id_of("entity", "slug", "vendor".into()).await;
    let owner = id_of("property_declaration", "key", "core.owner".into()).await;
    let owned = |object: Ref, kind, entity: Uuid| Operation::AssignProperty {
        object,
        kind,
        property: Ref::Id(owner),
        values: vec![RawInput::Text(entity.to_string())],
    };
    apply(
        &pool,
        writer,
        vec![
            Operation::CreateDomain {
                local: Some(1),
                slug: "lab".into(),
            },
            owned(Ref::Local(1), ObjectKind::Domain, group),
            Operation::CreateDomain {
                local: Some(2),
                slug: "other".into(),
            },
            owned(Ref::Local(2), ObjectKind::Domain, vendor),
        ],
    )
    .await;
    let lab = id_of("addressing_domain", "slug", "lab".into()).await;
    let other = id_of("addressing_domain", "slug", "other".into()).await;
    apply(&pool, writer, vec![prefix_in(lab, "10.0.0.0/8")]).await;
    let container = id_of("prefix", "prefix", "10.0.0.0/8".into()).await;
    let i = instance(&pool, &ring, quick()).await;
    // An external operator: its issuer confined to the vendor.
    let vendor_idp = "https://vendor.example.invalid";
    sqlx::query(
        "INSERT INTO issuer_attachment (issuer, entity_id, valid_period, changeset_id)
         VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(vendor_idp)
    .bind(vendor)
    .execute(&pool)
    .await
    .unwrap();
    let mut operator = person(&pool, &READ_ALL, ("global", None)).await;
    operator.issuer = Some(vendor_idp.into());
    let subscribers = vec![
        ("global", person(&pool, &READ_ALL, ("global", None)).await),
        ("lab", person(&pool, &READ_ALL, ("domain", Some(lab))).await),
        (
            "vendor",
            person(&pool, &READ_ALL, ("entity", Some(vendor))).await,
        ),
        (
            "subtree",
            person(&pool, &READ_ALL, ("prefix", Some(container))).await,
        ),
        ("operator", operator),
        ("nobody", person(&pool, &[], ("global", None)).await),
    ];
    let mut clients = Vec::new();
    for (name, who) in &subscribers {
        let app = app(&pool, &ring, &i.hub, who.clone());
        let (_, mut c) = open(&app, "/v1/events", None).await;
        assert_eq!(c.next(WAIT).await.unwrap().event, "ready", "{name}");
        clients.push(c);
    }
    // Changesets drawn from a seed: one or two prefixes in either domain, some owned by
    // the vendor.
    let mut draws = Draws(0x9e37_79b9_7f4a_7c15);
    let mut written: Vec<Uuid> = Vec::new();
    for n in 0..12u32 {
        let mut operations = Vec::new();
        for k in 0..=draws.next(2) {
            let (d, base) = if draws.next(2) == 0 {
                (lab, "10")
            } else {
                (other, "172")
            };
            let local = u32::try_from(100 + k).unwrap();
            operations.push(Operation::CreatePrefix {
                local: Some(local),
                domain: Ref::Id(d),
                prefix: format!("{base}.{}.{k}.0/24", 16 + n),
                nature: Nature::Container,
                point_to_point: false,
                anycast: false,
            });
            if draws.next(3) == 0 {
                operations.push(owned(Ref::Local(local), ObjectKind::Prefix, vendor));
            }
        }
        apply(&pool, writer, operations).await;
        let ids: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM prefix WHERE split_part(host(prefix), '.', 2) = $1")
                .bind((16 + n).to_string())
                .fetch_all(&pool)
                .await
                .unwrap();
        written.extend(ids);
    }
    let authorizer = kartenn_store_pg::rbac::PgAuthorizer::new(pool.clone());
    for ((name, who), client) in subscribers.iter().zip(clients.iter_mut()) {
        let mut delivered = std::collections::BTreeSet::new();
        for e in client.drain(Duration::from_secs(2)).await {
            if e.event == "changes" {
                for o in data(&e)["objects"].as_array().into_iter().flatten() {
                    let id = Uuid::parse_str(o["id"].as_str().unwrap()).unwrap();
                    if written.contains(&id) {
                        delivered.insert(id);
                    }
                }
            }
        }
        let mut expected = std::collections::BTreeSet::new();
        for id in &written {
            let denied = authorizer
                .authorize_many(&who.subject(), "core.prefix.read", &[*id])
                .await
                .unwrap();
            if denied.is_empty() {
                expected.insert(*id);
            }
        }
        assert_eq!(delivered, expected, "{name}");
    }
}

/// The changesets of the `changes` events among `events`; a second one on the same
/// connection (`segment`) fails.
fn changesets(events: Vec<Parsed>, segment: &mut std::collections::BTreeSet<Uuid>) -> Vec<Uuid> {
    let mut out = Vec::new();
    for e in events {
        if e.event == "changes" {
            let c: Uuid = serde_json::from_value(data(&e)["changeset"].clone()).unwrap();
            assert!(segment.insert(c), "a duplicate without resumption: {c}");
            out.push(c);
        }
    }
    out
}

/// REQ-022-7, 27: two instances, a long transaction that holds the lowest number, cuts
/// and resumptions drawn at random on either: every changeset committed is received, and
/// no connection receives one twice without a resumption (a duplicate after one repeats
/// an identity already received).
#[kartenn_testing::pg_test]
async fn resumptions_lose_nothing_and_duplicate_only_on_resumption(pool: PgPool) {
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let a = instance(&pool, &ring, quick()).await;
    let b = instance(&pool, &ring, quick()).await;
    let writer = author(&pool).await;
    let d = domain(&pool, writer, "prod").await;
    let e = domain(&pool, writer, "else").await;
    apply(&pool, writer, vec![prefix_in(e, "192.168.0.0/16")]).await;
    let held: Uuid = sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = '192.168.0.0/16'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let reader = person(&pool, &READ_ALL, ("global", None)).await;
    let apps = [
        app(&pool, &ring, &a.hub, reader.clone()),
        app(&pool, &ring, &b.hub, reader.clone()),
    ];
    let (_, mut client) = open(&apps[0], "/v1/events", None).await;
    client.next(WAIT).await.unwrap();
    // The long transaction: an event on a visible object, numbered first, committed late.
    let late = Uuid::now_v7();
    let mut long = Some(pool.begin().await.unwrap());
    if let Some(tx) = long.as_mut() {
        sqlx::query(
            "INSERT INTO changeset (id, author_id, motive, state, kind, default_period)
             VALUES ($1, $2, 'late', 'applied', 'direct', tstzrange(now(), NULL))",
        )
        .bind(late)
        .bind(writer)
        .execute(&mut **tx)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO outbox (changeset_id, domain_ids, payload) VALUES ($1, ARRAY[$2::uuid],
                 jsonb_build_object('type', 'org.kartenn.changeset.applied', 'id', $1::text,
                                    'time', now(),
                                    'data', jsonb_build_object('changeset', $1, 'objects',
                                                               jsonb_build_array($3::uuid))))",
        )
        .bind(late)
        .bind(e)
        .bind(held)
        .execute(&mut **tx)
        .await
        .unwrap();
    }
    let mut draws = Draws(42);
    let mut committed = Vec::new();
    let mut received: Vec<Uuid> = Vec::new();
    let mut segment = std::collections::BTreeSet::new();
    for n in 0..10u32 {
        committed.push(apply(&pool, writer, vec![prefix_in(d, &format!("10.{n}.0.0/16"))]).await);
        if n == 6
            && let Some(tx) = long.take()
        {
            tx.commit().await.unwrap();
            committed.push(late);
        }
        let events = client
            .drain(Duration::from_millis(150 + draws.next(300)))
            .await;
        received.extend(changesets(events, &mut segment));
        if draws.next(2) == 0 {
            // Cut, then resume on either instance.
            let last = client.parser.last_id.clone();
            let on = usize::try_from(draws.next(2)).unwrap();
            let (_, mut next) = open(&apps[on], "/v1/events", Some(&last)).await;
            assert_eq!(next.next(WAIT).await.unwrap().event, "ready");
            client = next;
            segment.clear();
        }
    }
    let events = client.drain(Duration::from_secs(2)).await;
    received.extend(changesets(events, &mut segment));
    let got: std::collections::BTreeSet<Uuid> = received.iter().copied().collect();
    for c in &committed {
        assert!(got.contains(c), "lost: {c}");
    }
    assert!(received.len() >= committed.len());
}
