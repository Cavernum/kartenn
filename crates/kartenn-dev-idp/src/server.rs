//! The issuer: discovery (OpenID Connect Discovery 1.0 §4), JWKS, `authorize` with a page
//! to choose the user (authorization code with PKCE S256, RFC 7636; `iss` in the
//! response, RFC 9207), `token` (code and refresh token, `client_secret_basic`, RFC 6749
//! §2.3.1) and `end-session` (OpenID Connect RP-Initiated Logout 1.0). Served over TLS on
//! the loopback; keys drawn at the start, codes and refresh tokens in memory.
//!
//! With `[native]` (spec 023, T10), a public client of the native applications too: no
//! secret (`client_id` in the form, RFC 6749 §2.3), a loopback redirection on any port
//! (RFC 8252 §7.3), the device flow (`device_authorization_endpoint`, RFC 8628 §3.1 to
//! 3.5, `slow_down` on demand) and access tokens `at+jwt` for the audience of kartenn
//! (RFC 9068). Refresh tokens rotate at each use for every client (RFC 9700 §2.2.2).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use hyper_util::service::TowerToHyperService;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use p256::pkcs8::EncodePrivateKey as _;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::crypto::ring;
use tokio_rustls::rustls::pki_types::pem::PemObject as _;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::certs::random;
use crate::config::{Config, Native, User};

/// Lifetime of a code (RFC 6749 §4.1.2: short, used once).
const CODE_WITHIN: Duration = Duration::from_secs(60);
/// Lifetime of an ID token and of an access token, in seconds.
const TOKEN_SECONDS: i64 = 300;
/// Longest TLS handshake.
const HANDSHAKE_WITHIN: Duration = Duration::from_secs(10);
/// The grant type of the device flow (RFC 8628 §3.4).
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// What `slow_down` adds to the interval (RFC 8628 §3.5).
const SLOW_DOWN: Duration = Duration::from_secs(5);
/// The characters of a user code: consonants, no vowel to spell nothing (RFC 8628 §6.1).
const USER_CODE_CHARS: &[u8; 20] = b"BCDFGHJKLMNPQRSTVWXZ";

/// The client a request comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Client {
    /// The confidential client of kartenn (the BFF).
    Confidential,
    /// The public client of the native applications.
    Native,
}

/// A code waiting for its exchange.
#[derive(Debug)]
struct Grant {
    client: Client,
    sub: String,
    nonce: Option<String>,
    scope: String,
    redirect_uri: String,
    challenge: String,
    issued: Instant,
}

/// What a refresh token stands for: used once, then replaced.
#[derive(Debug, Clone)]
struct Refresh {
    client: Client,
    sub: String,
    scope: String,
}

/// Where a device code stands (RFC 8628 §3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Decision {
    Pending,
    Approved(String),
    Denied,
}

/// A device code waiting for its approval.
#[derive(Debug)]
struct Device {
    user_code: String,
    scope: String,
    decision: Decision,
    expires: Instant,
    interval: Duration,
    last_poll: Option<Instant>,
    slow_down: u32,
}

/// The state of the issuer.
struct Idp {
    config: Config,
    secret: String,
    signing: EncodingKey,
    jwk: Jwk,
    codes: Mutex<HashMap<String, Grant>>,
    refresh: Mutex<HashMap<String, Refresh>>,
    /// The device codes, by device code.
    devices: Mutex<HashMap<String, Device>>,
}

