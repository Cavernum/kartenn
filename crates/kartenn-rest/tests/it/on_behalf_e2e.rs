//! `Kartenn-On-Behalf-Of` end to end (spec 016, REQ-016-12, 13; G10, INV-60): the real
//! authentication of the router (`Authn`: Basic secrets of the service accounts, access
//! tokens of an issuer) in front of the real API. A service account bound to
//! `core.identity.act_on_behalf` writes for a requester, named on the changeset and in the
//! audit, under its own rights only; without the permission, or from a person: 403 and an
//! `authz.access.denied`; malformed or repeated: 400; the real person who logs in later
//! takes back the record, kind `person` and the issuer's name.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use http_body_util::BodyExt;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use kartenn_auth::http::HttpClient;
use kartenn_auth::issuer::IssuerConfig;
use kartenn_auth::keys::KeyRing;
use kartenn_auth::oidc::Oidc;
use kartenn_auth::secrets::{FailureLimiter, generate};
use kartenn_auth::tokens::{KeySource, TokenError, TokenVerifier};
use kartenn_rest::authn::Authn;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::{ConnectionTls, Settings};
use kartenn_service::aggregate::FailureAggregator;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::StateCache;
use kartenn_service::authz::Unrestricted;
use kartenn_service::operations::{BindingScope, BindingSubject};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ChangesetRequest, Operation, Outcome, submit};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::write::PgStore;
use p256::pkcs8::EncodePrivateKey;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const ISSUER: &str = "https://idp.example.invalid";

/// The keys the issuer publishes.
struct Published(Vec<Jwk>);

impl KeySource for Published {
    fn keys(
        &self,
        _: &IssuerConfig,
    ) -> impl std::future::Future<Output = Result<JwkSet, TokenError>> + Send {
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
    admin: Uuid,
}

impl Fixture {
    async fn new(pool: &PgPool) -> Self {
        let secret = p256::SecretKey::random(&mut rand_core::OsRng);
        let pem = secret
            .to_pkcs8_pem(p256::pkcs8::LineEnding::default())
            .unwrap();
        let signing = EncodingKey::from_ec_pem(pem.as_bytes()).unwrap();
        let mut jwk = Jwk::from_encoding_key(&signing, Algorithm::ES256).unwrap();
        jwk.common.key_id = Some("k1".into());
        let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
        let store = PgStore::new(pool.clone());
        let journal = || PgAuditJournal::new(pool.clone(), Instance::new("test"));
        let authn = Authn {
            identities: store.clone(),
            journal: journal(),
            ring: ring.clone(),
            oidc: Oidc::new(
                TokenVerifier::new(vec![issuer()], Published(vec![jwk])),
                HttpClient::new().unwrap(),
                ring.clone(),
                "https://kartenn.example.invalid/v1/auth/callback".into(),
            ),
            states: StateCache::new(Duration::from_secs(5)),
            limiter: FailureLimiter::new(100, Duration::from_mins(1)),
            aggregator: FailureAggregator::new(10, Duration::from_mins(1)),
            idle: Duration::from_mins(30),
            absolute: Duration::from_hours(8),
        };
        let api = Api {
            pool: pool.clone(),
            ring: ring.clone(),
            bound: Duration::from_secs(60),
            journal: journal(),
            audit: ReadAudit::default(),
            sources: kartenn_service::expected::Sources::none(),
            outputs: Arc::from([]),
            modules: Arc::default(),
        };
        let app = kartenn_rest::router(
            kartenn_rest::v1(api),
            Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
            Settings::default(),
            Arc::new(Quotas::new(Rates::default())),
            Arc::new(authn),
        );
        let admin: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'admin') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        Self {
            pool: pool.clone(),
            store,
            ring,
            signing,
            app,
            admin,
        }
    }

