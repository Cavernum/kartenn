//! The OIDC flow of the BFF against an IdP in the test process (spec 005, REQ-005-1, 2, 4
//! to 8; task 005-12): a complete login, each refusal of the callback, the session
//! cookie and its lifetimes, the refresh of the groups, the logout URL.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use kartenn_auth::http::HttpClient;
use kartenn_auth::issuer::IssuerConfig;
use kartenn_auth::keys::KeyRing;
use kartenn_auth::oidc::{Callback, LoginError, Oidc, Session, SessionError};
use kartenn_auth::tokens::{HttpKeys, TokenError, TokenVerifier};
use p256::pkcs8::EncodePrivateKey;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const CLIENT: &str = "kartenn";
const SECRET: &str = "s3cret";
const REDIRECT: &str = "https://kartenn.example.invalid/auth/callback";

/// A code the IdP issued, with what the login asked.
struct Issued {
    nonce: String,
    challenge: String,
}

/// What the IdP does, and what it saw.
struct State {
    base: String,
    signing: EncodingKey,
    jwk: Jwk,
    codes: HashMap<String, Issued>,
    iss_supported: bool,
    nonce_override: Option<String>,
    audience_override: Option<String>,
    groups: Vec<String>,
}

type Idp = Arc<Mutex<State>>;

fn decode_form(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (unescape(k), unescape(v)))
        .collect()
}

fn unescape(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 1 {
            out.push(u8::from_str_radix(&value[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

fn id_token(state: &State, nonce: Option<&str>) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut claims = json!({
        "iss": state.base, "aud": state.audience_override.clone().unwrap_or_else(|| CLIENT.into()),
        "sub": "u-42", "name": "A. Person", "iat": now, "exp": now + 300,
        "groups": state.groups,
    });
    if let Some(nonce) = state.nonce_override.as_deref().or(nonce) {
        claims["nonce"] = nonce.into();
    }
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some("k1".into());
    encode(&header, &claims, &state.signing).unwrap()
}

async fn handle(
    idp: Idp,
    request: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = request.uri().path().to_owned();
    let auth = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = String::from_utf8(
        request
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    let mut state = idp.lock().unwrap();
    let reply = |status: u16, body: serde_json::Value| {
        Ok(Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(body.to_string())))
            .unwrap())
    };
    match path.as_str() {
        "/.well-known/openid-configuration" => reply(
            200,
            json!({
                "issuer": state.base,
                "authorization_endpoint": format!("{}/authorize", state.base),
                "token_endpoint": format!("{}/token", state.base),
                "jwks_uri": format!("{}/jwks", state.base),
                "end_session_endpoint": format!("{}/logout", state.base),
                "authorization_response_iss_parameter_supported": state.iss_supported,
            }),
        ),
        "/jwks" => reply(
            200,
            serde_json::to_value(JwkSet {
                keys: vec![state.jwk.clone()],
            })
            .unwrap(),
        ),
        "/token" => {
            let expected = format!("Basic {}", STANDARD.encode(format!("{CLIENT}:{SECRET}")));
            if auth.as_deref() != Some(expected.as_str()) {
                return reply(401, json!({ "error": "invalid_client" }));
            }
            let form = decode_form(&body);
            match form.get("grant_type").map(String::as_str) {
                Some("authorization_code") => {
                    let Some(issued) = form.get("code").and_then(|c| state.codes.remove(c)) else {
                        return reply(400, json!({ "error": "invalid_grant" }));
                    };
                    // PKCE: the verifier of the challenge (RFC 7636 §4.6).
                    let verifier = form.get("code_verifier").cloned().unwrap_or_default();
                    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
                    if challenge != issued.challenge
                        || form.get("redirect_uri").map(String::as_str) != Some(REDIRECT)
                    {
                        return reply(400, json!({ "error": "invalid_grant" }));
                    }
                    let token = id_token(&state, Some(&issued.nonce));
                    reply(
                        200,
                        json!({ "id_token": token, "refresh_token": "r1", "expires_in": 300, "token_type": "Bearer", "access_token": "a" }),
                    )
                }
                Some("refresh_token")
                    if form.get("refresh_token").map(String::as_str) == Some("r1") =>
                {
                    let token = id_token(&state, None);
                    reply(
                        200,
                        json!({ "id_token": token, "refresh_token": "r2", "expires_in": 300, "token_type": "Bearer", "access_token": "a" }),
                    )
                }
                _ => reply(400, json!({ "error": "invalid_grant" })),
            }
        }
        _ => reply(404, json!({})),
    }
}

/// Starts the IdP; its base URL is its issuer identifier.
async fn idp() -> Idp {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let secret = p256::SecretKey::random(&mut rand_core::OsRng);
    let pem = secret
        .to_pkcs8_pem(p256::pkcs8::LineEnding::default())
        .unwrap();
    let signing = EncodingKey::from_ec_pem(pem.as_bytes()).unwrap();
    let mut jwk = Jwk::from_encoding_key(&signing, Algorithm::ES256).unwrap();
    jwk.common.key_id = Some("k1".into());
    let idp: Idp = Arc::new(Mutex::new(State {
        base,
        signing,
        jwk,
        codes: HashMap::new(),
        iss_supported: false,
        nonce_override: None,
        audience_override: None,
        groups: vec!["netops".into()],
    }));
    let served = Arc::clone(&idp);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let idp = Arc::clone(&served);
            tokio::spawn(async move {
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(
                        TokioIo::new(stream),
                        service_fn(move |r| handle(Arc::clone(&idp), r)),
                    )
                    .await;
            });
        }
    });
    idp
}

