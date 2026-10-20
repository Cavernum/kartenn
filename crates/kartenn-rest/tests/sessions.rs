//! Login routes and sessions (spec 006, REQ-006-24, 25; task 006-7): the OIDC flow against
//! a test issuer, the callback played once, the attributes of the cookies, the session
//! sent again at most once a minute, its groups refreshed, the logout to the issuer, a
//! break-glass session refused after 1 h, no cookie without TLS; on a real port (task
//! 006-24), the cookies through TLS in HTTP/1.1 and HTTP/2, and TLS read from `Forwarded`.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use http_body_util::BodyExt;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use kartenn_auth::breakglass::{STEP, code, create};
use kartenn_auth::http::HttpClient;
use kartenn_auth::issuer::IssuerConfig;
use kartenn_auth::keys::KeyRing;
use kartenn_auth::oidc::{Oidc, Session};
use kartenn_auth::secrets::FailureLimiter;
use kartenn_auth::tokens::{HttpKeys, TokenVerifier};
use kartenn_rest::authn::Authn;
use kartenn_rest::health::Readiness;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::{ConnectionTls, Settings};
use kartenn_service::aggregate::FailureAggregator;
use kartenn_service::audit::Instance;
use kartenn_service::authn::StateCache;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::breakglass::Stored;
use kartenn_store_pg::write::PgStore;
use p256::pkcs8::EncodePrivateKey;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

mod wire;

const CLIENT: &str = "kartenn";
const REDIRECT: &str = "https://kartenn.example.invalid/v1/auth/callback";
const SESSION: &str = "__Host-kartenn";
const LOGIN: &str = "__Host-kartenn-login";

/// What the test issuer does.
struct Idp {
    base: String,
    signing: EncodingKey,
    jwk: Jwk,
    /// The nonce of each code issued.
    codes: HashMap<String, String>,
    groups: Vec<String>,
}

type Shared = Arc<Mutex<Idp>>;

fn unix_now() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap()
}

fn id_token(idp: &Idp, nonce: Option<&str>) -> String {
    let now = unix_now();
    let mut claims = json!({
        "iss": idp.base, "aud": CLIENT, "sub": "u-42", "name": "A. Person",
        "iat": now, "exp": now + 300, "groups": idp.groups,
    });
    if let Some(nonce) = nonce {
        claims["nonce"] = nonce.into();
    }
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some("k1".into());
    encode(&header, &claims, &idp.signing).unwrap()
}

async fn discovery(State(idp): State<Shared>) -> Json<Value> {
    let base = idp.lock().unwrap().base.clone();
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "jwks_uri": format!("{base}/jwks"),
        "end_session_endpoint": format!("{base}/logout"),
    }))
}

async fn jwks(State(idp): State<Shared>) -> Json<JwkSet> {
    Json(JwkSet {
        keys: vec![idp.lock().unwrap().jwk.clone()],
    })
}

/// The token endpoint: a code once, the refresh token `r1` (PKCE and the client are
/// checked by the tests of the 005).
async fn token(State(idp): State<Shared>, body: String) -> (StatusCode, Json<Value>) {
    let form: HashMap<String, String> = form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect();
    let mut idp = idp.lock().unwrap();
    let refused = (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": "invalid_grant" })),
    );
    let token = match form.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            let Some(nonce) = form.get("code").and_then(|c| idp.codes.remove(c)) else {
                return refused;
            };
            id_token(&idp, Some(&nonce))
        }
        Some("refresh_token") if form.get("refresh_token").map(String::as_str) == Some("r1") => {
            id_token(&idp, None)
        }
        _ => return refused,
    };
    (
        StatusCode::OK,
        Json(
            json!({ "id_token": token, "refresh_token": "r1", "expires_in": 300, "token_type": "Bearer", "access_token": "a" }),
        ),
    )
}

/// Starts the issuer on a local port; its base URL is its identifier.
async fn idp() -> Shared {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let secret = p256::SecretKey::random(&mut rand_core::OsRng);
    let pem = secret
        .to_pkcs8_pem(p256::pkcs8::LineEnding::default())
        .unwrap();
    let signing = EncodingKey::from_ec_pem(pem.as_bytes()).unwrap();
    let mut jwk = Jwk::from_encoding_key(&signing, Algorithm::ES256).unwrap();
    jwk.common.key_id = Some("k1".into());
    let idp: Shared = Arc::new(Mutex::new(Idp {
        base: format!("http://{}", listener.local_addr().unwrap()),
        signing,
        jwk,
        codes: HashMap::new(),
        groups: vec!["netops".into()],
    }));
    let routes = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/token", post(token))
        .with_state(Arc::clone(&idp));
    tokio::spawn(async move { axum::serve(listener, routes).await });
    idp
}

