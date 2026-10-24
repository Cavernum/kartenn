//! Authentication (spec 006, REQ-006-26, 27; task 006-6): an access token gives an
//! identity just in time without a login event; an expired one → `invalid_token`; no
//! proof → 401 without error code; two proofs → 400; Basic and the session cookie
//! refused without TLS, after the trusted proxies; the failures limited per source.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{ConnectInfo, Extension};
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use http_body_util::BodyExt;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use kartenn_auth::http::HttpClient;
use kartenn_auth::issuer::IssuerConfig;
use kartenn_auth::keys::KeyRing;
use kartenn_auth::oidc::{LoggedIn, Oidc, Session};
use kartenn_auth::secrets::{FailureLimiter, generate};
use kartenn_auth::tokens::{KeySource, TokenError, TokenVerifier};
use kartenn_rest::authn::Authn;
use kartenn_rest::health::Readiness;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::{ConnectionTls, Settings};
use kartenn_service::aggregate::FailureAggregator;
use kartenn_service::audit::{Instance, RequestContext};
use kartenn_service::authn::{Identities, Principal, StateCache};
use kartenn_service::authz::Unrestricted;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use p256::pkcs8::EncodePrivateKey;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const ISSUER: &str = "https://idp.example.invalid";

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

/// The keys the issuer publishes.
struct Published(Vec<Jwk>);

impl KeySource for Published {
    fn keys(&self, _: &IssuerConfig) -> impl Future<Output = Result<JwkSet, TokenError>> + Send {
        std::future::ready(Ok(JwkSet {
            keys: self.0.clone(),
        }))
    }
}

fn issuer() -> IssuerConfig {
    IssuerConfig {
        id: "main".into(),
        issuer: ISSUER.into(),
        client_id: "kartenn".into(),
        client_secret: "s".into(),
        audience: "kartenn".into(),
        groups_claim: "groups".into(),
        algorithms: vec![Algorithm::ES256],
        native: None,
    }
}

fn unix_now() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap()
}

struct Fixture {
    pool: PgPool,
    store: PgStore,
    ring: KeyRing,
    signing: EncodingKey,
    app: Router,
}

impl Fixture {
    fn new(pool: &PgPool, failures: u32) -> Self {
        let secret = p256::SecretKey::random(&mut rand_core::OsRng);
        let pem = secret
            .to_pkcs8_pem(p256::pkcs8::LineEnding::default())
            .unwrap();
        let signing = EncodingKey::from_ec_pem(pem.as_bytes()).unwrap();
        let mut jwk = Jwk::from_encoding_key(&signing, Algorithm::ES256).unwrap();
        jwk.common.key_id = Some("k1".into());
        let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
        let store = PgStore::new(pool.clone());
        let authn = Authn {
            identities: store.clone(),
            journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
            ring: ring.clone(),
            oidc: Oidc::new(
                TokenVerifier::new(vec![issuer()], Published(vec![jwk])),
                HttpClient::new().unwrap(),
                ring.clone(),
                "https://kartenn.example.invalid/v1/auth/callback".into(),
            ),
            states: StateCache::new(Duration::from_secs(5)),
            limiter: FailureLimiter::new(failures, Duration::from_mins(1)),
            aggregator: FailureAggregator::new(10, Duration::from_mins(1)),
            idle: Duration::from_mins(30),
            absolute: Duration::from_hours(8),
        };
        let api = Router::new().route(
            "/whoami",
            get(
                |Extension(principal): Extension<Principal>,
                 Extension(context): Extension<RequestContext>| async move {
                    Json(json!({
                        "identity": principal.identity,
                        "method": principal.method.as_str(),
                        "actor": context.actor.id,
                        "request_id": context.correlation.request_id,
                    }))
                },
            ),
        );
        let app = kartenn_rest::router(
            api,
            Arc::new(Ready),
            Settings {
                trusted_proxies: vec!["10.0.0.0/8".parse().unwrap()],
                ..Settings::default()
            },
            Arc::new(Quotas::new(Rates::default())),
            Arc::new(authn),
        );
        Self {
            pool: pool.clone(),
            store,
            ring,
            signing,
            app,
        }
    }