type Shared = Arc<Idp>;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Idp {
    /// The issuer of `config` and `secret`, its signing key drawn now.
    fn new(config: Config, secret: String) -> anyhow::Result<Self> {
        let key = p256::SecretKey::random(&mut rand_core::OsRng);
        let pem = key.to_pkcs8_pem(p256::pkcs8::LineEnding::default())?;
        let signing = EncodingKey::from_ec_pem(pem.as_bytes())?;
        let mut jwk = Jwk::from_encoding_key(&signing, Algorithm::ES256)?;
        jwk.common.key_id = Some(URL_SAFE_NO_PAD.encode(random::<8>()));
        Ok(Self {
            config,
            secret,
            signing,
            jwk,
            codes: Mutex::new(HashMap::new()),
            refresh: Mutex::new(HashMap::new()),
            devices: Mutex::new(HashMap::new()),
        })
    }

    /// Its public client, if any.
    fn native(&self) -> Option<&Native> {
        self.config.native.as_ref()
    }

    /// Its public client if it may use the device flow.
    fn device(&self) -> Option<&Native> {
        self.native().filter(|n| n.device)
    }

    /// The identifier of `client`.
    fn client_id(&self, client: Client) -> &str {
        match (client, self.native()) {
            (Client::Native, Some(native)) => &native.client_id,
            _ => &self.config.client_id,
        }
    }

    /// The client named `id`, if any.
    fn client(&self, id: Option<&str>) -> Option<Client> {
        let id = id?;
        if id == self.config.client_id {
            Some(Client::Confidential)
        } else if self.native().is_some_and(|n| n.client_id == id) {
            Some(Client::Native)
        } else {
            None
        }
    }
}

/// The issuer, ready to serve.
#[allow(missing_debug_implementations)]
pub struct Server {
    idp: Shared,
    acceptor: TlsAcceptor,
}

impl Server {
    /// Checks `config`, reads the client secret and the certificate, draws the keys.
    ///
    /// # Errors
    ///
    /// A refused configuration, an unreadable file.
    pub fn new(config: Config) -> anyhow::Result<Self> {
        config.check()?;
        let secret = std::fs::read_to_string(&config.client_secret_file)
            .with_context(|| format!("reading {}", config.client_secret_file.display()))?
            .trim()
            .to_owned();
        anyhow::ensure!(!secret.is_empty(), "the client secret is empty");
        let acceptor = acceptor(&config)?;
        Ok(Self {
            idp: Arc::new(Idp::new(config, secret)?),
            acceptor,
        })
    }

    /// Binds the address of the configuration and serves until `shutdown`.
    ///
    /// # Errors
    ///
    /// The address cannot be bound.
    pub async fn run(self, shutdown: impl Future<Output = ()>) -> anyhow::Result<()> {
        let listen = self.idp.config.listen;
        let listener = TcpListener::bind(listen)
            .await
            .with_context(|| format!("binding {listen}"))?;
        self.serve(listener, shutdown).await;
        Ok(())
    }

    /// Serves on `listener`, already bound, until `shutdown`.
    pub async fn serve(self, listener: TcpListener, shutdown: impl Future<Output = ()>) {
        let app = router(self.idp);
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((stream, _)) = accepted else { continue };
                    let acceptor = self.acceptor.clone();
                    let service = TowerToHyperService::new(app.clone());
                    tokio::spawn(async move {
                        let Ok(Ok(tls)) =
                            tokio::time::timeout(HANDSHAKE_WITHIN, acceptor.accept(stream)).await
                        else {
                            return;
                        };
                        let _ = auto::Builder::new(TokioExecutor::new())
                            .serve_connection(TokioIo::new(tls), service)
                            .await;
                    });
                }
                () = &mut shutdown => break,
            }
        }
    }
}

fn acceptor(config: &Config) -> anyhow::Result<TlsAcceptor> {
    let chain = CertificateDer::pem_file_iter(&config.cert_file)
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .with_context(|| format!("reading {}", config.cert_file.display()))?;
    let key = PrivateKeyDer::from_pem_file(&config.key_file)
        .with_context(|| format!("reading {}", config.key_file.display()))?;
    let mut tls = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .context("the key and the certificate")?;
    tls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(TlsAcceptor::from(Arc::new(tls)))
}

fn router(idp: Shared) -> Router {
    Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/authorize", get(authorize_page).post(authorize))
        .route("/token", axum::routing::post(token))
        .route(
            "/device_authorization",
            axum::routing::post(device_authorization),
        )
        .route("/device", get(device_page).post(device_decision))
        .route("/end-session", get(end_session))
        .with_state(idp)
}

async fn discovery(State(idp): State<Shared>) -> Json<Value> {
    let base = &idp.config.issuer;
    let mut grants = vec!["authorization_code", "refresh_token"];
    let mut methods = vec!["client_secret_basic"];
    if idp.native().is_some() {
        methods.push("none");
    }
    let mut document = json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "jwks_uri": format!("{base}/jwks"),
        "end_session_endpoint": format!("{base}/end-session"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["ES256"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": ["openid", "profile", "email", "offline_access"],
        "claims_supported": ["sub", "name", "email", "groups"],
        "authorization_response_iss_parameter_supported": true,
    });
    if idp.device().is_some() {
        grants.push(DEVICE_GRANT);
        document["device_authorization_endpoint"] = format!("{base}/device_authorization").into();
    }
    document["grant_types_supported"] = json!(grants);
    document["token_endpoint_auth_methods_supported"] = json!(methods);
    Json(document)
}