    fn apply(&self) -> impl Fn(Vec<Operation>) -> ChangesetRequest + '_ {
        |operations| ChangesetRequest {
            author: self.admin,
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
            apply_own: true,
        }
    }

    /// A service account `name`, bound everywhere to a role of `entries`: its identity and
    /// its `Basic` header value.
    async fn service_account(&self, name: &str, entries: &[&str]) -> (Uuid, String) {
        let secret = generate(&self.ring, None);
        let created = submit(
            &self.store,
            &Unrestricted,
            &self.apply()(vec![Operation::CreateServiceAccount {
                local: None,
                name: name.into(),
                secret: secret.record,
            }]),
            MAX_OPERATIONS,
        )
        .await
        .unwrap();
        assert!(matches!(created, Outcome::Applied { .. }), "{created:?}");
        let id: Uuid = sqlx::query_scalar(
            "SELECT id FROM person WHERE kind = 'service_account' AND display_name = $1",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let role = format!("role_{}", name.replace('-', "_"));
        let bound = submit(
            &self.store,
            &Unrestricted,
            &self.apply()(vec![
                Operation::SetRole {
                    key: role.clone(),
                    labels: [("fr".into(), role.clone())].into(),
                    entries: entries.iter().map(|e| (*e).to_owned()).collect(),
                },
                Operation::Bind {
                    local: None,
                    subject: BindingSubject::Identity { id },
                    role,
                    scope: BindingScope::Global,
                },
            ]),
            MAX_OPERATIONS,
        )
        .await
        .unwrap();
        assert!(matches!(bound, Outcome::Applied { .. }), "{bound:?}");
        let basic = format!(
            "Basic {}",
            STANDARD.encode(format!("{name}:{}", secret.plaintext))
        );
        (id, basic)
    }

    /// An access token of the issuer for the person `sub`, named `name`.
    fn token(&self, sub: &str, name: &str) -> String {
        let now = unix_now();
        let claims = json!({
            "iss": ISSUER, "aud": "kartenn", "sub": sub, "client_id": "cli",
            "iat": now, "exp": now + 300, "jti": Uuid::now_v7(),
            "name": name, "groups": []
        });
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some("k1".into());
        header.typ = Some("at+jwt".into());
        format!(
            "Bearer {}",
            encode(&header, &claims, &self.signing).unwrap()
        )
    }

    async fn call(&self, request: Request<Body>) -> Response {
        self.app
            .clone()
            .oneshot(crate::common::applied(request))
            .await
            .unwrap()
    }

    async fn denied(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM audit.event WHERE name = 'authz.access.denied'")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

/// A request over TLS from a client, with `authorization` and the `on_behalf` headers.
fn request(
    method: &str,
    uri: &str,
    authorization: &str,
    on_behalf: &[&str],
    body: Option<Value>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, authorization);
    for value in on_behalf {
        builder = builder.header("Kartenn-On-Behalf-Of", *value);
    }
    let mut request = match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    request
        .extensions_mut()
        .insert(ConnectInfo::<std::net::SocketAddr>(
            "192.0.2.10:40000".parse().unwrap(),
        ));
    request.extensions_mut().insert(ConnectionTls(true));
    request
}

#[allow(clippy::unnecessary_wraps)] // the body argument of `request`
fn entity(slug: &str) -> Option<Value> {
    Some(json!({"motive": "for a ticket", "slug": slug}))
}

const JDOE: &str = r#"sub="u-7", iss="https://idp.example.invalid", name="Named by the connector""#;

async fn json_of(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap_or(Value::Null)
}

// REQ-016-12, REQ-016-13, INV-60: the requester is named; the rights stay the account's.
#[kartenn_testing::pg_test]
async fn a_service_account_writes_for_its_requester_through_the_real_authentication(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let (account, basic) = f
        .service_account(
            "svc-tickets",
            &[
                "core.identity.act_on_behalf",
                "core.entity.read",
                "core.entity.edit",
                "core.entity.apply_own",
            ],
        )
        .await;
    let response = f
        .call(request(
            "POST",
            "/v1/entities",
            &basic,
            &[JDOE],
            entity("unit-a"),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let requester: (Uuid, String, String) = sqlx::query_as(
        "SELECT id, kind, display_name FROM person WHERE issuer = $1 AND subject = 'u-7'",
    )
    .bind(ISSUER)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(requester.1, "requester");
    // The changeset names the requester; its author stays the service account.
    let on_behalf: Option<Uuid> = sqlx::query_scalar(
        "SELECT on_behalf_of FROM changeset WHERE author_id = $1
         ORDER BY submitted_at DESC LIMIT 1",
    )
    .bind(account)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(on_behalf, Some(requester.0));
    // Every event of the request names it.
    let events: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(e) FROM audit.event e
         WHERE actor_id = $1 AND name <> 'authn.requester.created' ORDER BY seq",
    )
    .bind(account)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!events.is_empty());
    for event in &events {
        assert_eq!(
            event["details"]["on_behalf_of"],
            requester.0.to_string(),
            "{event}"
        );
    }
    // The requester's name opens no right: a site the account may not write stays refused.
    let site = f
        .call(request(
            "POST",
            "/v1/sites",
            &basic,
            &[JDOE],
            Some(json!({"motive": "for a ticket", "slug": "site-a"})),
        ))
        .await;
    assert!(
        site.status() == StatusCode::FORBIDDEN || site.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        site.status()
    );
}

// G10, INV-60: without the permission, the attempt is refused and audited.
#[kartenn_testing::pg_test]
async fn a_service_account_without_the_permission_is_refused_and_audited(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let (_, basic) = f
        .service_account(
            "svc-plain",
            &[
                "core.entity.read",
                "core.entity.edit",
                "core.entity.apply_own",
            ],
        )
        .await;
    let before = f.denied().await;
    let response = f
        .call(request(
            "POST",
            "/v1/entities",
            &basic,
            &[JDOE],
            entity("unit-b"),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(f.denied().await, before + 1);
    // No requester was created for it.
    let named: i64 = sqlx::query_scalar("SELECT count(*) FROM person WHERE subject = 'u-7'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(named, 0);
    // Without the header, the same account writes.
    let response = f
        .call(request(
            "POST",
            "/v1/entities",
            &basic,
            &[],
            entity("unit-b"),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

// G10, INV-60: a person never acts for another.
#[kartenn_testing::pg_test]
async fn a_person_with_the_header_is_refused_and_audited(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let before = f.denied().await;
    let response = f
        .call(request(
            "POST",
            "/v1/entities",
            &f.token("u-2", "B. Person"),
            &[JDOE],
            entity("unit-c"),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        json_of(response).await["detail"],
        "core.identity.act_on_behalf"
    );
    assert_eq!(f.denied().await, before + 1);
}

// REQ-016-12: a malformed or repeated header names no one.
#[kartenn_testing::pg_test]
async fn a_malformed_or_repeated_header_is_a_bad_request(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let (_, basic) = f
        .service_account(
            "svc-tickets",
            &[
                "core.identity.act_on_behalf",
                "core.entity.read",
                "core.entity.edit",
                "core.entity.apply_own",
            ],
        )
        .await;
    for headers in [
        &[r#"iss="https://idp.example.invalid""#][..],
        &[JDOE, JDOE][..],
    ] {
        let response = f
            .call(request(
                "POST",
                "/v1/entities",
                &basic,
                headers,
                entity("unit-d"),
            ))
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{headers:?}");
    }
    let entities: i64 = sqlx::query_scalar("SELECT count(*) FROM entity WHERE slug = 'unit-d'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entities, 0);
}

// INV-60 (audit of 016): the requester a connector named takes back nothing from the
// person who logs in: the record becomes `person`, with the issuer's name.
#[kartenn_testing::pg_test]
async fn the_real_person_logging_in_takes_back_the_requester_record(pool: PgPool) {
    let f = Fixture::new(&pool).await;
    let (_, basic) = f
        .service_account(
            "svc-tickets",
            &[
                "core.identity.act_on_behalf",
                "core.entity.read",
                "core.entity.edit",
                "core.entity.apply_own",
            ],
        )
        .await;
    let response = f
        .call(request(
            "POST",
            "/v1/entities",
            &basic,
            &[JDOE],
            entity("unit-e"),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let named: Uuid = sqlx::query_scalar("SELECT id FROM person WHERE subject = 'u-7'")
        .fetch_one(&pool)
        .await
        .unwrap();
    // The person comes with an access token of the issuer.
    let me = f
        .call(request(
            "GET",
            "/v1/me",
            &f.token("u-7", "J. Doe"),
            &[],
            None,
        ))
        .await;
    assert_eq!(me.status(), StatusCode::OK);
    let (id, kind, name): (Uuid, String, String) = sqlx::query_as(
        "SELECT id, kind, display_name FROM person WHERE issuer = $1 AND subject = 'u-7'",
    )
    .bind(ISSUER)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(id, named, "one record for one person");
    assert_eq!(kind, "person");
    assert_eq!(name, "J. Doe");
    // No binding names the record: it holds no right of its own.
    let bindings: i64 = sqlx::query_scalar("SELECT count(*) FROM binding WHERE subject_id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bindings, 0);
}