fn client(idp: &Idp) -> (Oidc<HttpKeys>, KeyRing) {
    let base = idp.lock().unwrap().base.clone();
    let issuer = IssuerConfig {
        id: "main".into(),
        issuer: base,
        client_id: CLIENT.into(),
        client_secret: SECRET.into(),
        audience: "kartenn".into(),
        groups_claim: "groups".into(),
        algorithms: vec![Algorithm::ES256],
    };
    let http = HttpClient::new().unwrap();
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
    let tokens = TokenVerifier::new(vec![issuer], HttpKeys(http.clone()));
    (Oidc::new(tokens, http, ring.clone(), REDIRECT.into()), ring)
}

/// The query parameter `name` of `url`.
fn param(url: &str, name: &str) -> String {
    url.split_once('?')
        .unwrap()
        .1
        .split('&')
        .find_map(|p| p.strip_prefix(&format!("{name}=")))
        .unwrap()
        .to_owned()
}

/// Plays the browser and the user at the IdP: a code for the login started at `url`.
fn authorize(idp: &Idp, url: &str) -> String {
    let code = Uuid::now_v7().to_string();
    idp.lock().unwrap().codes.insert(
        code.clone(),
        Issued {
            nonce: param(url, "nonce"),
            challenge: param(url, "code_challenge"),
        },
    );
    code
}

#[tokio::test]
async fn a_login_gives_a_session_that_lives_and_refreshes() {
    let idp = idp().await;
    let (oidc, ring) = client(&idp);
    let start = oidc.begin("main").await.unwrap();
    assert!(start.url.contains("code_challenge_method=S256"));
    assert!(start.url.contains("response_type=code"));
    let code = authorize(&idp, &start.url);
    let cookie: Vec<&str> = start.cookie.iter().map(String::as_str).collect();
    let callback = Callback {
        code: Some(code),
        state: Some(param(&start.url, "state")),
        ..Callback::default()
    };
    let login = oidc.complete(&callback, &cookie).await.unwrap();
    assert_eq!(login.subject, "u-42");
    assert_eq!(login.name.as_deref(), Some("A. Person"));
    assert_eq!(login.groups, ["netops"]);

    // The session: sealed, opened by another instance, bounded in time.
    let identity = Uuid::now_v7();
    let session = Session::new(identity, &login);
    let sealed = session.seal(&ring).unwrap();
    let chunks: Vec<&str> = sealed.iter().map(String::as_str).collect();
    let at = session.started;
    let opened = Session::open(&ring.clone(), &chunks, at + 60, 1_800, 28_800).unwrap();
    assert_eq!(opened.identity, identity);
    assert_eq!(
        Session::open(&ring, &chunks, at + 1_800, 1_800, 28_800),
        Err(SessionError::Idle)
    );
    assert_eq!(
        Session::open(&ring, &chunks, at + 28_800, 1_000_000_000, 28_800),
        Err(SessionError::Expired)
    );
    let mut forged = sealed[0].clone();
    forged.push('A');
    assert_eq!(
        Session::open(&ring, &[&forged], at, 1_800, 28_800),
        Err(SessionError::Invalid)
    );

    // A group withdrawn at the IdP is gone after the refresh (REQ-005-7).
    idp.lock().unwrap().groups = vec!["readers".into()];
    let mut refreshed = opened;
    oidc.refresh(&mut refreshed).await.unwrap();
    assert_eq!(refreshed.groups, ["readers"]);
    assert_eq!(refreshed.refresh_token.as_deref(), Some("r2"));

    // The logout goes through the IdP too, with the ID token as hint (REQ-005-8).
    let logout = oidc
        .logout_url(&refreshed, Some(REDIRECT))
        .await
        .unwrap()
        .unwrap();
    assert!(logout.contains("/logout?client_id=kartenn"));
    assert!(logout.contains("id_token_hint="));
}