async fn jwks(State(idp): State<Shared>) -> Json<JwkSet> {
    Json(JwkSet {
        keys: vec![idp.jwk.clone()],
    })
}

/// An authorization request, checked.
struct Authorization {
    client: Client,
    redirect_uri: String,
    state: Option<String>,
    nonce: Option<String>,
    scope: String,
    challenge: String,
}

/// Whether the audience a native client asks for (`resource`, RFC 8707, or `audience`)
/// is that of its tokens, when it names one.
fn audience_ok(native: &Native, params: &HashMap<String, String>) -> bool {
    ["resource", "audience"]
        .iter()
        .filter_map(|name| params.get(*name))
        .all(|asked| *asked == native.audience)
}

/// Checks a request: an unknown client or redirection gets a page (RFC 6749 §4.1.2.1,
/// never a redirection), any other fault goes back to the client.
#[allow(clippy::result_large_err)] // a response, built once per request
fn check(idp: &Idp, params: &HashMap<String, String>) -> Result<Authorization, Response> {
    let get = |name: &str| params.get(name).cloned();
    let Some(client) = idp.client(params.get("client_id").map(String::as_str)) else {
        return Err(page(StatusCode::BAD_REQUEST, "Unknown client", ""));
    };
    let registered = |r: &String| match (client, idp.native()) {
        (Client::Native, Some(native)) => native.redirects_to(r),
        _ => idp.config.redirect_uris.contains(r),
    };
    let Some(redirect_uri) = get("redirect_uri").filter(registered) else {
        return Err(page(
            StatusCode::BAD_REQUEST,
            "Unregistered redirect_uri",
            "",
        ));
    };
    let state = get("state");
    let back = |error: &str| {
        Err(back(
            idp,
            &redirect_uri,
            &[("error", error)],
            state.as_deref(),
        ))
    };
    if get("response_type").as_deref() != Some("code") {
        return back("unsupported_response_type");
    }
    let Some(scope) = get("scope").filter(|s| s.split(' ').any(|s| s == "openid")) else {
        return back("invalid_scope");
    };
    // PKCE S256 for every client, the public one first (RFC 8252 §6, RFC 9700 §2.1.1).
    let challenge = match (get("code_challenge"), get("code_challenge_method")) {
        (Some(c), Some(m)) if m == "S256" && !c.is_empty() => c,
        _ => return back("invalid_request"),
    };
    if let (Client::Native, Some(native)) = (client, idp.native())
        && !audience_ok(native, params)
    {
        return back("invalid_target");
    }
    Ok(Authorization {
        client,
        redirect_uri,
        state,
        nonce: get("nonce"),
        scope,
        challenge,
    })
}

/// The redirection to the client, with `iss` (RFC 9207) and `state`.
fn back(idp: &Idp, redirect_uri: &str, params: &[(&str, &str)], state: Option<&str>) -> Response {
    let mut query = form_urlencoded::Serializer::new(String::new());
    query.extend_pairs(params);
    if let Some(state) = state {
        query.append_pair("state", state);
    }
    query.append_pair("iss", &idp.config.issuer);
    let separator = if redirect_uri.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{redirect_uri}{separator}{}", query.finish())).into_response()
}