/// The query parameter `name` of `url`.
fn param(url: &str, name: &str) -> String {
    let query = url.split_once('?').unwrap().1;
    form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == name)
        .unwrap()
        .1
        .into_owned()
}

/// Plays the user at the issuer: a code for the login started at `url`.
fn authorize(idp: &Shared, url: &str) -> String {
    let code = Uuid::now_v7().to_string();
    idp.lock()
        .unwrap()
        .codes
        .insert(code.clone(), param(url, "nonce"));
    code
}

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

struct Fixture {
    pool: PgPool,
    ring: KeyRing,
    app: Router,
}

impl Fixture {
    fn new(pool: &PgPool, idp: &Shared) -> Self {
        Self::with(pool, idp, Settings::default())
    }

    fn with(pool: &PgPool, idp: &Shared, settings: Settings) -> Self {
        Self::of_issuers(pool, idp, settings, &["main"])
    }

    /// With the issuers `ids`, in this order, all at `idp`.
    fn of_issuers(pool: &PgPool, idp: &Shared, settings: Settings, ids: &[&str]) -> Self {
        let issuers = ids
            .iter()
            .map(|&id| IssuerConfig {
                id: id.into(),
                issuer: idp.lock().unwrap().base.clone(),
                client_id: CLIENT.into(),
                client_secret: "s".into(),
                audience: CLIENT.into(),
                groups_claim: "groups".into(),
                algorithms: vec![Algorithm::ES256],
            })
            .collect();
        let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
        let http = HttpClient::new().unwrap();
        let authn = Authn {
            identities: PgStore::new(pool.clone()),
            journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
            ring: ring.clone(),
            oidc: Oidc::new(
                TokenVerifier::new(issuers, HttpKeys(http.clone())),
                http,
                ring.clone(),
                REDIRECT.into(),
            ),
            states: StateCache::new(Duration::from_secs(5)),
            limiter: FailureLimiter::new(100, Duration::from_mins(1)),
            aggregator: FailureAggregator::new(1_000, Duration::from_mins(1)),
            idle: Duration::from_mins(30),
            absolute: Duration::from_hours(8),
        };
        let app = kartenn_rest::router(
            Router::new(),
            Arc::new(Ready),
            settings,
            Arc::new(Quotas::new(Rates::default())),
            Arc::new(authn),
        );
        Self {
            pool: pool.clone(),
            ring,
            app,
        }
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

    /// `session` sealed again after `change`, as a `Cookie` header value.
    fn forged(&self, session: &str, change: impl FnOnce(&mut Session)) -> String {
        let mut session = Session::unseal(&self.ring, &[session]).unwrap();
        change(&mut session);
        format!("{SESSION}={}", session.seal(&self.ring).unwrap()[0])
    }

    /// A login through the issuer: the value of the session cookie.
    async fn login(&self, idp: &Shared) -> String {
        let started = self
            .call(request("GET", "/v1/auth/login?issuer=main", None, true))
            .await;
        assert_eq!(started.status(), StatusCode::SEE_OTHER);
        let location = location_of(&started);
        let login = value(&started, LOGIN).unwrap();
        let code = authorize(idp, &location);
        let state = param(&location, "state");
        let back = self
            .call(request(
                "GET",
                &format!("/v1/auth/callback?code={code}&state={state}"),
                Some(&format!("{LOGIN}={login}")),
                true,
            ))
            .await;
        assert_eq!(back.status(), StatusCode::SEE_OTHER);
        value(&back, SESSION).unwrap()
    }
}

/// A request from a client of the Internet, over TLS or not.
fn request(method: &str, uri: &str, cookie: Option<&str>, tls: bool) -> Request<Body> {
    with_body(method, uri, cookie, tls, Body::empty())
}

fn with_body(
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    tls: bool,
    body: Body,
) -> Request<Body> {
    // As a browser sends it on the page of kartenn (REQ-006-28).
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("sec-fetch-site", "same-origin");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let mut request = request.body(body).unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo::<std::net::SocketAddr>(
            "192.0.2.1:40000".parse().unwrap(),
        ));
    request.extensions_mut().insert(ConnectionTls(tls));
    request
}