    fn token(&self, expires_in: i64) -> String {
        let now = unix_now();
        let claims = json!({
            "iss": ISSUER, "aud": "kartenn", "sub": "u-1", "client_id": "cli",
            "iat": now, "exp": now + expires_in, "jti": Uuid::now_v7(),
            "name": "A. Person", "groups": ["netops"]
        });
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some("k1".into());
        header.typ = Some("at+jwt".into());
        encode(&header, &claims, &self.signing).unwrap()
    }

    /// A service account `svc-backup` and its secret, as a `Basic` header value.
    async fn service_account(&self) -> String {
        let admin: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'admin') RETURNING id",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let secret = generate(&self.ring, None);
        let request = ChangesetRequest {
            author: admin,
            motive: "test".into(),
            period: None,
            operations: vec![Operation::CreateServiceAccount {
                local: None,
                name: "svc-backup".into(),
                secret: secret.record,
            }],
            simulate: false,
            idempotency_key: None,
            apply_own: true,
        };
        let outcome = submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        format!(
            "Basic {}",
            STANDARD.encode(format!("svc-backup:{}", secret.plaintext))
        )
    }

    /// The cookie of a session of a person, opened now.
    async fn session(&self) -> String {
        let (identity, _) = self
            .store
            .upsert_person(ISSUER, "u-2", "B. Person")
            .await
            .unwrap();
        let login = LoggedIn {
            issuer_id: "main".into(),
            issuer: ISSUER.into(),
            subject: "u-2".into(),
            name: None,
            groups: Vec::new(),
            refresh_token: None,
            id_token: "id".into(),
            expires_in: 300,
        };
        let chunks = Session::new(identity, &login).seal(&self.ring).unwrap();
        assert_eq!(chunks.len(), 1);
        format!("__Host-kartenn={}", chunks[0])
    }

    async fn call(&self, request: Request<Body>) -> Response {
        self.app.clone().oneshot(request).await.unwrap()
    }

    async fn events(&self, name: &str) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = $1")
            .bind(name)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

/// A request to `/v1/whoami` from `peer`, over TLS or not, with `headers`.
fn me(peer: &str, tls: bool, headers: &[(header::HeaderName, &str)]) -> Request<Body> {
    let mut request = Request::get("/v1/whoami");
    for (name, value) in headers {
        request = request.header(name, *value);
    }
    let mut request = request.body(Body::empty()).unwrap();
    let peer = format!("{peer}:40000").parse().unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo::<std::net::SocketAddr>(peer));
    request.extensions_mut().insert(ConnectionTls(tls));
    request
}

async fn json(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

fn challenge(response: &Response) -> &str {
    response.headers()[header::WWW_AUTHENTICATE]
        .to_str()
        .unwrap()
}

#[kartenn_testing::pg_test]
async fn a_token_gives_an_identity_just_in_time_without_a_login_event(pool: PgPool) {
    let f = Fixture::new(&pool, 20);
    let bearer = format!("Bearer {}", f.token(300));
    let mut identities = Vec::new();
    for _ in 0..3 {
        let response = f
            .call(me("192.0.2.9", false, &[(header::AUTHORIZATION, &bearer)]))
            .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Bearer needs no TLS here"
        );
        let body = json(response).await;
        assert_eq!(body["method"], "bearer");
        assert_eq!(
            body["actor"], body["identity"],
            "the context is the actor's"
        );
        assert!(body["request_id"].is_string(), "the correlation kept");
        identities.push(body["identity"].clone());
    }
    assert!(identities.iter().all(|i| *i == identities[0]));
    let persons: i64 =
        sqlx::query_scalar("SELECT count(*) FROM person WHERE issuer = $1 AND subject = 'u-1'")
            .bind(ISSUER)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(persons, 1, "created at the first request");
    assert_eq!(f.events("authn.login.succeeded").await, 0);
}