/// One button per user, in a form posting `params` to `action`.
fn chooser(idp: &Idp, action: &str, params: &HashMap<String, String>) -> String {
    let mut hidden = String::new();
    let mut names: Vec<_> = params.iter().filter(|(k, _)| *k != "user").collect();
    names.sort();
    for (name, value) in names {
        let _ = write!(
            hidden,
            r#"<input type="hidden" name="{}" value="{}">"#,
            escape(name),
            escape(value)
        );
    }
    let mut buttons = String::new();
    for user in &idp.config.users {
        let groups = if user.groups.is_empty() {
            String::new()
        } else {
            format!(" ({})", user.groups.join(", "))
        };
        let error = user
            .error
            .as_ref()
            .map(|e| format!(" → {e}"))
            .unwrap_or_default();
        let _ = write!(
            buttons,
            r#"<li><button type="submit" name="user" value="{}">{}</button> <code>{}</code>{}{}</li>"#,
            escape(&user.sub),
            escape(&user.name),
            escape(&user.sub),
            escape(&groups),
            escape(&error),
        );
    }
    format!(r#"<form method="post" action="{action}">{hidden}<ul>{buttons}</ul></form>"#)
}

/// The page to choose the user: one button per user, the request carried along.
async fn authorize_page(
    State(idp): State<Shared>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    if let Err(refused) = check(&idp, &params) {
        return refused;
    }
    page(
        StatusCode::OK,
        "Sign in as",
        &chooser(&idp, "authorize", &params),
    )
}

/// The user chosen in `params`, if known.
fn chosen<'a>(idp: &'a Idp, params: &HashMap<String, String>) -> Option<&'a User> {
    params
        .get("user")
        .and_then(|sub| idp.config.users.iter().find(|u| &u.sub == sub))
}

/// The choice of a user: a code, or the error of that user.
async fn authorize(
    State(idp): State<Shared>,
    Form(params): Form<HashMap<String, String>>,
) -> Response {
    let request = match check(&idp, &params) {
        Ok(request) => request,
        Err(refused) => return refused,
    };
    let Some(user) = chosen(&idp, &params) else {
        return page(StatusCode::BAD_REQUEST, "Unknown user", "");
    };
    let state = request.state.as_deref();
    if let Some(error) = &user.error {
        return back(&idp, &request.redirect_uri, &[("error", error)], state);
    }
    let code = URL_SAFE_NO_PAD.encode(random::<32>());
    {
        let mut codes = locked(&idp.codes);
        codes.retain(|_, g| g.issued.elapsed() < CODE_WITHIN);
        codes.insert(
            code.clone(),
            Grant {
                client: request.client,
                sub: user.sub.clone(),
                nonce: request.nonce.clone(),
                scope: request.scope.clone(),
                redirect_uri: request.redirect_uri.clone(),
                challenge: request.challenge.clone(),
                issued: Instant::now(),
            },
        );
    }
    back(&idp, &request.redirect_uri, &[("code", &code)], state)
}

/// An error of the token endpoint (RFC 6749 §5.2).
fn token_error(status: StatusCode, error: &str) -> Response {
    let mut response = (status, Json(json!({ "error": error }))).into_response();
    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static(r#"Basic realm="kartenn-dev-idp""#),
        );
    }
    response
}

/// The client of a request to the token endpoint: the confidential one when the
/// `Authorization` header carries it and its secret, each form-encoded (RFC 6749 §2.3.1);
/// the public one when the form names it and carries no secret (§2.3, §3.2.1).
fn client_authenticated(
    idp: &Idp,
    headers: &HeaderMap,
    form: &HashMap<String, String>,
) -> Option<Client> {
    let Some(authorization) = headers.get(header::AUTHORIZATION) else {
        let public = idp.client(form.get("client_id").map(String::as_str));
        return public.filter(|c| *c == Client::Native && !form.contains_key("client_secret"));
    };
    let decoded = authorization
        .to_str()
        .ok()
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(|v| STANDARD.decode(v.trim()).ok())
        .and_then(|v| String::from_utf8(v).ok());
    let (id, secret) = decoded.as_deref().and_then(|d| d.split_once(':'))?;
    let decode = |s: &str| -> String {
        form_urlencoded::parse(format!("x={s}").as_bytes())
            .next()
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default()
    };
    (decode(id) == idp.config.client_id && decode(secret) == idp.secret)
        .then_some(Client::Confidential)
}

/// What a grant gives once checked: the client, the user, the nonce of the login, the
/// scope.
type Granted = (Client, String, Option<String>, String);