#[tokio::test]
async fn each_refusal_of_the_callback() {
    let idp = idp().await;
    let (oidc, _) = client(&idp);
    let attempt = |change: Box<dyn Fn(&mut Callback) + Send>| {
        let (oidc, idp) = (&oidc, &idp);
        async move {
            let start = oidc.begin("main").await.unwrap();
            let code = authorize(idp, &start.url);
            let cookie: Vec<&str> = start.cookie.iter().map(String::as_str).collect();
            let mut callback = Callback {
                code: Some(code),
                state: Some(param(&start.url, "state")),
                ..Callback::default()
            };
            change(&mut callback);
            oidc.complete(&callback, &cookie).await.map(|_| ())
        }
    };
    // A state that is not the login's (CSRF).
    assert_eq!(
        attempt(Box::new(|c| c.state = Some("forged".into()))).await,
        Err(LoginError::State)
    );
    // A response from another issuer (mix-up, RFC 9207).
    assert_eq!(
        attempt(Box::new(
            |c| c.iss = Some("https://evil.example.invalid".into())
        ))
        .await,
        Err(LoginError::Issuer)
    );
    // An issuer that publishes RFC 9207 must send `iss`.
    idp.lock().unwrap().iss_supported = true;
    let (strict, _) = client(&idp);
    let start = strict.begin("main").await.unwrap();
    let code = authorize(&idp, &start.url);
    let cookie: Vec<&str> = start.cookie.iter().map(String::as_str).collect();
    let callback = Callback {
        code: Some(code),
        state: Some(param(&start.url, "state")),
        ..Callback::default()
    };
    assert_eq!(
        strict.complete(&callback, &cookie).await.map(|_| ()),
        Err(LoginError::Issuer)
    );
    idp.lock().unwrap().iss_supported = false;
    // An ID token of another login (nonce), or for another client (audience).
    idp.lock().unwrap().nonce_override = Some("another-login".into());
    let refused = attempt(Box::new(|_| {})).await.unwrap_err();
    assert_eq!(refused, LoginError::IdToken(TokenError::Nonce));
    assert_eq!(refused.reason(), "nonce");
    idp.lock().unwrap().nonce_override = None;
    idp.lock().unwrap().audience_override = Some("another-client".into());
    let refused = attempt(Box::new(|_| {})).await.unwrap_err();
    assert_eq!(refused.reason(), "audience");
    idp.lock().unwrap().audience_override = None;
    // A code replayed, or a code with another PKCE verifier: the IdP refuses the exchange.
    assert!(matches!(
        attempt(Box::new(|c| c.code = Some("unknown".into()))).await,
        Err(LoginError::Exchange(_))
    ));
    // No login cookie, or a forged one.
    let start = oidc.begin("main").await.unwrap();
    let callback = Callback {
        code: Some(authorize(&idp, &start.url)),
        state: Some(param(&start.url, "state")),
        ..Callback::default()
    };
    assert_eq!(
        oidc.complete(&callback, &["v1.k1.forged"])
            .await
            .map(|_| ()),
        Err(LoginError::Transaction)
    );
    // And the flow still works after all that.
    assert_eq!(attempt(Box::new(|_| {})).await, Ok(()));
}