#[kartenn_testing::pg_test]
async fn a_refused_token_is_an_invalid_token(pool: PgPool) {
    let f = Fixture::new(&pool, 20);
    for token in [f.token(-3600), "not.a.jwt".into()] {
        let response = f
            .call(me(
                "192.0.2.9",
                true,
                &[(header::AUTHORIZATION, &format!("Bearer {token}"))],
            ))
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            challenge(&response),
            "Bearer realm=\"kartenn\", error=\"invalid_token\""
        );
        assert_eq!(
            json(response).await["type"],
            "urn:kartenn:problem:unauthenticated"
        );
    }
    let reasons: Vec<String> = sqlx::query_scalar(
        "SELECT reason FROM audit.event WHERE name = 'authn.login.failed' ORDER BY seq",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(reasons, ["expired", "malformed"]);
}

#[kartenn_testing::pg_test]
async fn without_proof_401_without_error_and_two_proofs_400(pool: PgPool) {
    let f = Fixture::new(&pool, 20);
    let response = f.call(me("192.0.2.9", true, &[])).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(challenge(&response), "Bearer realm=\"kartenn\"");
    assert!(response.headers().contains_key("request-id"));

    let bearer = format!("Bearer {}", f.token(300));
    let cookie = f.session().await;
    let response = f
        .call(me(
            "192.0.2.9",
            true,
            &[(header::AUTHORIZATION, &bearer), (header::COOKIE, &cookie)],
        ))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let probe = f
        .call(Request::get("/health/live").body(Body::empty()).unwrap())
        .await;
    assert_eq!(probe.status(), StatusCode::OK, "the probes are public");
}

#[kartenn_testing::pg_test]
async fn basic_and_cookies_need_tls_after_the_trusted_proxies(pool: PgPool) {
    let f = Fixture::new(&pool, 100);
    let basic = f.service_account().await;
    let cookie = f.session().await;
    // (peer, TLS to kartenn, Forwarded, the client spoke TLS)
    let matrix = [
        ("192.0.2.9", true, None, true),
        ("192.0.2.9", false, None, false),
        (
            "10.0.0.5",
            false,
            Some("for=198.51.100.1;proto=https"),
            true,
        ),
        (
            "10.0.0.5",
            false,
            Some("for=198.51.100.1;proto=http"),
            false,
        ),
        ("10.0.0.5", true, Some("for=198.51.100.1;proto=http"), false),
        (
            "192.0.2.9",
            false,
            Some("for=198.51.100.1;proto=https"),
            false,
        ),
        ("192.0.2.9", true, Some("for=198.51.100.1;proto=http"), true),
    ];
    for (peer, tls, forwarded, secure) in matrix {
        for (name, value, method) in [
            (header::AUTHORIZATION, basic.as_str(), "basic"),
            (header::COOKIE, cookie.as_str(), "oidc"),
        ] {
            let mut headers = vec![(name, value)];
            if let Some(forwarded) = forwarded {
                headers.push((header::FORWARDED, forwarded));
            }
            let response = f.call(me(peer, tls, &headers)).await;
            let case = format!("{method} from {peer}, tls {tls}, {forwarded:?}");
            if secure {
                assert_eq!(response.status(), StatusCode::OK, "{case}");
                assert_eq!(json(response).await["method"], method, "{case}");
            } else {
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{case}");
                assert_eq!(challenge(&response), "Bearer realm=\"kartenn\"", "{case}");
            }
        }
    }
}

#[kartenn_testing::pg_test]
async fn the_failures_of_a_source_are_limited(pool: PgPool) {
    let f = Fixture::new(&pool, 3);
    let basic = f.service_account().await;
    let wrong = format!("Basic {}", STANDARD.encode("svc-backup:guess"));
    for _ in 0..3 {
        let response = f
            .call(me("192.0.2.9", true, &[(header::AUTHORIZATION, &wrong)]))
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = f
        .call(me("192.0.2.9", true, &[(header::AUTHORIZATION, &basic)]))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "even the right secret"
    );
    let response = f
        .call(me("192.0.2.10", true, &[(header::AUTHORIZATION, &basic)]))
        .await;
    assert_eq!(response.status(), StatusCode::OK, "another source");
    assert_eq!(f.events("authn.login.failed").await, 3);
}