/// The grant of a code (RFC 6749 §4.1.3), its verifier checked (RFC 7636 §4.6).
#[allow(clippy::result_large_err)] // a response, built once per request
fn code_grant(
    idp: &Idp,
    client: Client,
    form: &HashMap<String, String>,
) -> Result<Granted, Response> {
    let get = |name: &str| form.get(name).map(String::as_str);
    let invalid = || token_error(StatusCode::BAD_REQUEST, "invalid_grant");
    let grant = get("code").and_then(|c| locked(&idp.codes).remove(c));
    let Some(grant) = grant.filter(|g| g.issued.elapsed() < CODE_WITHIN && g.client == client)
    else {
        return Err(invalid());
    };
    let verified = get("code_verifier")
        .is_some_and(|v| URL_SAFE_NO_PAD.encode(Sha256::digest(v.as_bytes())) == grant.challenge);
    if !verified || get("redirect_uri") != Some(grant.redirect_uri.as_str()) {
        return Err(invalid());
    }
    Ok((client, grant.sub, grant.nonce, grant.scope))
}

/// The grant of a refresh token, rotated at each use: the token used is forgotten, a
/// second use is refused (RFC 9700 §2.2.2, §4.14.2).
#[allow(clippy::result_large_err)] // a response, built once per request
fn refresh_grant(
    idp: &Idp,
    client: Client,
    form: &HashMap<String, String>,
) -> Result<Granted, Response> {
    let refresh = form
        .get("refresh_token")
        .and_then(|r| locked(&idp.refresh).remove(r));
    match refresh {
        Some(r) if r.client == client => Ok((client, r.sub, None, r.scope)),
        _ => Err(token_error(StatusCode::BAD_REQUEST, "invalid_grant")),
    }
}

/// A poll of the device flow (RFC 8628 §3.4, §3.5): the tokens once approved, else
/// `authorization_pending`, `slow_down` (a poll before its interval, or one of the first
/// `slow_down` of the configuration), `access_denied` or `expired_token`.
#[allow(clippy::result_large_err)] // a response, built once per request
fn device_grant(
    idp: &Idp,
    client: Client,
    form: &HashMap<String, String>,
) -> Result<Granted, Response> {
    let error = |error: &str| Err(token_error(StatusCode::BAD_REQUEST, error));
    if client != Client::Native || idp.device().is_none() {
        return error("unauthorized_client");
    }
    let Some(code) = form.get("device_code") else {
        return error("invalid_request");
    };
    let mut devices = locked(&idp.devices);
    let Some(device) = devices.get_mut(code) else {
        return error("invalid_grant");
    };
    let now = Instant::now();
    if now >= device.expires {
        devices.remove(code);
        return error("expired_token");
    }
    match device.decision.clone() {
        Decision::Approved(sub) => {
            let scope = device.scope.clone();
            devices.remove(code);
            Ok((client, sub, None, scope))
        }
        Decision::Denied => {
            devices.remove(code);
            error("access_denied")
        }
        Decision::Pending => {
            let early = device
                .last_poll
                .is_some_and(|last| now.duration_since(last) < device.interval);
            device.last_poll = Some(now);
            if device.slow_down > 0 || early {
                device.slow_down = device.slow_down.saturating_sub(1);
                device.interval += SLOW_DOWN;
                return error("slow_down");
            }
            error("authorization_pending")
        }
    }
}

async fn token(
    State(idp): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let Some(client) = client_authenticated(&idp, &headers, &form) else {
        return token_error(StatusCode::UNAUTHORIZED, "invalid_client");
    };
    let granted = match form.get("grant_type").map(String::as_str) {
        Some("authorization_code") => code_grant(&idp, client, &form),
        Some("refresh_token") => refresh_grant(&idp, client, &form),
        Some(DEVICE_GRANT) => device_grant(&idp, client, &form),
        _ => Err(token_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
        )),
    };
    let (client, sub, nonce, scope) = match granted {
        Ok(granted) => granted,
        Err(refused) => return refused,
    };
    let Some(user) = idp.config.users.iter().find(|u| u.sub == sub) else {
        return token_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let access = match (client, idp.native()) {
        (Client::Native, Some(native)) => access_token(&idp, native, user, &scope),
        _ => Ok(URL_SAFE_NO_PAD.encode(random::<32>())),
    };
    let (Ok(access), Ok(id_token)) = (access, id_token(&idp, client, user, nonce.as_deref()))
    else {
        return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error");
    };
    let refresh = URL_SAFE_NO_PAD.encode(random::<32>());
    locked(&idp.refresh).insert(
        refresh.clone(),
        Refresh {
            client,
            sub,
            scope: scope.clone(),
        },
    );
    let mut response = Json(json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": TOKEN_SECONDS,
        "id_token": id_token,
        "refresh_token": refresh,
        "scope": scope,
    }))
    .into_response();
    // RFC 6749 §5.1.
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Seconds since the epoch.
fn now() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
    .unwrap_or(i64::MAX)
}

