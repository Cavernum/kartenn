//! The issuer: discovery (OpenID Connect Discovery 1.0 §4), JWKS, `authorize` with a page
//! to choose the user (authorization code with PKCE S256, RFC 7636; `iss` in the
//! response, RFC 9207), `token` (code and refresh token, `client_secret_basic`, RFC 6749
//! §2.3.1) and `end-session` (OpenID Connect RP-Initiated Logout 1.0). Served over TLS on
//! the loopback; keys drawn at the start, codes and refresh tokens in memory.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::future::Future;
use std::sync::{Arc, Mutex};
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
use crate::config::{Config, User};

/// Lifetime of a code (RFC 6749 §4.1.2: short, used once).
const CODE_WITHIN: Duration = Duration::from_secs(60);
/// Lifetime of an ID token, in seconds.
const TOKEN_SECONDS: i64 = 300;
/// Longest TLS handshake.
const HANDSHAKE_WITHIN: Duration = Duration::from_secs(10);

/// A code waiting for its exchange.
#[derive(Debug)]
struct Grant {
    sub: String,
    nonce: Option<String>,
    redirect_uri: String,
    challenge: String,
    issued: Instant,
}

/// The state of the issuer.
struct Idp {
    config: Config,
    secret: String,
    signing: EncodingKey,
    jwk: Jwk,
    codes: Mutex<HashMap<String, Grant>>,
    /// The subject of each refresh token.
    refresh: Mutex<HashMap<String, String>>,
}

type Shared = Arc<Idp>;

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
        let key = p256::SecretKey::random(&mut rand_core::OsRng);
        let pem = key.to_pkcs8_pem(p256::pkcs8::LineEnding::default())?;
        let signing = EncodingKey::from_ec_pem(pem.as_bytes())?;
        let mut jwk = Jwk::from_encoding_key(&signing, Algorithm::ES256)?;
        jwk.common.key_id = Some(URL_SAFE_NO_PAD.encode(random::<8>()));
        Ok(Self {
            idp: Arc::new(Idp {
                config,
                secret,
                signing,
                jwk,
                codes: Mutex::new(HashMap::new()),
                refresh: Mutex::new(HashMap::new()),
            }),
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
        .route("/end-session", get(end_session))
        .with_state(idp)
}

async fn discovery(State(idp): State<Shared>) -> Json<Value> {
    let base = &idp.config.issuer;
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "jwks_uri": format!("{base}/jwks"),
        "end_session_endpoint": format!("{base}/end-session"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["ES256"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic"],
        "scopes_supported": ["openid", "profile", "email"],
        "claims_supported": ["sub", "name", "email", "groups"],
        "authorization_response_iss_parameter_supported": true,
    }))
}

async fn jwks(State(idp): State<Shared>) -> Json<JwkSet> {
    Json(JwkSet {
        keys: vec![idp.jwk.clone()],
    })
}

/// An authorization request, checked.
struct Authorization {
    redirect_uri: String,
    state: Option<String>,
    nonce: Option<String>,
    challenge: String,
}