fn set_cookies<B>(response: &axum::http::Response<B>) -> Vec<&str> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect()
}

/// The value the response sets for the cookie `name`, if it sets one.
fn value<B>(response: &axum::http::Response<B>, name: &str) -> Option<String> {
    set_cookies(response).into_iter().find_map(|c| {
        let value = c.strip_prefix(&format!("{name}="))?.split(';').next()?;
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// Whether the response removes every form of the cookie `name`.
fn removes(response: &Response, name: &str) -> bool {
    let set = set_cookies(response);
    value(response, name).is_none()
        && set
            .iter()
            .any(|c| c.starts_with(&format!("{name}=;")) && c.ends_with("Max-Age=0"))
}

fn location_of<B>(response: &axum::http::Response<B>) -> String {
    response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned()
}

async fn json(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_oidc_login_opens_a_session_that_lives_and_closes(pool: PgPool) {
    let idp = idp().await;
    let f = Fixture::new(&pool, &idp);
    let base = idp.lock().unwrap().base.clone();

    let started = f
        .call(request("GET", "/v1/auth/login?issuer=main", None, true))
        .await;
    assert_eq!(started.status(), StatusCode::SEE_OTHER);
    let location = location_of(&started);
    assert!(
        location.starts_with(&format!("{base}/authorize?")),
        "{location}"
    );
    let login = value(&started, LOGIN).unwrap();
    assert_eq!(
        set_cookies(&started)[0],
        format!("{LOGIN}={login}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=600"),
        "REQ-006-25: __Host-, Secure, HttpOnly, SameSite=Strict, Path=/, no Domain"
    );

    let code = authorize(&idp, &location);
    let state = param(&location, "state");
    let back = f
        .call(request(
            "GET",
            &format!("/v1/auth/callback?code={code}&state={state}"),
            Some(&format!("{LOGIN}={login}")),
            true,
        ))
        .await;
    assert_eq!(back.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&back), "/");
    assert!(removes(&back, LOGIN), "the login cookie goes at its end");
    let session = value(&back, SESSION).unwrap();
    assert!(
        set_cookies(&back).contains(
            &format!("{SESSION}={session}; Path=/; Secure; HttpOnly; SameSite=Strict").as_str()
        ),
        "a cookie of the browser session"
    );
    assert_eq!(f.events("authn.login.succeeded").await, 1);

    // The group of the person reads everywhere (006-18: the rights in `GET /v1/me`).
    let issuer = Session::unseal(&f.ring, &[&session]).unwrap().issuer;
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, label, valid_period, changeset_id)
         VALUES ($1, 'viewer', 'v', tstzrange(now() - interval '1 day', NULL), gen_random_uuid());
         ",
    )
    .bind(role)
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
         VALUES ($1, 'core.entity.read', tstzrange(now() - interval '1 day', NULL),
                 gen_random_uuid())",
    )
    .bind(role)
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO binding (id, subject_kind, issuer, group_name, role_id, scope_kind,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'group', $1, 'netops', $2, 'global',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(&issuer)
    .bind(role)
    .execute(&f.pool)
    .await
    .unwrap();

    // Seen just now: not sent again.
    let cookie = format!("{SESSION}={session}");
    let me = f.call(request("GET", "/v1/me", Some(&cookie), true)).await;
    assert_eq!(me.status(), StatusCode::OK);
    assert!(set_cookies(&me).is_empty(), "{:?}", set_cookies(&me));
    let body = json(me).await;
    assert_eq!(body["method"], "oidc");
    assert_eq!(body["kind"], "person");
    assert_eq!(body["groups"], json!(["netops"]));
    assert_eq!(body["roles"], json!(["viewer"]));
    assert_eq!(
        body["bindings"],
        json!([{"role": "viewer", "scope": {"kind": "global"}}])
    );
    assert_eq!(body["global_permissions"], json!(["core.entity.read"]));
    assert_eq!(body["session"]["breakglass"], false);
    // Inactivity first: 30 minutes from now.
    let ends = time::OffsetDateTime::parse(
        body["session"]["expires_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap()
    .unix_timestamp();
    assert!((ends - unix_now() - 1800).abs() <= 2, "{ends}");

    // Seen two minutes ago: sent again, seen now.
    let older = f.forged(&session, |s| s.seen -= 120);
    let me = f.call(request("GET", "/v1/me", Some(&older), true)).await;
    assert_eq!(me.status(), StatusCode::OK);
    let again = value(&me, SESSION).expect("sent again");
    let again = Session::unseal(&f.ring, &[&again]).unwrap();
    assert!(unix_now() - again.seen <= 1);

    let out = f
        .call(request("POST", "/v1/auth/logout", Some(&cookie), true))
        .await;
    assert_eq!(out.status(), StatusCode::SEE_OTHER);
    let end = location_of(&out);
    assert!(end.starts_with(&format!("{base}/logout?")), "{end}");
    assert!(end.contains("id_token_hint="), "REQ-005-8");
    assert!(removes(&out, SESSION), "{:?}", set_cookies(&out));
    let cause: String = sqlx::query_scalar(
        "SELECT details->>'cause' FROM audit.event WHERE name = 'authn.session.closed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cause, "logout");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_callback_is_played_once(pool: PgPool) {
    let idp = idp().await;
    let f = Fixture::new(&pool, &idp);
    let started = f
        .call(request("GET", "/v1/auth/login?issuer=main", None, true))
        .await;
    let location = location_of(&started);
    let login = format!("{LOGIN}={}", value(&started, LOGIN).unwrap());
    let code = authorize(&idp, &location);
    let state = param(&location, "state");
    let callback = format!("/v1/auth/callback?code={code}&state={state}");
    let first = f.call(request("GET", &callback, Some(&login), true)).await;
    assert_eq!(first.status(), StatusCode::SEE_OTHER);

    // The same return again, with the login cookie kept, or without it.
    for cookie in [Some(login.as_str()), None] {
        let replayed = f.call(request("GET", &callback, cookie, true)).await;
        assert_eq!(replayed.status(), StatusCode::UNAUTHORIZED, "{cookie:?}");
        assert!(value(&replayed, SESSION).is_none());
        assert!(removes(&replayed, LOGIN));
    }
    // Another state than the one of the login.
    let started = f
        .call(request("GET", "/v1/auth/login?issuer=main", None, true))
        .await;
    let login = format!("{LOGIN}={}", value(&started, LOGIN).unwrap());
    let code = authorize(&idp, &location_of(&started));
    let forged = f
        .call(request(
            "GET",
            &format!("/v1/auth/callback?code={code}&state={state}"),
            Some(&login),
            true,
        ))
        .await;
    assert_eq!(forged.status(), StatusCode::UNAUTHORIZED);

    let reasons: Vec<String> = sqlx::query_scalar(
        "SELECT reason FROM audit.event WHERE name = 'authn.login.failed' ORDER BY seq",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(reasons, ["exchange", "transaction", "state"]);
    assert_eq!(f.events("authn.login.succeeded").await, 1);
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_groups_are_refreshed_when_the_token_of_the_issuer_expires(pool: PgPool) {
    let idp = idp().await;
    let f = Fixture::new(&pool, &idp);
    let session = f.login(&idp).await;
    idp.lock().unwrap().groups = vec!["admins".into()];

    let due = f.forged(&session, |s| s.refresh_at = unix_now() - 1);
    let me = f.call(request("GET", "/v1/me", Some(&due), true)).await;
    assert_eq!(me.status(), StatusCode::OK);
    let refreshed = value(&me, SESSION).expect("sent again with its groups");
    let refreshed = Session::unseal(&f.ring, &[&refreshed]).unwrap();
    assert_eq!(refreshed.groups, ["admins"]);
    assert!(refreshed.refresh_at > unix_now());
    assert_eq!(json(me).await["groups"], json!(["admins"]));

    // The issuer refuses: the session ends.
    let refused = f.forged(&session, |s| {
        s.refresh_at = unix_now() - 1;
        s.refresh_token = Some("revoked".into());
    });
    let me = f.call(request("GET", "/v1/me", Some(&refused), true)).await;
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
    assert!(removes(&me, SESSION));
    let cause: String = sqlx::query_scalar(
        "SELECT details->>'cause' FROM audit.event WHERE name = 'authn.session.closed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cause, "refresh_failed");
}

/// The secret of an `otpauth://` URI (RFC 4648 base32), as an authenticator app reads it.
fn secret_of(uri: &str) -> Vec<u8> {
    let encoded = uri
        .split("secret=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for c in encoded.bytes() {
        let value = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
            .iter()
            .position(|&a| a == c)
            .unwrap();
        buffer = (buffer << 5) | u32::try_from(value).unwrap();
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buffer >> bits) & 0xff).unwrap());
        }
    }
    out
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_breakglass_session_is_refused_after_one_hour(pool: PgPool) {
    let idp = idp().await;
    let f = Fixture::new(&pool, &idp);
    let account = create(&f.ring, "rescue");
    kartenn_store_pg::breakglass::create(
        &pool,
        &Stored {
            name: "rescue",
            password_hash: &account.password_hash,
            totp_sealed: &account.totp_sealed,
            totp_key_id: &account.totp_key_id,
            valid_until: OffsetDateTime::now_utc() + Duration::from_hours(1),
        },
        &Instance::new("test"),
    )
    .await
    .unwrap();
    let otp = format!(
        "{:06}",
        code(&secret_of(&account.otpauth), unix_now() / STEP)
    );
    let proofs = json!({ "name": "rescue", "passphrase": account.passphrase, "code": otp });
    let opened = f
        .call(with_body(
            "POST",
            "/v1/auth/breakglass",
            None,
            true,
            Body::from(proofs.to_string()),
        ))
        .await;
    assert_eq!(opened.status(), StatusCode::NO_CONTENT);
    assert_eq!(f.events("authn.breakglass.used").await, 1);
    let session = value(&opened, SESSION).unwrap();

    let me = f
        .call(request(
            "GET",
            "/v1/me",
            Some(&format!("{SESSION}={session}")),
            true,
        ))
        .await;
    assert_eq!(me.status(), StatusCode::OK);
    let body = json(me).await;
    assert_eq!(body["kind"], "breakglass");
    assert_eq!(body["method"], "breakglass");
    assert_eq!(body["session"]["breakglass"], true);

    let almost = f.forged(&session, |s| s.started -= 3_500);
    let me = f.call(request("GET", "/v1/me", Some(&almost), true)).await;
    assert_eq!(me.status(), StatusCode::OK);
    let past = f.forged(&session, |s| s.started -= 3_601);
    let me = f.call(request("GET", "/v1/me", Some(&past), true)).await;
    assert_eq!(
        me.status(),
        StatusCode::UNAUTHORIZED,
        "1 h, not the 8 h of a session"
    );
    assert!(removes(&me, SESSION));
    let cause: String = sqlx::query_scalar(
        "SELECT details->>'cause' FROM audit.event WHERE name = 'authn.session.closed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cause, "absolute");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn no_cookie_is_set_without_tls(pool: PgPool) {
    let idp = idp().await;
    let f = Fixture::new(&pool, &idp);
    for (method, uri) in [
        ("GET", "/v1/auth/login?issuer=main"),
        ("GET", "/v1/auth/callback?code=c&state=s"),
        ("POST", "/v1/auth/breakglass"),
    ] {
        let response = f.call(request(method, uri, None, false)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        assert!(set_cookies(&response).is_empty(), "{uri}");
    }
    let unknown = f
        .call(request("GET", "/v1/auth/login?issuer=other", None, true))
        .await;
    assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
    assert!(set_cookies(&unknown).is_empty());
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_issuers_of_the_login_are_public(pool: PgPool) {
    let idp = idp().await;
    let f = Fixture::of_issuers(&pool, &idp, Settings::default(), &["main", "backup"]);
    // Without session nor TLS: their identifiers alone, in the order of the
    // configuration, neither URL nor client.
    let response = f
        .call(request("GET", "/v1/auth/issuers", None, false))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json(response).await,
        serde_json::json!([{"id": "main"}, {"id": "backup"}])
    );
}

/// A login through the issuer over `client`, as a browser on the page of kartenn sends
/// it, with `forwarded` when set: the value of the session cookie.
async fn login_over(
    client: &mut wire::Client,
    idp: &Shared,
    forwarded: Option<&str>,
) -> Option<String> {
    let request = |client: &wire::Client, uri: &str, cookie: Option<&str>| {
        let mut request = client
            .request("GET", uri)
            .header("sec-fetch-site", "same-origin");
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        if let Some(forwarded) = forwarded {
            request = request.header(header::FORWARDED, forwarded);
        }
        request.body(wire::empty()).unwrap()
    };
    let started = client
        .send(request(client, "/v1/auth/login?issuer=main", None))
        .await;
    if started.status() != StatusCode::SEE_OTHER {
        assert_eq!(started.status(), StatusCode::BAD_REQUEST);
        assert!(set_cookies(&started).is_empty(), "no cookie without TLS");
        return None;
    }
    let location = location_of(&started);
    let login = value(&started, LOGIN).unwrap();
    let code = authorize(idp, &location);
    let state = param(&location, "state");
    let back = client
        .send(request(
            client,
            &format!("/v1/auth/callback?code={code}&state={state}"),
            Some(&format!("{LOGIN}={login}")),
        ))
        .await;
    assert_eq!(back.status(), StatusCode::SEE_OTHER);
    value(&back, SESSION)
}

/// `GET /v1/me` over `client` with the session cookie `session`.
async fn me_over(
    client: &mut wire::Client,
    session: &str,
    forwarded: Option<&str>,
) -> axum::http::Response<axum::body::Bytes> {
    let mut request = client
        .request("GET", "/v1/me")
        .header(header::COOKIE, format!("{SESSION}={session}"));
    if let Some(forwarded) = forwarded {
        request = request.header(header::FORWARDED, forwarded);
    }
    client.send(request.body(wire::empty()).unwrap()).await
}

/// REQ-006-25, 32: the cookies of a login cross a real TLS connection in HTTP/1.1 and
/// HTTP/2, and the session they open is recognised on a new connection.
#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn a_login_crosses_a_local_port_over_tls(pool: PgPool) {
    let idp = idp().await;
    let f = Fixture::new(&pool, &idp);
    let server = wire::start(f.app.clone(), true).await;
    for version in wire::VERSIONS {
        let mut client = server.connect(version).await;
        let session = login_over(&mut client, &idp, None).await.unwrap();
        let mut again = server.connect(version).await;
        let me = me_over(&mut again, &session, None).await;
        assert_eq!(me.status(), StatusCode::OK, "{version:?}");
        let body: Value = serde_json::from_slice(me.body()).unwrap();
        assert_eq!(body["method"], "oidc", "{version:?}");
    }
    assert_eq!(f.events("authn.login.succeeded").await, 2);
}

/// REQ-006-27: behind a trusted proxy on clear HTTP, the cookies follow its
/// `Forwarded: proto`; from an address not configured, `proto=https` is not believed; a
/// session cookie or a password arriving in clear → 401.
#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_cookies_follow_forwarded_on_a_local_port(pool: PgPool) {
    const HTTPS: Option<&str> = Some("for=198.51.100.1;proto=https");
    const HTTP: Option<&str> = Some("for=198.51.100.1;proto=http");
    let idp = idp().await;
    let proxied = Fixture::with(
        &pool,
        &idp,
        Settings {
            trusted_proxies: vec!["127.0.0.0/8".parse().unwrap()],
            ..Settings::default()
        },
    );
    let direct = Fixture::new(&pool, &idp);
    let behind = wire::start(proxied.app.clone(), false).await;
    let open = wire::start(direct.app.clone(), false).await;
    for version in wire::VERSIONS {
        let mut client = behind.connect(version).await;
        let session = login_over(&mut client, &idp, HTTPS).await.unwrap();
        assert_eq!(
            me_over(&mut client, &session, HTTPS).await.status(),
            StatusCode::OK,
            "{version:?}"
        );
        assert_eq!(
            me_over(&mut client, &session, HTTP).await.status(),
            StatusCode::UNAUTHORIZED,
            "{version:?}: a session cookie in clear"
        );
        assert_eq!(login_over(&mut client, &idp, HTTP).await, None);
        let basic = client
            .send(
                client
                    .request("GET", "/v1/me")
                    .header(header::AUTHORIZATION, "Basic YTpi")
                    .header(header::FORWARDED, HTTP.unwrap())
                    .body(wire::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(basic.status(), StatusCode::UNAUTHORIZED, "{version:?}");

        let mut client = open.connect(version).await;
        assert_eq!(
            login_over(&mut client, &idp, HTTPS).await,
            None,
            "{version:?}: an address not configured"
        );
    }
}