/// `claims` signed by the key of the issuer, under `typ` if given.
fn signed(idp: &Idp, typ: Option<&str>, claims: &Value) -> jsonwebtoken::errors::Result<String> {
    let mut header = Header::new(Algorithm::ES256);
    header.kid.clone_from(&idp.jwk.common.key_id);
    if let Some(typ) = typ {
        header.typ = Some(typ.to_owned());
    }
    encode(&header, claims, &idp.signing)
}

fn id_token(
    idp: &Idp,
    client: Client,
    user: &User,
    nonce: Option<&str>,
) -> jsonwebtoken::errors::Result<String> {
    let now = now();
    let mut claims = json!({
        "iss": idp.config.issuer,
        "aud": idp.client_id(client),
        "sub": user.sub,
        "name": user.name,
        "groups": user.groups,
        "iat": now,
        "auth_time": now,
        "exp": now + TOKEN_SECONDS,
    });
    if let Some(email) = &user.email {
        claims["email"] = email.as_str().into();
    }
    if let Some(nonce) = nonce {
        claims["nonce"] = nonce.into();
    }
    signed(idp, None, &claims)
}

/// An access token of the public client: a JWT `at+jwt` for the audience of kartenn
/// (RFC 9068 §2.1, §2.2), with the groups of the user.
fn access_token(
    idp: &Idp,
    native: &Native,
    user: &User,
    scope: &str,
) -> jsonwebtoken::errors::Result<String> {
    let now = now();
    let claims = json!({
        "iss": idp.config.issuer,
        "aud": native.audience,
        "sub": user.sub,
        "client_id": native.client_id,
        "scope": scope,
        "groups": user.groups,
        "name": user.name,
        "iat": now,
        "exp": now + TOKEN_SECONDS,
        "jti": URL_SAFE_NO_PAD.encode(random::<16>()),
    });
    signed(idp, Some("at+jwt"), &claims)
}

/// A user code: eight consonants, shown `XXXX-XXXX` (RFC 8628 §6.1).
fn user_code() -> String {
    let letters: String = random::<8>()
        .iter()
        .map(|b| char::from(USER_CODE_CHARS[usize::from(*b) % USER_CODE_CHARS.len()]))
        .collect();
    format!("{}-{}", &letters[..4], &letters[4..])
}

/// A user code as typed: upper case, without dash nor space (RFC 8628 §6.1).
fn normalized(code: &str) -> String {
    code.chars()
        .filter(|c| !matches!(c, '-' | ' '))
        .flat_map(char::to_uppercase)
        .collect()
}