/// Checks a request: an unknown client or redirection gets a page (RFC 6749 §4.1.2.1,
/// never a redirection), any other fault goes back to the client.
#[allow(clippy::result_large_err)] // a response, built once per request
fn check(idp: &Idp, params: &HashMap<String, String>) -> Result<Authorization, Response> {
    let get = |name: &str| params.get(name).cloned();
    if get("client_id").as_deref() != Some(idp.config.client_id.as_str()) {
        return Err(page(StatusCode::BAD_REQUEST, "Unknown client", ""));
    }
    let Some(redirect_uri) = get("redirect_uri").filter(|r| idp.config.redirect_uris.contains(r))
    else {
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
    if !get("scope").is_some_and(|s| s.split(' ').any(|s| s == "openid")) {
        return back("invalid_scope");
    }
    let challenge = match (get("code_challenge"), get("code_challenge_method")) {
        (Some(c), Some(m)) if m == "S256" && !c.is_empty() => c,
        _ => return back("invalid_request"),
    };
    Ok(Authorization {
        redirect_uri,
        state,
        nonce: get("nonce"),
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

/// The page to choose the user: one button per user, the request carried along.
async fn authorize_page(
    State(idp): State<Shared>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    if let Err(refused) = check(&idp, &params) {
        return refused;
    }
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
    page(
        StatusCode::OK,
        "Sign in as",
        &format!(r#"<form method="post" action="authorize">{hidden}<ul>{buttons}</ul></form>"#),
    )
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
    let Some(user) = params
        .get("user")
        .and_then(|sub| idp.config.users.iter().find(|u| &u.sub == sub))
    else {
        return page(StatusCode::BAD_REQUEST, "Unknown user", "");
    };
    let state = request.state.as_deref();
    if let Some(error) = &user.error {
        return back(&idp, &request.redirect_uri, &[("error", error)], state);
    }
    let code = URL_SAFE_NO_PAD.encode(random::<32>());
    {
        let mut codes = idp
            .codes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        codes.retain(|_, g| g.issued.elapsed() < CODE_WITHIN);
        codes.insert(
            code.clone(),
            Grant {
                sub: user.sub.clone(),
                nonce: request.nonce.clone(),
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

/// Whether the `Authorization` header carries the client and its secret, each
/// form-encoded (RFC 6749 §2.3.1).
fn client_authenticated(idp: &Idp, headers: &HeaderMap) -> bool {
    let decoded = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(|v| STANDARD.decode(v.trim()).ok())
        .and_then(|v| String::from_utf8(v).ok());
    let Some((id, secret)) = decoded.as_deref().and_then(|d| d.split_once(':')) else {
        return false;
    };
    let decode = |s: &str| -> String {
        form_urlencoded::parse(format!("x={s}").as_bytes())
            .next()
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default()
    };
    decode(id) == idp.config.client_id && decode(secret) == idp.secret
}

async fn token(
    State(idp): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    if !client_authenticated(&idp, &headers) {
        return token_error(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    let invalid = || token_error(StatusCode::BAD_REQUEST, "invalid_grant");
    let get = |name: &str| form.get(name).map(String::as_str);
    let (sub, nonce) = match get("grant_type") {
        Some("authorization_code") => {
            let grant = get("code").and_then(|c| {
                idp.codes
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(c)
            });
            let Some(grant) = grant.filter(|g| g.issued.elapsed() < CODE_WITHIN) else {
                return invalid();
            };
            let verified = get("code_verifier").is_some_and(|v| {
                URL_SAFE_NO_PAD.encode(Sha256::digest(v.as_bytes())) == grant.challenge
            });
            if !verified || get("redirect_uri") != Some(grant.redirect_uri.as_str()) {
                return invalid();
            }
            (grant.sub, grant.nonce)
        }
        Some("refresh_token") => {
            // Rotated at each use (RFC 9700 §4.14.2).
            let sub = get("refresh_token").and_then(|r| {
                idp.refresh
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(r)
            });
            let Some(sub) = sub else { return invalid() };
            (sub, None)
        }
        _ => return token_error(StatusCode::BAD_REQUEST, "unsupported_grant_type"),
    };
    let Some(user) = idp.config.users.iter().find(|u| u.sub == sub) else {
        return invalid();
    };
    let Ok(id_token) = id_token(&idp, user, nonce.as_deref()) else {
        return token_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error");
    };
    let refresh = URL_SAFE_NO_PAD.encode(random::<32>());
    idp.refresh
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(refresh.clone(), sub);
    let mut response = Json(json!({
        "access_token": URL_SAFE_NO_PAD.encode(random::<32>()),
        "token_type": "Bearer",
        "expires_in": TOKEN_SECONDS,
        "id_token": id_token,
        "refresh_token": refresh,
    }))
    .into_response();
    // RFC 6749 §5.1.
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn id_token(idp: &Idp, user: &User, nonce: Option<&str>) -> jsonwebtoken::errors::Result<String> {
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
    .unwrap_or(i64::MAX);
    let mut claims = json!({
        "iss": idp.config.issuer,
        "aud": idp.config.client_id,
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
    let mut header = Header::new(Algorithm::ES256);
    header.kid.clone_from(&idp.jwk.common.key_id);
    encode(&header, &claims, &idp.signing)
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
mod tests {
    use super::*;

    #[test]
    fn origins_and_escapes() {
        assert_eq!(
            origin("http://127.0.0.1:5173/v1/auth/callback"),
            Some("http://127.0.0.1:5173")
        );
        assert_eq!(origin("https://h"), Some("https://h"));
        assert_eq!(origin("nope"), None);
        assert_eq!(
            escape(r#"<a href="x">'&"#),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;"
        );
    }
}