/// The device authorization request of the public client (RFC 8628 §3.1, §3.2).
async fn device_authorization(
    State(idp): State<Shared>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let Some(native) = idp.device() else {
        return token_error(StatusCode::BAD_REQUEST, "unauthorized_client");
    };
    if form.get("client_id") != Some(&native.client_id) || form.contains_key("client_secret") {
        return token_error(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    let Some(scope) = form
        .get("scope")
        .filter(|s| s.split(' ').any(|s| s == "openid"))
    else {
        return token_error(StatusCode::BAD_REQUEST, "invalid_scope");
    };
    if !audience_ok(native, &form) {
        return token_error(StatusCode::BAD_REQUEST, "invalid_target");
    }
    let device_code = URL_SAFE_NO_PAD.encode(random::<32>());
    let code = user_code();
    let now = Instant::now();
    {
        let mut devices = locked(&idp.devices);
        devices.retain(|_, d| d.expires > now);
        devices.insert(
            device_code.clone(),
            Device {
                user_code: normalized(&code),
                scope: scope.clone(),
                decision: Decision::Pending,
                expires: now + Duration::from_secs(native.device_expires_in),
                interval: Duration::from_secs(native.device_interval),
                last_poll: None,
                slow_down: native.slow_down,
            },
        );
    }
    let base = &idp.config.issuer;
    let complete = form_urlencoded::Serializer::new(format!("{base}/device?"))
        .append_pair("user_code", &code)
        .finish();
    let mut response = Json(json!({
        "device_code": device_code,
        "user_code": code,
        "verification_uri": format!("{base}/device"),
        "verification_uri_complete": complete,
        "expires_in": native.device_expires_in,
        "interval": native.device_interval,
    }))
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Whether a device code waits for `code`, typed by the user.
fn waits(idp: &Idp, code: &str) -> bool {
    let code = normalized(code);
    let now = Instant::now();
    locked(&idp.devices)
        .values()
        .any(|d| d.user_code == code && d.decision == Decision::Pending && d.expires > now)
}

/// The verification page (RFC 8628 §3.3): the user code to type, then the user to choose.
async fn device_page(
    State(idp): State<Shared>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    if idp.device().is_none() {
        return page(StatusCode::NOT_FOUND, "No device flow", "");
    }
    match params.get("user_code") {
        Some(code) if waits(&idp, code) => {
            let carried = HashMap::from([("user_code".to_owned(), code.clone())]);
            page(
                StatusCode::OK,
                "Approve the device as",
                &chooser(&idp, "device", &carried),
            )
        }
        found => page(
            if found.is_some() {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::OK
            },
            if found.is_some() {
                "Unknown or expired code"
            } else {
                "Enter the code"
            },
            r#"<form method="get" action="device"><input name="user_code" autocomplete="off"> <button type="submit">Next</button></form>"#,
        ),
    }
}

/// The choice of a user for a device code: approved, or denied when the user answers an
/// error.
async fn device_decision(
    State(idp): State<Shared>,
    Form(params): Form<HashMap<String, String>>,
) -> Response {
    let Some(user) = chosen(&idp, &params) else {
        return page(StatusCode::BAD_REQUEST, "Unknown user", "");
    };
    let code = normalized(params.get("user_code").map_or("", String::as_str));
    let now = Instant::now();
    let mut devices = locked(&idp.devices);
    let Some(device) = devices
        .values_mut()
        .find(|d| d.user_code == code && d.decision == Decision::Pending && d.expires > now)
    else {
        return page(StatusCode::BAD_REQUEST, "Unknown or expired code", "");
    };
    if user.error.is_some() {
        device.decision = Decision::Denied;
        return page(StatusCode::OK, "Device denied", "");
    }
    device.decision = Decision::Approved(user.sub.clone());
    page(
        StatusCode::OK,
        "Device approved",
        "<p>You may go back to the terminal.</p>",
    )
}

/// The origin of `url`: scheme, host and port.
fn origin(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once("://")?;
    let end = rest.find('/').unwrap_or(rest.len());
    Some(&url[..scheme.len() + 3 + end])
}

/// The logout: back to `post_logout_redirect_uri` when it shares the origin of a
/// registered redirection, a page otherwise.
async fn end_session(
    State(idp): State<Shared>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let target = params.get("post_logout_redirect_uri").filter(|after| {
        origin(after).is_some_and(|o| {
            idp.config
                .redirect_uris
                .iter()
                .any(|r| origin(r) == Some(o))
        })
    });
    match target {
        Some(after) => {
            let mut url = after.clone();
            if let Some(state) = params.get("state") {
                let separator = if url.contains('?') { '&' } else { '?' };
                url.push(separator);
                url.push_str(
                    &form_urlencoded::Serializer::new(String::new())
                        .append_pair("state", state)
                        .finish(),
                );
            }
            Redirect::to(&url).into_response()
        }
        None => page(StatusCode::OK, "Signed out", ""),
    }
}

fn page(status: StatusCode, title: &str, body: &str) -> Response {
    let mut response = (
        status,
        Html(format!(
            r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>{0} · kartenn-dev-idp</title><style>body{{font:1rem system-ui,sans-serif;margin:2rem}}li{{margin:.5rem 0}}button{{font:inherit;min-width:12rem}}</style></head><body><h1>{0}</h1><p>kartenn development issuer — never in production.</p>{1}</body></html>"#,
            escape(title),
            body
        )),
    )
        .into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'",
        ),
    );
    response
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests;
